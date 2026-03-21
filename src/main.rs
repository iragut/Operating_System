#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use game_os::{println};
use bootloader::{BootInfo, entry_point};
use game_os::memory::{self, BootInfoFrameAllocator};
use game_os::scheduler::SCHEDULER;
use game_os::allocator;
use x86_64::VirtAddr;

extern crate alloc;

entry_point!(kernel_main);

extern "C" fn test_process() {
    loop {
        // This process just runs forever
        // If context switching works, the kernel
        // will keep switching between process 0 and this one
    }
}

fn init_processes() {    
    println!("Initializing process management...");
    
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler = unsafe { SCHEDULER.get() };
        scheduler.init_kernel_process();
        scheduler.create_process(test_process);
    });
    
}

fn heap_init(boot_info: &'static BootInfo) {
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };

    // Store globals
    unsafe {
        memory::PHYS_MEM_OFFSET = phys_mem_offset.as_u64();
        memory::FRAME_ALLOCATOR.init(frame_allocator);
    }

    let alloc = unsafe { memory::FRAME_ALLOCATOR.get() };
    allocator::init_heap(&mut mapper, alloc)
        .expect("heap initialization failed");

    println!("Heap initialized successfully!");
}

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello World{}", "!");
    game_os::init();

    heap_init(boot_info);
    init_processes();

    println!("It did not crash!");
    // as before
    #[cfg(test)]
    test_main();
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