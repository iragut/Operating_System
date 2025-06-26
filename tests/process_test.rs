#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(game_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::string::ToString;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use game_os::allocator;
use game_os::process::*;
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
fn test_process_creation() {
    let init_pid = create_process("init".to_string(), None, 0)
        .expect("Failed to create init process");
    
    assert!(init_pid > 0);
    assert_eq!(get_active_process_count(), 1);
}

#[test_case]
fn test_pid_uniqueness() {
    let pid1 = create_process("proc1".to_string(), None, 0)
        .expect("Failed to create process 1");
    let pid2 = create_process("proc2".to_string(), None, 0)
        .expect("Failed to create process 2");
    let pid3 = create_process("proc3".to_string(), None, 0)
        .expect("Failed to create process 3");
    
    assert_ne!(pid1, pid2);
    assert_ne!(pid2, pid3);
    assert_ne!(pid1, pid3);
    
    // Clean up
    kill_process(pid1).unwrap();
    kill_process(pid2).unwrap();
    kill_process(pid3).unwrap();
}

#[test_case]
fn test_process_lookup() {
    let pid = create_process("test_lookup".to_string(), None, 1)
        .expect("Failed to create test process");
    
    let process = get_process(pid).expect("Failed to find process");
    assert_eq!(process.get_pid(), pid);
    assert_eq!(process.get_name(), "test_lookup");
    assert_eq!(process.get_priority(), 1);
    assert_eq!(process.get_parent_pid(), None);
    
    kill_process(pid).unwrap();
}

#[test_case]
fn test_process_not_found() {
    let result = get_process(999);
    assert!(result.is_err());
    
    match result {
        Err(ProcessError::ProcessNotFound) => {},
        _ => panic!("Expected ProcessNotFound error"),
    }
}

#[test_case]
fn test_parent_child_relationship() {
    let parent_pid = create_process("parent".to_string(), None, 0)
        .expect("Failed to create parent");
    
    let child_pid = create_process("child".to_string(), Some(parent_pid), 1)
        .expect("Failed to create child");
    
    let child = get_process(child_pid).expect("Failed to get child");
    assert_eq!(child.get_parent_pid(), Some(parent_pid));
    assert!(child.is_child_of(parent_pid));
    
    let children = get_all_children(parent_pid);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].get_pid(), child_pid);
    
    // Clean up
    kill_process(child_pid).unwrap();
    kill_process(parent_pid).unwrap();
}

#[test_case]
fn test_invalid_parent() {
    let result = create_process("orphan".to_string(), Some(999), 0);
    assert!(result.is_err());
    
    match result {
        Err(ProcessError::InvalidPid) => {},
        _ => panic!("Expected InvalidPid error"),
    }
}

#[test_case]
fn test_process_state_transitions() {
    let pid = create_process("state_test".to_string(), None, 0)
        .expect("Failed to create process");
    
    // Initial state should be New
    let process = get_process(pid).expect("Failed to get process");
    assert_eq!(process.get_state(), ProcessState::New);
    
    // Transition to Ready
    set_process_state(pid, ProcessState::Ready).expect("Failed to set Ready state");
    let process = get_process(pid).expect("Failed to get process");
    assert_eq!(process.get_state(), ProcessState::Ready);
    
    // Transition to Running
    set_process_state(pid, ProcessState::Running).expect("Failed to set Running state");
    let process = get_process(pid).expect("Failed to get process");
    assert_eq!(process.get_state(), ProcessState::Running);
    
    kill_process(pid).unwrap();
}

#[test_case]
fn test_single_running_process_constraint() {
    let pid1 = create_process("runner1".to_string(), None, 0)
        .expect("Failed to create process 1");
    let pid2 = create_process("runner2".to_string(), None, 0)
        .expect("Failed to create process 2");
    
    // Set first process to running
    set_process_state(pid1, ProcessState::Running).expect("Failed to set first running");
    
    // Try to set second process to running - should fail
    let result = set_process_state(pid2, ProcessState::Running);
    assert!(result.is_err());
    
    match result {
        Err(ProcessError::InvalidState) => {},
        _ => panic!("Expected InvalidState error"),
    }
    
    // Verify first is still running
    let running = get_running_process().expect("No running process found");
    assert_eq!(running.get_pid(), pid1);
    
    // Clean up
    kill_process(pid1).unwrap();
    kill_process(pid2).unwrap();
}

#[test_case]
fn test_running_process_tracking() {
    let pid = create_process("tracker".to_string(), None, 0)
        .expect("Failed to create process");
    
    // Initially no running process
    assert!(get_running_process().is_none());
    
    // Set to running
    set_process_state(pid, ProcessState::Running).expect("Failed to set running");
    let running = get_running_process().expect("No running process found");
    assert_eq!(running.get_pid(), pid);
    
    // Change to ready
    set_process_state(pid, ProcessState::Ready).expect("Failed to set ready");
    assert!(get_running_process().is_none());
    
    // Clean up
    kill_process(pid).unwrap();
}

#[test_case]
fn test_process_listing() {
    let initial_count = list_all_processes().len();
    
    let pid1 = create_process("list1".to_string(), None, 0)
        .expect("Failed to create process 1");
    let pid2 = create_process("list2".to_string(), None, 1)
        .expect("Failed to create process 2");
    
    let processes = list_all_processes();
    assert_eq!(processes.len(), initial_count + 2);
    
    // Find our processes in the list
    let found_pids: alloc::vec::Vec<u64> = processes.iter().map(|p| p.get_pid()).collect();
    assert!(found_pids.contains(&pid1));
    assert!(found_pids.contains(&pid2));
    
    // Clean up
    kill_process(pid1).unwrap();
    kill_process(pid2).unwrap();
}

