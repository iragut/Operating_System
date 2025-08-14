use core::alloc::Layout;

use crate::{assembly::setup_initial_stack, memory::ProcessMemoryLayout};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::VirtAddr;

const STACK_SIZE: usize = 8 * 1024;
const STACK_ALIGNMENT: usize = 16;


lazy_static! {
    pub static ref PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
}

#[derive(Debug)]
pub enum ProcessError {
    OutOfMemory,
    InvalidPid,
    ProcessTableFull,
    InvalidMemoryLayout,
}

#[derive(Clone, Copy)]
enum ProcessState {
    New,
    Ready,
    Waiting,
    Running,
    End
}

#[derive(Clone)]
pub struct ProcessTable {
    processes: Vec<ProcessControlBlock>,
    next_pid: u64,
    current_process: Option<u64>
}

#[derive(Clone, Copy)]
pub struct ProcessControlBlock {
    pub pid: u64,
    pub parent_pid: Option<u64>,
    
    pub state: ProcessState,    
    pub page_table: ProcessMemoryLayout,
    pub priority: u8,
    pub creation_time: u64,
    pub pointer_stack: u64,
}

impl ProcessTable {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            next_pid: 1,
            current_process: None,
        }
    }
    
    pub fn add_process(&mut self, mut pcb: ProcessControlBlock) -> u64 {
        pcb.pid = self.next_pid;
        self.next_pid += 1;
        self.processes.push(pcb);
        pcb.pid
    }
    
    pub fn get_process(&self, pid: u64) -> Option<&ProcessControlBlock> {
        self.processes.iter().find(|p| p.pid == pid)
    }
    
    pub fn get_process_mut(&mut self, pid: u64) -> Option<&mut ProcessControlBlock> {
        self.processes.iter_mut().find(|p| p.pid == pid)
    }
    
    pub fn find_next_ready_process(&self) -> Option<usize> {
        self.processes.iter()
            .position(|p| matches!(p.state, ProcessState::Ready))
    }
}

impl ProcessControlBlock {
    pub fn set_state(&mut self, state: ProcessState){
        self.state = state;
    }
    
}

pub fn allocate_process_stack() -> Result<VirtAddr, ProcessError> {
    let layout = Layout::from_size_align(STACK_SIZE, 16).map_err(|_| ProcessError::OutOfMemory)?;
    
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    
    if ptr.is_null() {
        return Err(ProcessError::OutOfMemory);
    }
    
    // Convert to VirtAddr
    let stack_base = VirtAddr::from_ptr(ptr) + STACK_SIZE;
    return Ok(stack_base)
}

pub fn create_process(entry_point: VirtAddr) -> Result<u64, ProcessError> {
    // Create memory layout
    let memory_layout = ProcessMemoryLayout::create_simple_layout(entry_point)
        .map_err(|_| ProcessError::InvalidMemoryLayout)?;

    let stack_base = allocate_process_stack()?;
    let initial_stack_pointer = setup_initial_stack(stack_base, entry_point);
    
    // Create PCB
    let pcb = ProcessControlBlock {
        pid: 0,
        parent_pid: None,
        state: ProcessState::New,
        page_table: memory_layout,
        priority: 100,
        creation_time: 0, // TODO: Get current time
        pointer_stack: initial_stack_pointer
    };
    
    // Add to process table
    let mut table = PROCESS_TABLE.lock();
    let pid = table.add_process(pcb);
    
    if let Some(process) = table.get_process_mut(pid) {
        process.set_state(ProcessState::Ready);
    }
    
    Ok(pid)
}