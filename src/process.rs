use alloc::boxed::Box;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use alloc::collections::{BTreeMap, VecDeque};
use spin::Mutex;
use x86_64::registers::control::Cr3;
use lazy_static::lazy_static;
use crate::asm_switch::CpuState;
use crate::memory::{allocate_kernel_stack, allocate_user_stack, free_process_memory};

lazy_static! {
    pub static ref SCHEDULER: Mutex<ProcessManager> = 
        Mutex::new(ProcessManager::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Waiting,
    Terminated,
}

pub struct ProcessMemory {
    pub page_table_addr: PhysAddr,

    code_start: VirtAddr,
    data_start: VirtAddr,
    heap_start: VirtAddr,
    stack_start: VirtAddr,
    user_stack: VirtAddr,

    pages_allocated: usize,
}

pub struct ProcessBlock {
    pid: u32,
    state: ProcessState,
    pub priority: u8,
    pub parent_pid: u32,
    pub cpu_state: CpuState,
    pub memory: ProcessMemory,
    pub kernel_stack: VirtAddr,
    pub time : u64,
}

pub struct ProcessManager {
    pub processes: BTreeMap<u32, Box<ProcessBlock>>,
    pub ready_queue: VecDeque<u32>,
    pub current_pid: Option<u32>,
    pub next_pid: u32
}

impl ProcessBlock {
    pub fn get_pid(&self) -> u32 {
        self.pid
    }
    pub fn get_state(&self) -> ProcessState {
        self.state
    }

    pub fn set_state(&mut self, state: ProcessState) {
        self.state = state;
    }
    
}

impl ProcessManager {
    pub fn new() -> Self {
        ProcessManager {
            processes: BTreeMap::new(),
            ready_queue: VecDeque::new(),
            current_pid: None,
            next_pid: 1,
        }
    }

    pub fn schedule(&mut self) -> Option<u32> {
        // Move current to ready queue if still running
        if let Some(current) = self.current_pid {
            if let Some(proc) = self.processes.get_mut(&current) {
                if matches!(proc.state, ProcessState::Running) {
                    proc.state = ProcessState::Ready;
                    self.ready_queue.push_back(current);
                }
            }
        }
        
        // Get next from ready queue
        if let Some(next_pid) = self.ready_queue.pop_front() {
            if let Some(proc) = self.processes.get_mut(&next_pid) {
                proc.state = ProcessState::Running;
                self.current_pid = Some(next_pid);
                return Some(next_pid);
            }
        }
        
        self.current_pid  // Keep current if no other process
    }


    pub fn create_process(&mut self, entry_point: extern "C" fn()) -> u32 {
        let pid = self.next_pid;
        self.next_pid += 1;

        // Allocate kernel stack and user stack
        let kernel_stack = allocate_kernel_stack();
        let user_stack = allocate_user_stack();

        // For now, we still use the kernel's page table
        // In a full implementation, we would create a new page table here
        // using create_process_page_table() from memory.rs
        let page_table = Cr3::read().0.start_address();

        let process = Box::new(ProcessBlock {
            pid,
            state: ProcessState::Ready,
            priority: 1,
            parent_pid: 0,
            cpu_state: CpuState::new(
                0, 0, 0, 0, 0, 0, 0,
                kernel_stack.as_u64(),
                0, 0, 0, 0, 0, 0, 0, 0,
                entry_point as u64,
                0x202,
                0x08,
                0x10
            ),
            memory: ProcessMemory::new(
                page_table,
                VirtAddr::new(entry_point as u64),
                VirtAddr::new(0),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                kernel_stack,
                user_stack
            ),
            kernel_stack,
            time: 0,
        });

        self.processes.insert(pid, process);
        self.ready_queue.push_back(pid);
        pid
    }
    
    pub fn init_kernel_process(&mut self) {
        let process_zero = Box::new(ProcessBlock {
            pid: 0,
            state: ProcessState::Running,
            priority: 1,
            parent_pid: 0,
            cpu_state: CpuState::default(),
            memory: ProcessMemory::new(
                Cr3::read().0.start_address(),
                VirtAddr::new(0x200000),
                VirtAddr::new(0x300000),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                VirtAddr::new(0),
                VirtAddr::new(0),
            ),
            kernel_stack: VirtAddr::new(0),
            time: 0,
        });

        self.processes.insert(0, process_zero);
        self.current_pid = Some(0);
    }

    pub fn terminate_process(&mut self, pid: u32) {
        // Don't allow terminating the kernel process (PID 0)
        if pid == 0 {
            return;
        }

        let process = self.processes.get_mut(&pid);

        if process.is_none() {
            return;
        }

        let process = process.unwrap();

        // Save stack addresses before we drop the process
        let kernel_stack = process.kernel_stack;
        let user_stack = process.memory.user_stack;

        // If terminating the current process, switch to next available
        if self.current_pid == Some(pid) {
            self.current_pid = self.ready_queue.front().copied();
        }

        // Set process state to terminated
        process.state = ProcessState::Terminated;

        // Remove from ready queue if present
        self.ready_queue.retain(|&p| p != pid);

        // Clear current_pid if it was this process and no other process available
        if self.current_pid == Some(pid) {
            self.current_pid = None;
        }

        // Cleanup memory
        unsafe {
            free_process_memory(kernel_stack, user_stack);
        }

        // Note: We keep the process in the processes map for now
        // to maintain process history. In a full implementation,
        // we would eventually remove it completely.
    }
}


impl ProcessMemory {
    pub fn new(page_table_addr: PhysAddr, code_start: VirtAddr, data_start: VirtAddr,
               heap_start: VirtAddr, stack_start: VirtAddr, user_stack: VirtAddr) -> Self {
        ProcessMemory {
            page_table_addr,
            code_start,
            data_start,
            heap_start,
            stack_start,
            user_stack,
            pages_allocated: 0,
        }
    }

    pub fn get_user_stack(&self) -> VirtAddr {
        self.user_stack
    }

    pub fn get_kernel_stack(&self) -> VirtAddr {
        self.stack_start
    }
}