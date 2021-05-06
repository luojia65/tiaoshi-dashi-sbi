use riscv::register::{
    mstatus::Mstatus,
    mtval, mcause::{self, Trap, Exception},
    mtvec::{self, TrapMode},
    mstatus::{self, MPP},
};
use core::{
    pin::Pin,
    ops::{Generator, GeneratorState},
};

pub fn init() {
    let mut addr = machine_trap_entry as usize;
    if addr & 0x2 != 0 {
        addr += 0x2; // 必须对齐到4个字节
    }
    unsafe { mtvec::write(addr, TrapMode::Direct) };
}

#[repr(C)]
pub struct Runtime {
    context: KernelContext,
}

impl Runtime {
    pub fn new(mhartid: usize, opaque: usize, mepc: usize) -> Runtime {
        let mut context: KernelContext = unsafe { core::mem::MaybeUninit::zeroed().assume_init() };
        unsafe { mstatus::set_mpp(MPP::Supervisor) };
        context.mstatus = mstatus::read();
        context.mepc = mepc;
        context.a0 = mhartid;
        context.a1 = opaque;
        Runtime { context }
    }

    pub fn context_mut(&mut self) -> &mut KernelContext {
        &mut self.context
    }
}

impl Generator for Runtime {
    type Yield = MachineTrap;
    type Return = ();
    fn resume(mut self: Pin<&mut Self>, _arg: ()) -> GeneratorState<Self::Yield, Self::Return> {
        unsafe { do_resume(&mut self.context as *mut _) };
        let mtval = mtval::read();
        let trap = match mcause::read().cause() {
            Trap::Exception(Exception::SupervisorEnvCall) => MachineTrap::SbiCall(),
            Trap::Exception(Exception::LoadFault) => MachineTrap::LoadAccessFault(mtval),
            Trap::Exception(Exception::StoreFault) => MachineTrap::StoreAccessFault(mtval),
            Trap::Exception(Exception::IllegalInstruction) => MachineTrap::IllegalInstruction(mtval),
            e => panic!("unhandled exception: {:?}! mtval: {:#x?}, ctx: {:#x?}", e, mtval, self.context)
        };
        GeneratorState::Yielded(trap)
    }
}

#[repr(C)]
pub enum MachineTrap {
    SbiCall(),
    LoadAccessFault(usize),
    StoreAccessFault(usize),
    IllegalInstruction(usize),
}

#[derive(Debug)]
#[repr(C)]
pub struct KernelContext {
    pub ra: usize, // 0
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize, // 30
    pub mstatus: Mstatus, // 31
    pub mepc: usize, // 32
    pub machine_stack: usize, // 33
}

#[naked]
#[link_section = ".text"]
unsafe extern "C" fn do_resume(_kernel_context: *mut KernelContext) {
    asm!("j     {machine_resume}", machine_resume = sym machine_resume, options(noreturn))
}

