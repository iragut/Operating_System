#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use game_os::allocator;
use game_os::shell::Shell;
use game_os::memory::{self, BootInfoFrameAllocator};
use x86_64::VirtAddr;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    game_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop {}
}

#[test_case]
fn test_shell_creation() {
    let shell = Shell::new();
    assert!(shell.buffer.is_empty());
    assert!(shell.history.is_empty());
}

#[test_case]
fn test_empty_buffer_backspace() {
    let mut shell = Shell::new();
    shell.handle_char('\x08');
    assert!(shell.buffer.is_empty());
}

#[test_case]
fn test_buffer_add_character() {
    let mut shell = Shell::new();
    shell.handle_char('h');
    assert_eq!(shell.buffer, "h");
}

#[test_case]
fn test_buffer_add_multiple_characters() {
    let mut shell = Shell::new();
    shell.handle_char('h');
    shell.handle_char('e');
    shell.handle_char('l');
    shell.handle_char('l');
    shell.handle_char('o');
    assert_eq!(shell.buffer, "hello");
}

#[test_case]
fn test_buffer_backspace() {
    let mut shell = Shell::new();
    shell.handle_char('h');
    shell.handle_char('i');
    shell.handle_char('\x08');
    assert_eq!(shell.buffer, "h");
}

#[test_case]
fn test_multiple_backspaces() {
    let mut shell = Shell::new();
    shell.handle_char('h');
    shell.handle_char('e');
    shell.handle_char('l');
    shell.handle_char('l');
    shell.handle_char('o');
    shell.handle_char('\x08');
    shell.handle_char('\x08');
    shell.handle_char('\x08');
    assert_eq!(shell.buffer, "he");
}

#[test_case]
fn test_backspace_beyond_empty() {
    let mut shell = Shell::new();
    shell.handle_char('a');
    shell.handle_char('\x08');
    shell.handle_char('\x08');
    shell.handle_char('\x08');
    assert!(shell.buffer.is_empty());
}

#[test_case]
fn test_buffer_special_characters() {
    let mut shell = Shell::new();
    shell.handle_char('!');
    shell.handle_char('@');
    shell.handle_char('#');
    shell.handle_char('$');
    assert_eq!(shell.buffer, "!@#$");
}

#[test_case]
fn test_buffer_numbers() {
    let mut shell = Shell::new();
    shell.handle_char('1');
    shell.handle_char('2');
    shell.handle_char('3');
    shell.handle_char('4');
    shell.handle_char('5');
    assert_eq!(shell.buffer, "12345");
}

#[test_case]
fn test_buffer_spaces() {
    let mut shell = Shell::new();
    shell.handle_char('h');
    shell.handle_char('i');
    shell.handle_char(' ');
    shell.handle_char('t');
    shell.handle_char('h');
    shell.handle_char('e');
    shell.handle_char('r');
    shell.handle_char('e');
    assert_eq!(shell.buffer, "hi there");
}

#[test_case]
fn test_long_input() {
    let mut shell = Shell::new();
    let long_string = "this is a very long command that tests buffer capacity";
    for c in long_string.chars() {
        shell.handle_char(c);
    }
    assert_eq!(shell.buffer, long_string);
}

#[test_case]
fn test_mixed_input_and_backspace() {
    let mut shell = Shell::new();
    shell.handle_char('t');
    shell.handle_char('e');
    shell.handle_char('s');
    shell.handle_char('t');
    shell.handle_char('\x08'); // remove 't'
    shell.handle_char('\x08'); // remove 's'
    shell.handle_char('s');
    shell.handle_char('t');
    shell.handle_char('i');
    shell.handle_char('n');
    shell.handle_char('g');
    assert_eq!(shell.buffer, "testing");
}

#[test_case]
fn test_buffer_ready_for_parsing() {
    let mut shell = Shell::new();
    shell.handle_char('e');
    shell.handle_char('c');
    shell.handle_char('h');
    shell.handle_char('o');
    shell.handle_char(' ');
    shell.handle_char('h');
    shell.handle_char('e');
    shell.handle_char('l');
    shell.handle_char('l');
    shell.handle_char('o');
    
    assert_eq!(shell.buffer, "echo hello");
    assert!(shell.buffer.contains(' '));
    assert!(shell.buffer.starts_with("echo"));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}