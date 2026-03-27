use crate::print;
use crate::asm_switch::CpuState;
use crate::scheduler::SCHEDULER;
use crate::input::INPUT;

core::arch::global_asm!(
    ".global syscall_interrupt_entry",
    "syscall_interrupt_entry:",


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
    "call syscall_dispatch",
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

    "pop rax",
    "iretq",
);

fn sys_read(state: &mut CpuState) -> *mut CpuState {
    let input = unsafe { INPUT.get() };
    let buffer = state.rdi as *mut u8;
    let length = state.rsi as usize;
    let mut count = 0;

    while count < length {
        if let Some(byte) = input.pop() {
            unsafe { *buffer.add(count) = byte; }
            count += 1;
        } else {
            break;
        }
    }

    state.rax = count as u64;
    state as *mut CpuState
}

fn sys_write(state: &mut CpuState) -> *mut CpuState {
    let length = state.rsi as usize;
    let buffer = state.rdi as *const u8;

    let slice = unsafe { core::slice::from_raw_parts(buffer, length) };
    if let Ok(s) = core::str::from_utf8(slice) {
        crate::print!("{}", s);
    }

    state.rax = 0;
    state
}

fn sys_exit(state: &mut CpuState) -> *mut CpuState {
    unsafe {
        let scheduler = SCHEDULER.get();
        let pid = scheduler.current_pid.unwrap();
        scheduler.terminate_process(pid);

        crate::asm_switch::switch_to_next(core::ptr::null_mut())
    }
}


#[no_mangle]
pub extern "C" fn syscall_dispatch(current_state: *mut CpuState) -> *mut CpuState {
    let state: &mut CpuState = unsafe { &mut *current_state };

    match state.rax {
        0 => sys_read(state),
        1 => sys_write(state),
        60 => sys_exit(state),
        _ => {
            state.rax = u64::MAX;
            state as *mut CpuState
        }
    }
}