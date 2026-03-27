#![no_std]
#![no_main]

use core::panic::PanicInfo;

fn syscall(number: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") number,
            in("rdi") arg1,
            in("rsi") arg2,
            lateout("rax") result,
        );
    }
    result
}

fn write(buf: &[u8]) -> u64 {
    syscall(1, buf.as_ptr() as u64, buf.len() as u64)
}

fn read(buf: &mut [u8]) -> u64 {
    syscall(0, buf.as_mut_ptr() as u64, buf.len() as u64)
}

fn exit() -> ! {
    syscall(2, 0, 0);
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let hello = b"Echo ready\n";
    write(hello);

    loop {
        let mut buf = [0u8; 1];
        let count = read(&mut buf);
        if count > 0 {
            write(&buf);
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit();
}