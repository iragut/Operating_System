pub fn write(buf: &[u8]) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "mov rax, 1",
            "int 0x80",
            in("rdi") buf.as_ptr(),
            in("rsi") buf.len(),
            lateout("rax") result,
        );
    }
    result
}

pub fn read(buf: &mut [u8]) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "mov rax, 0",
            "int 0x80",
            in("rdi") buf.as_mut_ptr(),
            in("rsi") buf.len(),
            lateout("rax") result,
        );
    }
    result
}

pub fn exit() -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, 2",
            "int 0x80",
            options(noreturn),
        );
    }
}