#[test_case]
fn test_processes_by_state() {
    let pid1 = create_process("new1".to_string(), None, 0)
        .expect("Failed to create process 1");
    let pid2 = create_process("new2".to_string(), None, 0)
        .expect("Failed to create process 2");
    
    // Both should be in New state
    let new_processes = get_process_in_state(ProcessState::New);
    let new_pids: alloc::vec::Vec<u64> = new_processes.iter().map(|p| p.get_pid()).collect();
    assert!(new_pids.contains(&pid1));
    assert!(new_pids.contains(&pid2));
    
    // Set one to Ready
    set_process_state(pid1, ProcessState::Ready).expect("Failed to set ready");
    
    // Check state filtering
    let ready_processes = get_process_in_state(ProcessState::Ready);
    assert_eq!(ready_processes.len(), 1);
    assert_eq!(ready_processes[0].get_pid(), pid1);
    
    let new_processes = get_process_in_state(ProcessState::New);
    let new_pids: alloc::vec::Vec<u64> = new_processes.iter().map(|p| p.get_pid()).collect();
    assert!(new_pids.contains(&pid2));
    assert!(!new_pids.contains(&pid1));
    
    // Clean up
    kill_process(pid1).unwrap();
    kill_process(pid2).unwrap();
}

#[test_case]
fn test_process_cleanup() {
    let initial_count = get_active_process_count();
    
    let pid = create_process("cleanup_test".to_string(), None, 0)
        .expect("Failed to create process");
    
    assert_eq!(get_active_process_count(), initial_count + 1);
    
    // Kill the process
    kill_process(pid).expect("Failed to kill process");
    
    assert_eq!(get_active_process_count(), initial_count);
    assert!(get_process(pid).is_err());
}

#[test_case]
fn test_kill_running_process() {
    let pid = create_process("kill_runner".to_string(), None, 0)
        .expect("Failed to create process");
    
    // Set to running
    set_process_state(pid, ProcessState::Running).expect("Failed to set running");
    assert!(get_running_process().is_some());
    
    // Kill it
    kill_process(pid).expect("Failed to kill running process");
    
    assert!(get_running_process().is_none());
}

#[test_case]
fn test_process_active_status() {
    let pid = create_process("active_test".to_string(), None, 0)
        .expect("Failed to create process");
    
    let process = get_process(pid).expect("Failed to get process");
    assert!(!process.is_active());
    
    // Set to Ready (active)
    set_process_state(pid, ProcessState::Ready).expect("Failed to set ready");
    let process = get_process(pid).expect("Failed to get process");
    assert!(process.is_active());
    
    // Set to Running (active)
    set_process_state(pid, ProcessState::Running).expect("Failed to set running");
    let process = get_process(pid).expect("Failed to get process");
    assert!(process.is_active());
    
    // Set to Blocked (not active)
    set_process_state(pid, ProcessState::Blocked).expect("Failed to set blocked");
    let process = get_process(pid).expect("Failed to get process");
    assert!(!process.is_active());
    
    // Clean up
    kill_process(pid).unwrap();
}

#[test_case]
fn test_process_counts() {
    let initial_active = get_active_process_count();
    let initial_total = get_total_process_count();
    
    let pid1 = create_process("count1".to_string(), None, 0)
        .expect("Failed to create process 1");
    let pid2 = create_process("count2".to_string(), None, 0)
        .expect("Failed to create process 2");
    
    assert_eq!(get_active_process_count(), initial_active + 2);
    assert_eq!(get_total_process_count(), initial_total + 2);
    
    // Kill one process
    kill_process(pid1).expect("Failed to kill process");
    
    assert_eq!(get_active_process_count(), initial_active + 1);
    assert_eq!(get_total_process_count(), initial_total + 2);
    

    kill_process(pid2).unwrap();
}

#[test_case]
fn test_complex_parent_child_tree() {
    // Create init
    let init_pid = create_process("init".to_string(), None, 0)
        .expect("Failed to create init");
    
    // Create children of init
    let shell_pid = create_process("shell".to_string(), Some(init_pid), 1)
        .expect("Failed to create shell");
    let daemon_pid = create_process("daemon".to_string(), Some(init_pid), 2)
        .expect("Failed to create daemon");
    
    // Create grandchild
    let app_pid = create_process("app".to_string(), Some(shell_pid), 3)
        .expect("Failed to create app");
    
    // Test relationships
    let init_children = get_all_children(init_pid);
    assert_eq!(init_children.len(), 2);
    
    let shell_children = get_all_children(shell_pid);
    assert_eq!(shell_children.len(), 1);
    assert_eq!(shell_children[0].get_pid(), app_pid);
    
    let daemon_children = get_all_children(daemon_pid);
    assert_eq!(daemon_children.len(), 0);
    
    // Clean up 
    kill_process(app_pid).unwrap();
    kill_process(daemon_pid).unwrap();
    kill_process(shell_pid).unwrap();
    kill_process(init_pid).unwrap();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    game_os::test_panic_handler(info)
}