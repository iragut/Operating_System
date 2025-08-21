use alloc::boxed::Box;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use alloc::collections::{BTreeMap, VecDeque};
use spin::Mutex;
use x86_64::registers::control::Cr3;
use lazy_static::lazy_static;
use crate::memory::allocate_kernel_stack;

lazy_static! {
    pub static ref SCHEDULER: Mutex<ProcessManager> = 
        Mutex::new(ProcessManager::new());
}

pub enum process_state {
    Ready,
    Running,
    Waiting,
    Terminated,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct cpu_state {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64, 
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    pub rip: u64,
    pub rflags: u64,
    
    pub cs: u64,
    pub ss: u64,
}

pub struct process_memory {
    pub page_table_addr: PhysAddr,
    
    code_start: VirtAddr,
    data_start: VirtAddr,
    heap_start: VirtAddr,
    stack_start: VirtAddr,

    pages_allocated: usize,
}

pub struct process_block {
    pub pid: u32,
    pub state: process_state,
    pub priority: u8,
    pub parent_pid: u32,
    pub cpu_state: cpu_state,
    pub memory: process_memory,
    pub kernel_stack: VirtAddr,
    pub time : u64,
}

pub struct ProcessManager {
    pub processes: BTreeMap<u32, Box<process_block>>,
    pub ready_queue: VecDeque<u32>,
    pub current_pid: Option<u32>,
    pub next_pid: u32
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
                if matches!(proc.state, process_state::Running) {
                    proc.state = process_state::Ready;
                    self.ready_queue.push_back(current);
                }
            }
        }
        
        // Get next from ready queue
        if let Some(next_pid) = self.ready_queue.pop_front() {
            if let Some(proc) = self.processes.get_mut(&next_pid) {
                proc.state = process_state::Running;
                self.current_pid = Some(next_pid);
                return Some(next_pid);
            }
        }
        
        self.current_pid  // Keep current if no other process
    }


     pub fn create_kernel_process(&mut self, entry_point: extern "C" fn()) -> u32 {
        let pid = self.next_pid;
        self.next_pid += 1;
        
        // Allocate kernel stack
        let kernel_stack = allocate_kernel_stack();
        
        let process = Box::new(process_block {
            pid,
            state: process_state::Ready,
            priority: 1,
            parent_pid: 0,
            cpu_state: cpu_state {
                rip: entry_point as u64,
                rsp: kernel_stack.as_u64() - 8,  // Top of stack minus 8
                rflags: 0x202,  // Interrupts enabled
                cs: 0x08,  // Kernel code segment
                ss: 0x10,  // Kernel data segment
                rax: 0, rbx: 0, rcx: 0, rdx: 0,
                rsi: 0, rdi: 0, rbp: 0,
                r8: 0, r9: 0, r10: 0, r11: 0,
                r12: 0, r13: 0, r14: 0, r15: 0,
            },
            memory: process_memory {
                page_table_addr: Cr3::read().0.start_address(),
                code_start: VirtAddr::new(entry_point as u64),
                data_start: VirtAddr::new(0),
                heap_start: VirtAddr::new(crate::allocator::HEAP_START as u64),
                stack_start: kernel_stack,
                pages_allocated: 0,
            },
            kernel_stack,
            time: 0,
        });
        
        self.processes.insert(pid, process);
        self.ready_queue.push_back(pid);
        pid
    }
    
    pub fn init_process_zero(&mut self) {
        // Current execution becomes process 0
        let process_zero = Box::new(process_block {
            pid: 0,
            state: process_state::Running,
            priority: 1,
            parent_pid: 0,
            cpu_state: cpu_state {
                // Will be filled on first context save
                rip: 0, rsp: 0, rflags: 0x202,
                cs: 0x08, ss: 0x10,
                rax: 0, rbx: 0, rcx: 0, rdx: 0,
                rsi: 0, rdi: 0, rbp: 0,
                r8: 0, r9: 0, r10: 0, r11: 0,
                r12: 0, r13: 0, r14: 0, r15: 0,
            },
            memory: process_memory {
                page_table_addr: Cr3::read().0.start_address(),
                code_start: VirtAddr::new(0x200000),
                data_start: VirtAddr::new(0x300000),  
                heap_start: VirtAddr::new(crate::allocator::HEAP_START as u64),
                stack_start: VirtAddr::new(0),  // Current stack
                pages_allocated: 0,
            },
            kernel_stack: VirtAddr::new(0),  // Will use current stack
            time: 0,
        });
        
        self.processes.insert(0, process_zero);
        self.current_pid = Some(0);
    }
}