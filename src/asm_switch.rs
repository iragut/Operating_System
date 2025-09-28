use x86_64::structures::idt::InterruptStackFrame;
use crate::process::SCHEDULER;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuState {
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

pub unsafe fn do_switch(frame: &mut InterruptStackFrame) {
    let mut scheduler = SCHEDULER.lock();
    
    // Save current process state
    if let Some(current_pid) = scheduler.current_pid {
        if let Some(current) = scheduler.processes.get_mut(&current_pid) {
            // Save interrupt frame values
            current.cpu_state.rip = frame.instruction_pointer.as_u64();
            current.cpu_state.rsp = frame.stack_pointer.as_u64();
            current.cpu_state.rflags = frame.cpu_flags;
            current.cpu_state.cs = frame.code_segment as u64;
            current.cpu_state.ss = frame.stack_segment as u64;
            
            // Save general purpose registers
            core::arch::asm!(
                "mov {}, rax",
                "mov {}, rbx",
                "mov {}, rcx",
                "mov {}, rdx",
                out(reg) current.cpu_state.rax,
                out(reg) current.cpu_state.rbx,
                out(reg) current.cpu_state.rcx,
                out(reg) current.cpu_state.rdx,
            );
        }
    }
    
    // Get next process
    if let Some(next_pid) = scheduler.schedule() {
        if let Some(next) = scheduler.processes.get(&next_pid) {
            
            // Update the interrupt frame
            let frame_ptr = frame as *mut InterruptStackFrame;
            
            // Update stack frame
            core::ptr::write_volatile(
                frame_ptr as *mut u64,
                next.cpu_state.rip
            );
            core::ptr::write_volatile(
                (frame_ptr as *mut u64).offset(2),
                next.cpu_state.rflags
            );
            core::ptr::write_volatile(
                (frame_ptr as *mut u64).offset(3),
                next.cpu_state.rsp
            );
            
            // Restore general purpose registers
            core::arch::asm!(
                "mov rax, {}",
                "mov rbx, {}",
                "mov rcx, {}",
                "mov rdx, {}",
                in(reg) next.cpu_state.rax,
                in(reg) next.cpu_state.rbx,
                in(reg) next.cpu_state.rcx,
                in(reg) next.cpu_state.rdx,
            );
        }
    }
}

impl CpuState {
    pub fn new(rax: u64, rbx: u64, rcx: u64, rdx: u64,
               rsi: u64, rdi: u64, rbp: u64, rsp: u64,
               r8: u64, r9: u64, r10: u64, r11: u64,
               r12: u64, r13: u64, r14: u64, r15: u64,
               rip: u64, rflags: u64, cs: u64, ss: u64) -> Self {
        CpuState {
            rax, rbx, rcx, rdx,
            rsi, rdi, rbp, rsp,
            r8, r9, r10, r11,
            r12, r13, r14, r15,
            rip,
            rflags,
            cs,
            ss,
        }
    }

    pub fn default() -> Self {
        CpuState {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0, rsp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0,
            rflags: 0x202, // Interrupts enabled
            cs: 0x08, // Kernel code segment
            ss: 0x10, // Kernel data segment
        }
    }
}