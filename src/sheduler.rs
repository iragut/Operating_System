use crate::process::{ProcessState, ProcessTable, PROCESS_TABLE};


pub fn primitive_scheduler() -> Option<u64> {
    let mut table = PROCESS_TABLE.lock();

    let current_pid = table.current_process;
    let next_index = table.find_next_ready_process_index();

    match (current_pid, next_index) {
        (Some(curr_pid), Some(next_idx)) => {
            
            if let Some(current_process) = table.get_process_mut(curr_pid) {
                current_process.set_state(ProcessState::Ready);
            }
            
            let next_pid = table.processes[next_idx].pid;
            table.processes[next_idx].set_state(ProcessState::Running);
            table.current_process = Some(next_pid);
            
            Some(next_pid)
        }
        (None, Some(next_idx)) => {
            let next_pid = table.processes[next_idx].pid;
            table.processes[next_idx].set_state(ProcessState::Running);
            table.current_process = Some(next_pid);
            
            Some(next_pid)
        }
        _ => None, 
    }
}