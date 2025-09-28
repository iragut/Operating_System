#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use game_os::{allocator, serial_print};
use game_os::memory::{self, BootInfoFrameAllocator};
use x86_64::VirtAddr;
use game_os::process::SCHEDULER;
use game_os::process::ProcessState;

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

// Test process functions
static mut COUNTER_A: u64 = 0;
static mut COUNTER_B: u64 = 0;

extern "C" fn test_process_a() {
    loop {
        unsafe { 
            COUNTER_A += 1;
            if COUNTER_A == 100 {
                game_os::serial_print!("A");  // Use serial for tests
            }
        }
        unsafe { core::arch::asm!("nop"); }
    }
}

extern "C" fn test_process_b() {
    loop {
        unsafe { 
            COUNTER_B += 1;
            if COUNTER_B == 100 {
                game_os::serial_print!("B");  // Use serial for tests
            }
        }
        unsafe { core::arch::asm!("nop"); }
    }
}

extern "C" fn test_process_c() {
    loop {
        unsafe { core::arch::asm!("nop"); }
    }
}

#[test_case]
fn test_process_creation() {
    // Initialize process system
    {
        let mut scheduler = SCHEDULER.lock();
        scheduler.init_kernel_process();
        
        let pid1 = scheduler.create_process(test_process_a);
        let pid2 = scheduler.create_process(test_process_b);
        
        assert_eq!(pid1, 1);
        assert_eq!(pid2, 2);
        assert_eq!(scheduler.processes.len(), 3);  // 0, 1, 2

        scheduler.terminate_process(pid1);
        scheduler.terminate_process(pid2);
    }    
}

#[test_case]
fn test_process_termination_simple() {
    {
        let mut scheduler = SCHEDULER.lock();
        let pid = scheduler.create_process(test_process_c);
        assert_eq!(scheduler.processes.get(&pid).unwrap().get_state(), ProcessState::Ready);
        
        scheduler.terminate_process(pid);
        assert!(scheduler.processes.get(&pid).unwrap().get_state() == ProcessState::Terminated);
    }
}

#[test_case]
fn test_process_termination_running() {
    {
        let mut scheduler = SCHEDULER.lock();
        let pid = scheduler.create_process(test_process_a);
        assert_eq!(scheduler.processes.get(&pid).unwrap().get_state(), ProcessState::Ready);
        
        scheduler.terminate_process(pid);
        assert!(scheduler.processes.get(&pid).unwrap().get_state() == ProcessState::Terminated);
        assert!(scheduler.current_pid.is_none() || scheduler.current_pid == Some(0));
    }
}

#[test_case]
fn test_process_terminate_complex() {
    {
        let mut scheduler = SCHEDULER.lock();
        let pid1 = scheduler.create_process(test_process_a);
        let pid2 = scheduler.create_process(test_process_b);
        let pid3 = scheduler.create_process(test_process_c);
        
        scheduler.schedule();
        assert_eq!(scheduler.current_pid, Some(pid1));
        assert_eq!(scheduler.processes.get(&pid1).unwrap().get_state(), ProcessState::Running);
    
        scheduler.terminate_process(pid1);
        assert!(scheduler.processes.get(&pid1).unwrap().get_state() == ProcessState::Terminated);
        assert!(scheduler.current_pid.is_none() || scheduler.current_pid == Some(4));
        
        let next_pid = scheduler.schedule();
        assert!(next_pid == Some(pid2) || next_pid == Some(pid3));
        if let Some(npid) = next_pid {
            assert_eq!(scheduler.processes.get(&npid).unwrap().get_state(), ProcessState::Running);
        }
        scheduler.terminate_process(pid2);
        scheduler.terminate_process(pid3);

    }

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}