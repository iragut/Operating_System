#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use game_os::allocator;
use game_os::memory::{self, BootInfoFrameAllocator};
use game_os::scheduler::SCHEDULER;
use game_os::process::ProcessState;
use x86_64::VirtAddr;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    game_os::init();
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
    
    test_main();
    loop {}
}

fn with_scheduler<F, R>(f: F) -> R
where
    F: FnOnce(&mut game_os::scheduler::ProcessManager) -> R,
{
    x86_64::instructions::interrupts::without_interrupts(|| {
        let scheduler = unsafe { SCHEDULER.get() };
        f(scheduler)
    })
}

extern "C" fn nop_process() {
    loop { unsafe { core::arch::asm!("nop"); } }
}

// Test 1: Basic creation
#[test_case]
fn test_create_two_processes() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let pid1 = s.create_process(nop_process);
        let pid2 = s.create_process(nop_process);

        assert_eq!(pid1, 1);
        assert_eq!(pid2, 2);
        assert_eq!(s.processes.len(), 3);
        assert_eq!(s.processes.get(&pid1).unwrap().get_state(), ProcessState::Ready);
        assert_eq!(s.processes.get(&pid2).unwrap().get_state(), ProcessState::Ready);
    });
}

// Test 2: Round robin order
#[test_case]
fn test_schedule_order() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let pid1 = s.create_process(nop_process);
        let pid2 = s.create_process(nop_process);
        let pid3 = s.create_process(nop_process);

        assert_eq!(s.schedule(), Some(pid1));
        assert_eq!(s.schedule(), Some(pid2));
        assert_eq!(s.schedule(), Some(pid3));
        assert_eq!(s.schedule(), Some(0));
        assert_eq!(s.schedule(), Some(pid1));
    });
}

// Test 3: Terminate removes from queue
#[test_case]
fn test_terminate_skips_dead() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let pid1 = s.create_process(nop_process);
        let pid2 = s.create_process(nop_process);
        let pid3 = s.create_process(nop_process);

        s.terminate_process(pid2);
        assert_eq!(s.schedule(), Some(pid1));
        assert_eq!(s.schedule(), Some(pid3));
        assert_eq!(s.schedule(), Some(0));
        assert_eq!(s.schedule(), Some(pid1));
    });
}

// Test 4: Terminate running process
#[test_case]
fn test_terminate_current() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let pid1 = s.create_process(nop_process);
        let pid2 = s.create_process(nop_process);

        s.schedule();
        assert_eq!(s.current_pid, Some(pid1));

        s.terminate_process(pid1);
        assert_eq!(s.processes.get(&pid1).unwrap().get_state(), ProcessState::Terminated);

        assert_eq!(s.schedule(), Some(pid2));
    });
}

// Test 5: Ten processes round robin
#[test_case]
fn test_ten_processes() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let mut pids = [0u32; 10];
        for i in 0..10 {
            pids[i] = s.create_process(nop_process);
        }

        for i in 0..10 {
            assert_eq!(s.schedule(), Some(pids[i]));
        }
        assert_eq!(s.schedule(), Some(0));
        assert_eq!(s.schedule(), Some(pids[0]));

        for pid in pids.iter() {
            s.terminate_process(*pid);
        }
    });
}

// Test 6: Initial saved state is correct
#[test_case]
fn test_initial_cpu_state() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let pid = s.create_process(nop_process);
        let process = s.processes.get(&pid).unwrap();

        assert!(!process.saved_state.is_null());
        let state = unsafe { &*process.saved_state };
        assert_eq!(state.cs, 0x08);
        assert_eq!(state.ss, 0x10);
        assert_eq!(state.rflags, 0x202);
        assert!(state.rip > 0);
    });
}

// Test 7: Kernel process starts as running with null state
#[test_case]
fn test_kernel_process() {
    with_scheduler(|s| {
        s.reset();
        s.init_kernel_process();

        let p = s.processes.get(&0).unwrap();
        assert!(p.saved_state.is_null());
        assert_eq!(p.get_state(), ProcessState::Running);
        assert_eq!(s.current_pid, Some(0));
    });
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}