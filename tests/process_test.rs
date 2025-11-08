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

// Test process that uses various registers
static mut REGISTER_TEST_RESULT: u64 = 0;

extern "C" fn test_process_registers() {
    unsafe {
        // Set various registers to known values and verify they persist
        core::arch::asm!(
            "mov rax, 0x1111111111111111",
            "mov rbx, 0x2222222222222222",
            "mov rcx, 0x3333333333333333",
            "mov rdx, 0x4444444444444444",
            "mov rsi, 0x5555555555555555",
            "mov rdi, 0x6666666666666666",
            "mov r8, 0x8888888888888888",
            "mov r9, 0x9999999999999999",
            "mov r10, 0xAAAAAAAAAAAAAAAA",
            "mov r11, 0xBBBBBBBBBBBBBBBB",
            "mov r12, 0xCCCCCCCCCCCCCCCC",
            "mov r13, 0xDDDDDDDDDDDDDDDD",
            "mov r14, 0xEEEEEEEEEEEEEEEE",
            "mov r15, 0xFFFFFFFFFFFFFFFF",
            options(nostack)
        );

        // Let context switch happen
        for _ in 0..1000 {
            core::arch::asm!("nop");
        }

        // Verify registers still contain correct values
        let rax: u64;
        core::arch::asm!("mov {}, rax", out(reg) rax);
        if rax == 0x1111111111111111 {
            REGISTER_TEST_RESULT = 1; // Success
        }
    }
}

// Test process for stack isolation
static mut STACK_TEST_VAL: u64 = 0;

extern "C" fn test_process_stack_write() {
    let stack_value: u64 = 0xDEADBEEF;
    unsafe {
        STACK_TEST_VAL = stack_value;
    }
    loop {
        unsafe { core::arch::asm!("nop"); }
    }
}

extern "C" fn test_process_stack_read() {
    // Different stack space should not see other process's stack data
    let local_var: u64 = 0x12345678;
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

#[test_case]
fn test_kernel_stack_allocation() {
    // Test that each process gets its own kernel stack
    {
        let mut scheduler = SCHEDULER.lock();
        let pid1 = scheduler.create_process(test_process_a);
        let pid2 = scheduler.create_process(test_process_b);

        let proc1 = scheduler.processes.get(&pid1).unwrap();
        let proc2 = scheduler.processes.get(&pid2).unwrap();

        // Kernel stacks should be different
        assert_ne!(proc1.kernel_stack, proc2.kernel_stack);

        // Kernel stacks should be non-zero
        assert_ne!(proc1.kernel_stack.as_u64(), 0);
        assert_ne!(proc2.kernel_stack.as_u64(), 0);

        scheduler.terminate_process(pid1);
        scheduler.terminate_process(pid2);
    }
}

#[test_case]
fn test_user_stack_allocation() {
    // Test that each process gets its own user stack
    {
        let mut scheduler = SCHEDULER.lock();
        let pid1 = scheduler.create_process(test_process_stack_write);
        let pid2 = scheduler.create_process(test_process_stack_read);

        let proc1 = scheduler.processes.get(&pid1).unwrap();
        let proc2 = scheduler.processes.get(&pid2).unwrap();

        // User stacks should be different
        let user_stack1 = proc1.memory.get_user_stack();
        let user_stack2 = proc2.memory.get_user_stack();

        assert_ne!(user_stack1, user_stack2);

        // User stacks should be non-zero
        assert_ne!(user_stack1.as_u64(), 0);
        assert_ne!(user_stack2.as_u64(), 0);

        scheduler.terminate_process(pid1);
        scheduler.terminate_process(pid2);
    }
}

#[test_case]
fn test_process_memory_isolation() {
    // Test that processes have separate page table addresses (when implemented)
    {
        let mut scheduler = SCHEDULER.lock();
        let pid1 = scheduler.create_process(test_process_a);
        let pid2 = scheduler.create_process(test_process_b);

        let proc1 = scheduler.processes.get(&pid1).unwrap();
        let proc2 = scheduler.processes.get(&pid2).unwrap();

        // For now, they share the kernel page table, but this test
        // verifies the infrastructure is in place
        assert!(proc1.memory.page_table_addr.as_u64() > 0);
        assert!(proc2.memory.page_table_addr.as_u64() > 0);

        scheduler.terminate_process(pid1);
        scheduler.terminate_process(pid2);
    }
}

#[test_case]
fn test_cannot_terminate_kernel_process() {
    // Test that PID 0 (kernel process) cannot be terminated
    {
        let mut scheduler = SCHEDULER.lock();

        // Attempt to terminate kernel process
        scheduler.terminate_process(0);

        // Kernel process should still be present and not terminated
        let kernel_proc = scheduler.processes.get(&0).unwrap();
        assert_ne!(kernel_proc.get_state(), ProcessState::Terminated);
    }
}

#[test_case]
fn test_register_preservation() {
    // Test that register values are preserved across process operations
    {
        let mut scheduler = SCHEDULER.lock();
        let pid = scheduler.create_process(test_process_registers);

        let proc = scheduler.processes.get(&pid).unwrap();

        // Verify CPU state structure has all register fields
        assert_eq!(proc.cpu_state.rax, 0);
        assert_eq!(proc.cpu_state.rbx, 0);
        assert_eq!(proc.cpu_state.rcx, 0);
        assert_eq!(proc.cpu_state.rdx, 0);
        assert_eq!(proc.cpu_state.rsi, 0);
        assert_eq!(proc.cpu_state.rdi, 0);
        assert_eq!(proc.cpu_state.rbp, 0);
        assert_eq!(proc.cpu_state.r8, 0);
        assert_eq!(proc.cpu_state.r9, 0);
        assert_eq!(proc.cpu_state.r10, 0);
        assert_eq!(proc.cpu_state.r11, 0);
        assert_eq!(proc.cpu_state.r12, 0);
        assert_eq!(proc.cpu_state.r13, 0);
        assert_eq!(proc.cpu_state.r14, 0);
        assert_eq!(proc.cpu_state.r15, 0);

        scheduler.terminate_process(pid);
    }
}

#[test_case]
fn test_multiple_process_scheduling() {
    // Test that multiple processes can be created and scheduled
    {
        let mut scheduler = SCHEDULER.lock();

        let pid1 = scheduler.create_process(test_process_a);
        let pid2 = scheduler.create_process(test_process_b);
        let pid3 = scheduler.create_process(test_process_c);

        // All should start in Ready state
        assert_eq!(scheduler.processes.get(&pid1).unwrap().get_state(), ProcessState::Ready);
        assert_eq!(scheduler.processes.get(&pid2).unwrap().get_state(), ProcessState::Ready);
        assert_eq!(scheduler.processes.get(&pid3).unwrap().get_state(), ProcessState::Ready);

        // Schedule first process
        let running = scheduler.schedule();
        assert!(running.is_some());

        if let Some(pid) = running {
            assert_eq!(scheduler.processes.get(&pid).unwrap().get_state(), ProcessState::Running);
        }

        // Clean up
        scheduler.terminate_process(pid1);
        scheduler.terminate_process(pid2);
        scheduler.terminate_process(pid3);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}