use core::alloc::Layout;

use crate::{assembly::{restore_context, save_context, setup_initial_stack}, memory::ProcessMemoryLayout};
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
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ProcessState {
    New,
    Ready,
    Waiting,
    Running,
    End
}

#[derive(Clone)]
pub struct ProcessTable {
    pub processes: Vec<ProcessControlBlock>,
    pub next_pid: u64,
    pub current_process: Option<u64>
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

    pub fn get_current_runnig_process(&mut self) -> Option<&mut ProcessControlBlock> {
        if let Some(pid) = self.current_process {
            self.get_process_mut(pid)
        } else {
            None
        }
    }

    pub fn get_current_running_process_pid(&self) -> Option<u64> {
        self.current_process
    }

    pub fn set_current_running_process(&mut self, pid: u64) -> Result<(), ProcessError> {
        if self.get_process(pid).is_some() {
            self.current_process = Some(pid);
            Ok(())
        } else {
            Err(ProcessError::InvalidPid)
        }
    }
    
    pub fn find_next_ready_process_index(&self) -> Option<usize> {
        self.processes
            .iter()
            .enumerate()
            .find(|(_, p)| p.state == ProcessState::Ready)
            .map(|(index, _)| index)
    }


}

impl ProcessControlBlock {
    pub fn new(pid: u64, parent_pid: Option<u64>, page_table: ProcessMemoryLayout, priority: u8, creation_time: u64, pointer_stack: u64) -> Self {
        Self {
            pid,
            parent_pid,
            state: ProcessState::New,
            page_table,
            priority,
            creation_time,
            pointer_stack
        }
    }

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
    let pcb = ProcessControlBlock::new(
        0,
        None,
        memory_layout,
        1,
        0,
        initial_stack_pointer
    );
    
    // Add to process table
    let mut table = PROCESS_TABLE.lock();
    let pid = table.add_process(pcb);
    
    if let Some(process) = table.get_process_mut(pid) {
        process.set_state(ProcessState::Ready);
    }
    
    Ok(pid)
}

pub fn switch_process(current_pid: u64, next_pid: u64) {
    let mut table = PROCESS_TABLE.lock();

    let next_stack = table.get_process(next_pid).unwrap().pointer_stack;

    let current_process = table.get_process_mut(current_pid).unwrap();
    current_process.pointer_stack = unsafe { save_context() };

    unsafe { restore_context(next_stack) }
}