#[naked]
#[link_section = ".text"]
unsafe extern "C" fn machine_resume() -> ! {
    asm!( // sp:机器栈顶
        "addi   sp, sp, -15*8", // sp:机器栈顶
        // 进入函数之前，已经保存了调用者寄存器，应当保存被调用者寄存器
        "sd     ra, 0*8(sp)
        sd      gp, 1*8(sp)
        sd      tp, 2*8(sp)
        sd      s0, 3*8(sp)
        sd      s1, 4*8(sp)
        sd      s2, 5*8(sp)
        sd      s3, 6*8(sp)
        sd      s4, 7*8(sp)
        sd      s5, 8*8(sp)
        sd      s6, 9*8(sp)
        sd      s7, 10*8(sp)
        sd      s8, 11*8(sp)
        sd      s9, 12*8(sp)
        sd      s10, 13*8(sp)
        sd      s11, 14*8(sp)", 
        // a0:内核上下文
        "sd     sp, 33*8(a0)", // 机器栈顶放进内核上下文
        "csrw   mscratch, a0", // 新mscratch:内核上下文
        "mv     sp, a0", // 新sp:内核上下文
        "ld     t0, 31*8(sp)
        ld      t1, 32*8(sp)
        csrw    mstatus, t0
        csrw    mepc, t1",
        "ld     ra, 0*8(sp)
        ld      gp, 2*8(sp)
        ld      tp, 3*8(sp)
        ld      t0, 4*8(sp)
        ld      t1, 5*8(sp)
        ld      t2, 6*8(sp)
        ld      s0, 7*8(sp)
        ld      s1, 8*8(sp)
        ld      a0, 9*8(sp)
        ld      a1, 10*8(sp)
        ld      a2, 11*8(sp)
        ld      a3, 12*8(sp)
        ld      a4, 13*8(sp)
        ld      a5, 14*8(sp)
        ld      a6, 15*8(sp)
        ld      a7, 16*8(sp)
        ld      s2, 17*8(sp)
        ld      s3, 18*8(sp)
        ld      s4, 19*8(sp)
        ld      s5, 20*8(sp)
        ld      s6, 21*8(sp)
        ld      s7, 22*8(sp)
        ld      s8, 23*8(sp)
        ld      s9, 24*8(sp)
        ld     s10, 25*8(sp)
        ld     s11, 26*8(sp)
        ld      t3, 27*8(sp)
        ld      t4, 28*8(sp)
        ld      t5, 29*8(sp)
        ld      t6, 30*8(sp)",
        "ld     sp, 1*8(sp)", // 新sp:内核栈
        // sp:内核栈, mscratch:内核上下文
        "mret",
        options(noreturn)
    )
}

// 中断开始

#[naked]
#[link_section = ".text"]
pub unsafe extern "C" fn machine_trap_entry() -> ! {
    asm!( // sp:内核栈,mscratch:内核上下文
        ".p2align 2",
        "csrrw  sp, mscratch, sp", // 新mscratch:内核栈, 新sp:内核上下文
        "sd     ra, 0*8(sp)
        sd      gp, 2*8(sp)
        sd      tp, 3*8(sp)
        sd      t0, 4*8(sp)
        sd      t1, 5*8(sp)
        sd      t2, 6*8(sp)
        sd      s0, 7*8(sp)
        sd      s1, 8*8(sp)
        sd      a0, 9*8(sp)
        sd      a1, 10*8(sp)
        sd      a2, 11*8(sp)
        sd      a3, 12*8(sp)
        sd      a4, 13*8(sp)
        sd      a5, 14*8(sp)
        sd      a6, 15*8(sp)
        sd      a7, 16*8(sp)
        sd      s2, 17*8(sp)
        sd      s3, 18*8(sp)
        sd      s4, 19*8(sp)
        sd      s5, 20*8(sp)
        sd      s6, 21*8(sp)
        sd      s7, 22*8(sp)
        sd      s8, 23*8(sp)
        sd      s9, 24*8(sp)
        sd     s10, 25*8(sp)
        sd     s11, 26*8(sp)
        sd      t3, 27*8(sp)
        sd      t4, 28*8(sp)
        sd      t5, 29*8(sp)
        sd      t6, 30*8(sp)",
        "csrr   t0, mstatus
        sd      t0, 31*8(sp)",
        "csrr   t1, mepc
        sd      t1, 32*8(sp)",
        // mscratch:内核栈,sp:内核上下文
        "csrrw  t2, mscratch, sp", // 新mscratch:内核上下文,t2:内核栈
        "sd     t2, 1*8(sp)", // 保存内核栈
        "csrr   sp, mscratch", // sp:内核上下文
        "ld     sp, 33*8(sp)", // sp:机器栈
        "ld     ra, 0*8(sp)
        ld      gp, 1*8(sp)
        ld      tp, 2*8(sp)
        ld      s0, 3*8(sp)
        ld      s1, 4*8(sp)
        ld      s2, 5*8(sp)
        ld      s3, 6*8(sp)
        ld      s4, 7*8(sp)
        ld      s5, 8*8(sp)
        ld      s6, 9*8(sp)
        ld      s7, 10*8(sp)
        ld      s8, 11*8(sp)
        ld      s9, 12*8(sp)
        ld      s10, 13*8(sp)
        ld      s11, 14*8(sp)", 
        "addi   sp, sp, 15*8", // sp:机器栈顶
        "jr     ra", // 其实就是ret
        options(noreturn)
    )
}
