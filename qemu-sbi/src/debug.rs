use crate::executor::KernelContext;
use rustsbi::{print, println, legacy_stdio_putchar, legacy_stdio_getchar};
use alloc::vec::Vec;
use alloc::format;
use alloc::string::String;

const INPUT_LIMIT: usize = 256;

pub fn on_breakpoint(ctx: &mut KernelContext) {
    println!("[DebugSBI] Breakpoint at {:#x}", ctx.mepc);
    loop {
        match get_command() {
            Ok(ControlFlow::Continue) => continue,
            Ok(ControlFlow::Break) => break,
            Err(e) => println!("Error: {:?}", e),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ControlFlow {
    Break,
    Continue
}

fn get_command() -> Result<ControlFlow, ParseError> {
    let buf = fill_input_buffer();
    let string = match String::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => return Err(ParseError::InvalidUtf8(e))
    };
    let mut metadata = Metadata::default();
    let mut iter = lexer(&string);
    // println!("{:?}", lexer(&string).collect::<Vec<_>>());
    let mut sym = iter.next();
    command(&mut iter, &mut sym, &mut metadata).map_err(|_| ParseError::SyntaxError)?;
    Ok(execute_command(&metadata))
}

fn execute_command(metadata: &Metadata) -> ControlFlow {
    // println!("Metadata: {:?}", metadata);
    if let Some(CommandType::X) = metadata.command_type {
        let address = if let Some(address) = metadata.address_number {
            address
        } else {
            println!("[DebugSBI] Address not provided for command x");
            return ControlFlow::Continue;
        };
        let mut ans: u128 = 0;
        let mut ansi: i128 = 0;
        let ans_signed;
        // todo: 优化
        let ty = if metadata.data_type == None {
            Some(DataType::Basic(BasicType { signed: true, width: (core::mem::size_of::<usize>() * 8) as u8 }))
        } else {
            metadata.data_type.clone() 
        };
        match ty {
            Some(DataType::Basic(BasicType { signed, width })) => {
                ans_signed = signed;
                match (signed, width) {
                    (false, 8) => ans = unsafe { core::ptr::read_volatile(address as *const u8) as u128 },
                    (false, 16) => ans = unsafe { core::ptr::read_volatile(address as *const u16) as u128 },
                    (false, 32) => ans = unsafe { core::ptr::read_volatile(address as *const u32) as u128 },
                    (false, 64) => ans = unsafe { core::ptr::read_volatile(address as *const u64) as u128 },
                    (false, 128) => ans = unsafe { core::ptr::read_volatile(address as *const u128) },
                    (true, 8) => ansi = unsafe { core::ptr::read_volatile(address as *const i8) as i128 },
                    (true, 16) => ansi = unsafe { core::ptr::read_volatile(address as *const i16) as i128 },
                    (true, 32) => ansi = unsafe { core::ptr::read_volatile(address as *const i32) as i128 },
                    (true, 64) => ansi = unsafe { core::ptr::read_volatile(address as *const i64) as i128 },
                    (true, 128) => ansi = unsafe { core::ptr::read_volatile(address as *const i128) as i128 },
                    _ => unreachable!()
                };
            }
            None => unreachable!(),
            _ => todo!(),
        }
        let value = if metadata.print_mode == Some(PrintMode::Decimal) {
            if ans_signed { format!("{}", ansi) } else { format!("{}", ans) }
        } else {
            if ans_signed { format!("{:#x}", ansi) } else { format!("{:#x}", ans) }
        };
        println!("[DebugSBI] PhysMem[{:#x}], Machine = {}", address, value);
    } else if let Some(CommandType::C) = metadata.command_type {
        println!("[DebugSBI] Continuing.");
        return ControlFlow::Break
    }
    ControlFlow::Continue
}

struct Lexer<I: Iterator> {
    iter: core::iter::Peekable<I>
}

impl<I: Iterator<Item = char>> Iterator for Lexer<I> {
    type Item = Word;
    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.peek() {
            Some('1' ..= '9')  => {
                let mut ans = String::new();
                loop {
                    match self.iter.peek() {
                        Some(&ch @ '0' ..= '9') => {
                            ans.push(ch);
                            self.iter.next();
                        }
                        _ => break
                    }
                }
                if let Ok(integer) = ans.parse() {
                    Some(Word::Integer(integer))
                } else {
                    Some(Word::Other)
                }
            },
            Some('0')  => {
                let mut ans = String::new();
                self.iter.next();
                match self.iter.peek() {
                    Some('x') => self.iter.next(),
                    Some('0' ..= '9' | 'a'..= 'f' | 'A' ..= 'F') => return Some(Word::Other),
                    _ => return Some(Word::Integer(0))
                };
                loop {
                    match self.iter.peek() {
                        Some(&ch @ '0' ..= '9' | &ch @ 'a'..= 'f' | &ch @ 'A' ..= 'F') => {
                            ans.push(ch);
                            self.iter.next();
                        }
                        Some(_) | None => break
                    }
                }
                if let Ok(integer) = usize::from_str_radix(&ans, 16) {
                    Some(Word::Integer(integer))
                } else {
                    Some(Word::Other)
                }
            },
            Some('/') => { self.iter.next(); Some(Word::Backslash) },
            Some('[') => { self.iter.next(); Some(Word::LeftSquareBracket) },
            Some(']') => { self.iter.next(); Some(Word::RightSquareBracket) },
            Some(';') => { self.iter.next(); Some(Word::Semicolon) },
            Some(' ') | Some('\t') => { 
                loop {
                    match self.iter.peek() {
                        Some(' ') | Some('\t') => self.iter.next(),
                        _ => break
                    };
                }
                Some(Word::Space) 
            },
            Some(&ch @'a'..='z' | &ch @ 'A'..='Z') => { self.iter.next(); Some(Word::Character(ch)) },
            None => None,
            _ => { self.iter.next(); Some(Word::Other) }
        }
    }
}

fn lexer(input: &str) -> Lexer<alloc::str::Chars> {
    Lexer {
        iter: input.chars().peekable() // LL(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Word {
    Character(char),
    Integer(usize),
    Backslash,
    LeftSquareBracket,
    RightSquareBracket,
    Semicolon,
    Space,
    Other,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct Metadata {
    command_type: Option<CommandType>,
    privileged_mode: Option<PrivilegeMode>,
    data_type: Option<DataType>,
    address_number: Option<usize>,
    print_mode: Option<PrintMode>,
}

fn command<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  { 
    if *sym == Some(Word::Character('x')) {
        x(iter, sym, m)
    } else if *sym == Some(Word::Character('c')) {
        c(iter, sym, m)
    } else {
        Err(())
    }
}

fn c<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  { 
    if *sym == Some(Word::Character('c')) {
        *sym = iter.next();
        m.command_type = Some(CommandType::C);
        Ok(())
    } else {
        Err(())
    }
}

fn x<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  {
    // println!("x; sym = {:?}", *sym);
    if *sym == Some(Word::Character('x')) {
        *sym = iter.next();
        privilege_mode(iter, sym, m)?;
        if *sym == Some(Word::Backslash) {
            *sym = iter.next();
            data_type(iter, sym, m)?;
            print_mode(iter, sym, m)?;
        } 
        space(iter, sym)?;
        let address_number = if let Some(Word::Integer(i)) = *sym {
            i
        } else {
            return Err(())
        };
        m.command_type = Some(CommandType::X);
        m.address_number = Some(address_number);
        Ok(())
    } else {
        Err(())
    }
}

// P → m | s | u | ε
fn privilege_mode<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  {
    // println!("privilege_mode; sym = {:?}", *sym);
    if *sym == Some(Word::Character('m')) {
        *sym = iter.next();
        m.privileged_mode = Some(PrivilegeMode::Machine);
        Ok(())
    } else if *sym == Some(Word::Character('s')) {
        *sym = iter.next();
        m.privileged_mode = Some(PrivilegeMode::Supervisor);
        Ok(())
    } else if *sym == Some(Word::Character('u')) {
        *sym = iter.next();
        m.privileged_mode = Some(PrivilegeMode::User);
        Ok(())
    } else if *sym == Some(Word::Backslash) || *sym == None || *sym == Some(Word::Space) {
        m.privileged_mode = Some(PrivilegeMode::Current);
        Ok(())
    } else {
        Err(())
    }
}

// T → 类型 | [类型; 常数]
fn data_type<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  {
    // println!("data_type; sym = {:?}", *sym);
    if *sym == Some(Word::LeftSquareBracket) {
        *sym = iter.next();
        let basic_type = basic_type(iter, sym)?;
        if *sym != Some(Word::Semicolon) {
            return Err(())
        }
        *sym = iter.next();
        space(iter, sym)?;
        let array_len = if let Some(Word::Integer(i)) = *sym {
            i
        } else {
            return Err(())
        };
        *sym = iter.next();
        if *sym != Some(Word::RightSquareBracket) {
            return Err(())
        }
        *sym = iter.next();
        let array = DataType::Array(basic_type, array_len);
        m.data_type = Some(array);
        Ok(())
    } else if *sym == Some(Word::Character('z')) {
        *sym = iter.next();
        m.data_type = Some(DataType::Instruction);
        Ok(())
    } else {
        if let Ok(basic_type) = basic_type(iter, sym) {
            m.data_type = Some(DataType::Basic(basic_type));
        }
        Ok(())
    }
}

fn basic_type<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>) -> Result<BasicType, ()>  {
    // println!("data_type; sym = {:?}", *sym);
    if *sym == Some(Word::Character('u')) {
        *sym = iter.next();
        if let Some(Word::Integer(i)) = *sym {
            if !is_valid_width(i) {
                return Err(())
            }
            *sym = iter.next();
            let width = i as u8;
            Ok(BasicType { signed: false, width })
        } else {
            Err(())
        }
    } else if *sym == Some(Word::Character('i')) {
        *sym = iter.next();
        if let Some(Word::Integer(i)) = *sym {
            if !is_valid_width(i) {
                return Err(())
            }
            *sym = iter.next();
            let width = i as u8;
            Ok(BasicType { signed: true, width })
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}

fn is_valid_width(a: usize) -> bool {
    a == 8 || a == 16 || a == 32 || a == 64 || a == 128
}

fn space<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>) -> Result<(), ()> {
    while *sym == Some(Word::Space) {
        *sym = iter.next();
    } 
    Ok(())
}

fn print_mode<I: Iterator<Item = Word>>(iter: &mut I, sym: &mut Option<Word>, m: &mut Metadata) -> Result<(), ()>  {
    // println!("privilege_mode; sym = {:?}", *sym);
    if *sym == Some(Word::Character('d')) {
        *sym = iter.next();
        m.print_mode = Some(PrintMode::Decimal);
        Ok(())
    } else if *sym == None || *sym == Some(Word::Space) {
        m.print_mode = Some(PrintMode::Hex);
        Ok(())
    } else {
        Err(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParseError {
    InvalidUtf8(alloc::string::FromUtf8Error),
    SyntaxError,
}

fn fill_input_buffer() -> Vec<u8> {
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
    input_buf
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CommandType {
    X,
    C,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PrivilegeMode {
    Machine,
    Supervisor,
    User,
    Current,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DataType {
    Basic(BasicType),
    Array(BasicType, usize),
    Instruction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BasicType {
    signed: bool,
    width: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PrintMode {
    Hex,
    Decimal,
}
