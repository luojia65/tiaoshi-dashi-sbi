#![feature(naked_functions, asm)]
#![feature(generator_trait)]
#![feature(alloc_error_handler)]

#![no_std]
#![no_main]

extern crate alloc;

mod executor;
mod reset;
mod uart;

use rustsbi::println;

const STACK_SIZE: usize = 0x10000 * 8;

use core::pin::Pin;
use core::ops::{Generator, GeneratorState};
use executor::{Runtime, MachineTrap};

fn rust_main(mhartid: usize, opaque: usize) -> ! { 
    if mhartid == 0 {
        first_hart_init();
    }
    let addr = 0x8020_0000;
    executor::init();
    let mut rt = Runtime::new(mhartid, opaque, addr);
    loop {
        match Pin::new(&mut rt).resume(()) {
            GeneratorState::Yielded(MachineTrap::SbiCall()) => {
                let ctx = rt.context_mut();
                let (extension, function, param) = (ctx.a7, ctx.a6, 
                    [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4]);
                let ans = rustsbi::ecall(extension, function, param);
                ctx.a0 = ans.error;
                ctx.a1 = ans.value;
                ctx.mepc = ctx.mepc.wrapping_add(4);
            }
            GeneratorState::Yielded(_trap) => todo!(),
            GeneratorState::Complete(()) => shutdown(),
        }
    }
}

fn first_hart_init() {
    // todo: clean bss memory using r0
    // todo: i18n
    init_alloc();
    init_println();
    init_reset();
    println!("RustSBI version: {}", rustsbi::VERSION);
}

const HEAP_SIZE: usize = 0x10_000;

static mut MACHINE_HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

fn init_alloc() {
    unsafe {
        let heap_bottom = &mut MACHINE_HEAP as *mut _ as usize;
        ALLOCATOR.lock().init(heap_bottom, HEAP_SIZE);
    }
}

fn init_println() {
    let serial = uart::Ns16550a::new(0x10000000, 0, 11_059_200, 115200);
    rustsbi::legacy_stdio::init_legacy_stdio_embedded_hal(serial);
}

fn init_reset() {
    rustsbi::init_reset(reset::Reset);
}

fn shutdown() -> ! {
    use rustsbi::Reset;
    reset::Reset.system_reset(
        rustsbi::reset::RESET_TYPE_SHUTDOWN,
        rustsbi::reset::RESET_REASON_NO_REASON,
    );
    loop {}
}

use core::panic::PanicInfo;

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(info: &PanicInfo) -> ! {
    let hart_id = riscv::register::mhartid::read();
    // 输出的信息大概是“[rustsbi-panic] hart 0 panicked at ...”
    println!("[rustsbi-panic] hart {} {}", hart_id, info);
    println!("[rustsbi-panic] system shutdown scheduled due to SBI panic");
    use rustsbi::Reset;
    reset::Reset.system_reset(
        rustsbi::reset::RESET_TYPE_SHUTDOWN,
        rustsbi::reset::RESET_REASON_SYSTEM_FAILURE
    );
    loop { }
}

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

use alloc::alloc::Layout;

#[cfg_attr(not(test), alloc_error_handler)]
#[allow(unused)]
fn alloc_error(layout: Layout) -> ! {
    println!("[rustsbi] out of memory for layout {:?}", layout);
    use rustsbi::Reset;
    reset::Reset.system_reset(
        rustsbi::reset::RESET_TYPE_SHUTDOWN,
        rustsbi::reset::RESET_REASON_SYSTEM_FAILURE
    );
    loop {}
}

#[link_section = ".bss.stack"]
static mut MACHINE_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

#[naked]
#[link_section = ".text.entry"] 
#[export_name = "_start"]
unsafe extern "C" fn entry() -> ! {
    asm!(
    // 1. set sp = bootstack + (hartid + 1) * 0x10000
    "add    t0, a0, 1
    slli    t0, t0, 14
    la      sp, {machine_stack}
    add     sp, sp, t0",
    // 2. jump to rust_main (absolute address)
    "j      {rust_main}", 
    machine_stack = sym MACHINE_STACK, 
    rust_main = sym rust_main,
    options(noreturn))
}
