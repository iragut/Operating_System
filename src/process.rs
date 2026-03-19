use crate::asm_switch::CpuState;
use x86_64::PhysAddr;
use x86_64::VirtAddr;

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
    pages_allocated: usize,
}

pub struct ProcessBlock {
    pub(crate) pid: u32,
    pub(crate) state: ProcessState,
    pub priority: u8,
    pub parent_pid: u32,
    pub saved_state: *mut CpuState,
    pub memory: ProcessMemory,
    pub kernel_stack: VirtAddr,
    pub time: u64,
}

unsafe impl Send for ProcessBlock {}

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

impl ProcessMemory {
    pub fn new(
        page_table_addr: PhysAddr,
        code_start: VirtAddr,
        data_start: VirtAddr,
        heap_start: VirtAddr,
        stack_start: VirtAddr,
    ) -> Self {
        ProcessMemory {
            page_table_addr,
            code_start,
            data_start,
            heap_start,
            stack_start,
            pages_allocated: 0,
        }
    }
}
