use core::arch::asm;

use x86_64::VirtAddr;

pub fn write_u64_to_memory(address: u64, value: u64) {
    // Cast address to pointer and write value
    let unsafe_ptr = address as *mut u64;
    unsafe { *unsafe_ptr = value };
}

pub fn setup_initial_stack(stack_base: VirtAddr, entry_point: VirtAddr) -> u64 {
    let mut current_stack = stack_base.as_u64();
    
    current_stack -= 8;
    write_u64_to_memory(current_stack, entry_point.as_u64());
    
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0); 
    current_stack -= 8; write_u64_to_memory(current_stack, 0); 
    current_stack -= 8; write_u64_to_memory(current_stack, stack_base.as_u64() - 16);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    current_stack -= 8; write_u64_to_memory(current_stack, 0);
    
    current_stack
}

pub unsafe fn save_context() -> u64 {
    let rsp: u64;
    asm!(
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
        
        "mov {}, rsp",
        out(reg) rsp,
    );
    rsp
}

pub unsafe fn restore_context(stack_pointer: u64) -> ! {
    asm!(
        "mov rsp, {}",
        
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
        "pop rax",
        
        "ret",
        in(reg) stack_pointer,
        options(noreturn)
    );
}