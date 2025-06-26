use core::sync::atomic::AtomicUsize;
use alloc::{string::String, vec::Vec};
use alloc::collections::BTreeMap;
use x86_64::PhysAddr;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
}

pub fn create_process(name: String, parent_pid: Option<u64>, priority: u8) -> ProcessResult<u64> {
    PROCESS_TABLE.lock().create_process(name, parent_pid, priority)
}

pub fn get_process(pid: u64) -> ProcessResult<ProcessControlBlock> {
    PROCESS_TABLE.lock().get_process(pid)
}
pub fn set_process_state(pid: u64, state: ProcessState) -> ProcessResult<()> {
    PROCESS_TABLE.lock().set_process_state(pid, state)
}

pub fn kill_process(pid: u64) -> ProcessResult<()> {
    PROCESS_TABLE.lock().kill_process(pid)
}

pub fn get_active_process_count() -> usize {
    PROCESS_TABLE.lock().get_active_process_count()
}

pub fn get_total_process_count() -> usize {
    PROCESS_TABLE.lock().get_total_process_count()
}

pub fn get_running_processes() -> Vec<ProcessControlBlock> {
    PROCESS_TABLE.lock().get_running_processes()
}

pub fn list_all_processes() -> Vec<ProcessControlBlock> {
    PROCESS_TABLE.lock().list_all_processes()
}

pub fn get_running_process() -> Option<ProcessControlBlock> {
    PROCESS_TABLE.lock().get_running_process()
}

pub fn get_process_in_state(state: ProcessState) -> Vec<ProcessControlBlock> {
    PROCESS_TABLE.lock().get_process_in_state(state)
}

pub fn get_all_children(parent_pid: u64) -> Vec<ProcessControlBlock> {
    PROCESS_TABLE.lock().get_all_children(parent_pid)
}


const MAX_PROCESSES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    New,
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    ProcessNotFound,
    ProcessTableFull,
    InvalidPid,
    InvalidState,
    OutOfMemory,
}

pub type ProcessResult<T> = Result<T, ProcessError>;

pub struct ProcessTable {
    processes: Vec<Option<ProcessControlBlock>>,
    pid_to_index: BTreeMap<u64, usize>,
    running_process: Option<ProcessControlBlock>,
    next_pid: AtomicUsize,
    active_count: usize,
}

#[derive(Clone)]
pub struct ProcessControlBlock {
    pid: u64,
    parent_pid: Option<u64>, 
    name: String,
    
    state: ProcessState,
    exit_code: Option<i32>,
    
    page_table: Option<PhysAddr>,
    priority: u8,
    creation_time: u64,
}

