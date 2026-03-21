use crate::gdt::set_tss_rsp0;
use crate::scheduler::SCHEDULER;
use crate::serial_println;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuState {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl CpuState {
    pub fn default() -> Self {
        CpuState {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0, rsp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0,
            rflags: 0x202,
            cs: 0x08,
            ss: 0x10,
        }
    }
}

#[no_mangle]
pub extern "C" fn switch_context(current_state: *mut CpuState) -> *mut CpuState {
    static mut TICK_COUNT: u64 = 0;
    
    unsafe {
        TICK_COUNT += 1;
        
        if TICK_COUNT % 18 != 0 {
            return current_state;
        }

        
        let scheduler = SCHEDULER.get();

        
        if let Some(current_pid) = scheduler.current_pid {
            if let Some(process) = scheduler.processes.get_mut(&current_pid) {
                process.saved_state = current_state;
            }
        }
        
        if let Some(next_pid) = scheduler.schedule() {
            if let Some(next) = scheduler.processes.get(&next_pid) {
                let cr3_addr = next.memory.page_table_addr;                
                let (current_cr3, _) = x86_64::registers::control::Cr3::read();

                set_tss_rsp0(next.kernel_stack);
                
                if current_cr3.start_address() != cr3_addr {
                    core::arch::asm!(
                        "mov cr3, {}",
                        in(reg) cr3_addr.as_u64(),
                    );
                }
                return next.saved_state;
            }
        }
        
        current_state
    }
}

core::arch::global_asm!(
    ".global timer_interrupt_entry",
    "timer_interrupt_entry:",

    "push rax",
    "push rbx",
    "push rcx",
    "push rdx",
    "push rsi",
    "push rdi",
    "push rbp",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",

    "mov rdi, rsp",
    "sub rsp, 256",
    "and rsp, 0xFFFFFFFFFFFFFFF0",
    "call switch_context",
    "mov rsp, rax",

    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rbp",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",

    "mov al, 0x20",
    "out 0x20, al",

    "pop rax",
    "iretq",
);