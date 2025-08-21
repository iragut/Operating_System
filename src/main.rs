#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use game_os::{println, print};
use bootloader::{BootInfo, entry_point};
use game_os::memory::{self, BootInfoFrameAllocator};
use alloc::{boxed::Box, vec, vec::Vec, rc::Rc};
use game_os::allocator;
use x86_64::VirtAddr;

extern crate alloc;

entry_point!(kernel_main);

extern "C" fn process_a() {
    let mut counter: u64 = 0;
    loop {
        counter += 1;
        if counter % 100000 == 0 {
            crate::println!("A{}", counter / 100000);
        }
        unsafe { core::arch::asm!("nop"); }
    }
}

extern "C" fn process_b() {
    let mut counter: u64 = 1000;
    loop {
        counter += 1;
        if counter % 100000 == 0 {
            crate::println!("B{}", counter / 100000);
        }
        unsafe { core::arch::asm!("nop"); }
    }
}

fn init_processes() {
    use game_os::process::SCHEDULER;
    
    println!("Initializing process management...");
    
    let mut scheduler = SCHEDULER.lock();
    scheduler.init_process_zero();
    let pid1 = scheduler.create_kernel_process(process_a);
    let pid2 = scheduler.create_kernel_process(process_b);
    
    println!("Created processes: 0 (kernel), {} (A), {} (B)", pid1, pid2);
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello World{}", "!");
    game_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // new
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    println!("Heap initialized successfully!");
    init_processes();


    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    game_os::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    game_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}