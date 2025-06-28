#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use game_os::shell::Shell;
use game_os::{println};
use bootloader::{BootInfo, entry_point};
use game_os::memory::{self,  BootInfoFrameAllocator};
use game_os::allocator;
use x86_64::VirtAddr;
extern crate alloc;

entry_point!(kernel_main);

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

    memory::init_global_memory_management(phys_mem_offset, frame_allocator);

    let test_page_table = memory::create_physical_addr()
        .expect("Failed to create page table");
        
    let test_layout = memory::setup_standard_process_layout(test_page_table, 999)
        .expect("Failed to setup layout");
        
    println!("Created page table at: {:?}", test_page_table);
    println!("Code region: {:?} - {:?}", 
        test_layout.code_start(), 
        test_layout.code_start() + 0x0010_0000u64);

    let mut _shell = Shell::new();

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