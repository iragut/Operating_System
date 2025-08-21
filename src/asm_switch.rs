// New asm_switch.rs - simpler and safer
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::PhysFrame;
use crate::interrupts::{PICS, InterruptIndex};
use crate::process::SCHEDULER;
use x86_64::VirtAddr;

pub extern "x86-interrupt" fn timer_interrupt_handler(mut stack_frame: InterruptStackFrame) {
    static mut TICK_COUNT: u64 = 0;
    
    unsafe {
        TICK_COUNT += 1;
        
        // Only switch every 18 ticks
        if TICK_COUNT % 18 != 0 {
            PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
            return;
        }
        
        crate::print!("[{}]", TICK_COUNT / 18);
        
        // Simple context switch without register saving
        do_switch(&mut stack_frame);
        
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

unsafe fn do_switch(frame: &mut InterruptStackFrame) {
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
            
            // Save general purpose registers using inline assembly
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
            crate::print!(" -> P{}", next_pid);
            
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