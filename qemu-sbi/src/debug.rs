use crate::executor::KernelContext;
use rustsbi::{print, println, legacy_stdio_putchar, legacy_stdio_getchar};
use alloc::vec::Vec;
use alloc::string::String;

const INPUT_LIMIT: usize = 256;

pub fn on_breakpoint(ctx: &mut KernelContext) {
    println!("[DebugSBI] Breakpoint at {:#x}", ctx.mepc);
    loop {
        if console_loop() {
            break
        }
    }
}

fn console_loop() -> bool {
    print!("[DebugSBI] (debug) ");
    let mut input_buf = Vec::new();
    loop {
        let input = legacy_stdio_getchar();
        match input {
            // 退格键
            8 => {
                if let Some(_) = input_buf.pop() {
                    legacy_stdio_putchar(8);
                    legacy_stdio_putchar(b' ');
                    legacy_stdio_putchar(8);
                } // 否则什么也不做
            },
            // 回车键
            13 => {
                legacy_stdio_putchar(10);
                legacy_stdio_putchar(13);
                break
            },
            // 其它控制字符
            0 ..= 31 => {
                println!("");
                break
            }
            // 其它字符
            input => {
                if input_buf.len() < INPUT_LIMIT {
                    input_buf.push(input);
                    legacy_stdio_putchar(input);
                }
            }
        }
    }
    match String::from_utf8(input_buf) {
        Ok(s) => process_command(&s),
        Err(e) => {
            println!("UTF-8 error! {:?}", e);
            false // 不退出
        },
    }
}

fn process_command(input: &str) -> bool {
    // println!("Your input: {}", input);
    let parts: Vec<_> = input.split(' ').collect();
    if parts.len() == 0 {
        return false;
    }
    match parts[0] {
        "x" => {
            println!("[DebugSBI] (0x1000) = 0x00000297"); // todo
            return false;
        }
        "c" => {
            println!("[DebugSBI] Continuing.");
            return true
        },
        unknown => {
            println!("[DebugSBI] Unknown command '{}'", unknown);
            return false
        },
    }
}