impl ProcessTable {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            pid_to_index: BTreeMap::new(),
            next_pid: AtomicUsize::new(1), 
            running_process: None,
            active_count: 0,
        }
    }

    pub fn get_active_process_count(&self) -> usize {
        self.active_count
    }

    pub fn get_total_process_count(&self) -> usize {
        self.next_pid.load(core::sync::atomic::Ordering::SeqCst) - 1
    }

    pub fn create_process(&mut self, name: String, parent_pid: Option<u64>, priority: u8) -> ProcessResult<u64> {
        if self.active_count >= MAX_PROCESSES {
            return Err(ProcessError::ProcessTableFull);
        }

        if let Some(pid) = parent_pid {
            if !self.process_exists(pid) {
                return Err(ProcessError::InvalidPid);
            }
        }

        let pid = self.next_pid.fetch_add(1, core::sync::atomic::Ordering::SeqCst) as u64;
        let pcb = ProcessControlBlock::new(pid, name, parent_pid, priority);
        
        if let Some(index) = self.find_free_index() {
            self.processes[index] = Some(pcb);
            self.pid_to_index.insert(pid, index);
        } else {
            self.processes.push(Some(pcb));
            self.pid_to_index.insert(pid, self.processes.len() - 1);
        }
        self.active_count += 1;

        Ok(pid)
    }

    fn find_process_index(&self, pid: u64) -> ProcessResult<usize> {
        if let Some(&index) = self.pid_to_index.get(&pid) {
            if self.processes[index].is_some() {
                return Ok(index);
            }
        }
        Err(ProcessError::ProcessNotFound)
    }

    fn find_free_index(&self) -> Option<usize> {
        self.processes.iter().position(|pcb| pcb.is_none())
    }

    pub fn process_exists(&self, pid: u64) -> bool {
        self.pid_to_index.contains_key(&pid)
    }

    pub fn get_process(&self, pid: u64) -> ProcessResult<ProcessControlBlock> {
        let index = self.find_process_index(pid)?;
        Ok((*self.processes[index].as_ref().unwrap()).clone())
    }

    pub fn get_process_mut(&mut self, pid: u64) -> ProcessResult<&mut ProcessControlBlock> {
        let index = self.find_process_index(pid)?;
        Ok(self.processes[index].as_mut().unwrap())
    }

    pub fn set_process_state(&mut self, pid: u64, state: ProcessState) -> ProcessResult<()> {
        let index = self.find_process_index(pid)?;
        if ProcessState::Running == state {
            let  process = self.get_running_process();
            if process.is_some() {
                return Err(ProcessError::InvalidState);
            }
            self.running_process = Some(self.processes[index].as_ref().unwrap().clone());
        }

        if self.processes[index].as_mut().unwrap().state == ProcessState::Running && state != ProcessState::Running {
            self.running_process = None;
        }

        self.processes[index].as_mut().unwrap().state = state;
        Ok(())
    }

    pub fn kill_process(&mut self, pid: u64) -> ProcessResult<()> {
        let index = self.find_process_index(pid)?;
        if let Some(pcb) = self.processes[index].take() {
            if self.running_process.as_ref().map_or(false, |p| p.pid == pcb.pid) {
                self.running_process = None;
            }

            self.pid_to_index.remove(&pcb.pid);
            self.active_count -= 1;
            self.processes[index] = None;

            Ok(())
        } else {
            Err(ProcessError::ProcessNotFound)
        }
    }

    pub fn get_process_in_state(&self, state: ProcessState) -> Vec<ProcessControlBlock> {
        self.processes.iter()
            .filter_map(|pcb| pcb.as_ref())
            .filter(|pcb| pcb.state == state)
            .cloned()
            .collect()
    }

    pub fn get_running_process(&self) -> Option<ProcessControlBlock> {
        self.running_process.clone()
    }

    pub fn get_running_processes(&self) -> Vec<ProcessControlBlock> {
        self.get_process_in_state(ProcessState::Running)
            .into_iter()
            .filter(|pcb| pcb.is_active())
            .collect()
    }

    pub fn list_all_processes(&self) -> Vec<ProcessControlBlock> {
        self.processes
            .iter()
            .filter_map(|process_opt| process_opt.as_ref())
            .cloned()
            .collect()
    }

    pub fn get_all_children(&self, parent_pid: u64) -> Vec<ProcessControlBlock> {
        self.processes.iter()
            .filter_map(|pcb| pcb.as_ref())
            .filter(|pcb| pcb.is_child_of(parent_pid))
            .cloned()
            .collect()
    }



}

impl ProcessControlBlock {
    pub fn new(pid: u64, name: String, parent_pid: Option<u64>, prior: u8) -> Self {
        Self {
            pid,
            parent_pid,
            name,
            state: ProcessState::New,
            exit_code: None,
            page_table: None,
            priority: prior,
            creation_time: 0, 
        }
    }

    pub fn get_pid(&self) -> u64 {
        self.pid
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_state(&self) -> ProcessState {
        self.state
    }
    pub fn get_parent_pid(&self) -> Option<u64> {
        self.parent_pid
    }
    pub fn get_priority(&self) -> u8 {
        self.priority
    }
    pub fn get_creation_time(&self) -> u64 {
        self.creation_time
    }

    pub fn is_child_of(&self, parent_pid: u64) -> bool {
        if self.parent_pid.is_some() {
            self.parent_pid.unwrap() == parent_pid
        } else {
            false
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, ProcessState::Running | ProcessState::Ready)
    }
    
}