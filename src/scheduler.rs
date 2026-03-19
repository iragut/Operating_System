use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use core::cell::UnsafeCell;
use x86_64::registers::control::Cr3;
use x86_64::VirtAddr;

use crate::asm_switch::CpuState;
use crate::memory::allocate_kernel_stack;
use crate::process::{ProcessBlock, ProcessMemory, ProcessState};


pub struct SchedulerCell(UnsafeCell<ProcessManager>);

unsafe impl Sync for SchedulerCell {}
unsafe impl Send for SchedulerCell {}

impl SchedulerCell {
    const fn new(pm: ProcessManager) -> Self {
        SchedulerCell(UnsafeCell::new(pm))
    }

    // Caller must ensure interrupts are disabled.
    pub unsafe fn get(&self) -> &mut ProcessManager {
        &mut *self.0.get()
    }
}

pub static SCHEDULER: SchedulerCell = SchedulerCell::new(ProcessManager::new());

pub struct ProcessManager {
    pub processes: BTreeMap<u32, Box<ProcessBlock>>,
    pub ready_queue: VecDeque<u32>,
    pub current_pid: Option<u32>,
    pub next_pid: u32,
}

impl ProcessManager {
    pub const fn new() -> Self {
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

        self.current_pid
    }

    pub fn create_process(&mut self, entry_point: extern "C" fn()) -> u32 {
        let pid = self.next_pid;
        self.next_pid += 1;

        let kernel_stack = allocate_kernel_stack();
        let stack_top = kernel_stack.as_u64();
        let state_ptr = (stack_top - core::mem::size_of::<CpuState>() as u64) as *mut CpuState;

        unsafe {
            *state_ptr = CpuState {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rbp: 0,
                rdi: 0,
                rsi: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
                rip: entry_point as u64,
                cs: 0x08,
                rflags: 0x202,
                rsp: stack_top,
                ss: 0x10,
            };
        }

        

        let process = Box::new(ProcessBlock {
            pid,
            state: ProcessState::Ready,
            priority: 1,
            parent_pid: 0,
            saved_state: state_ptr,
            memory: ProcessMemory::new(
                Cr3::read().0.start_address(),
                VirtAddr::new(entry_point as u64),
                VirtAddr::new(0),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                kernel_stack,
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
            saved_state: core::ptr::null_mut(),
            memory: ProcessMemory::new(
                Cr3::read().0.start_address(),
                VirtAddr::new(0x200000),
                VirtAddr::new(0x300000),
                VirtAddr::new(crate::allocator::HEAP_START as u64),
                VirtAddr::new(0),
            ),
            kernel_stack: VirtAddr::new(0),
            time: 0,
        });

        self.processes.insert(0, process_zero);
        self.current_pid = Some(0);
    }

    pub fn terminate_process(&mut self, pid: u32) {
        let process = self.processes.get_mut(&pid);

        if process.is_none() {
            return;
        }

        let process = process.unwrap();

        if self.current_pid == Some(pid) {
            self.current_pid = self.ready_queue.front().copied();
        }

        process.state = ProcessState::Terminated;
        self.ready_queue.retain(|&p| p != pid);

        if self.current_pid == Some(pid) {
            self.current_pid = None;
        }
    }

    // Function to use for testing
    pub fn reset(&mut self) {
        self.processes.clear();
        self.ready_queue.clear();
        self.current_pid = None;
        self.next_pid = 1;
    }
}
