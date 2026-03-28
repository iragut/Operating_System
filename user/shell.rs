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

fn write(buf: &[u8]) {
    syscall(1, buf.as_ptr() as u64, buf.len() as u64);
}

fn read(buf: &mut [u8]) -> u64 {
    syscall(0, buf.as_mut_ptr() as u64, buf.len() as u64)
}

fn spawn(name: &[u8]) -> u64 {
    syscall(3, name.as_ptr() as u64, name.len() as u64)
}

fn wait(pid: u64) -> u64 {
    syscall(4, pid, 0)
}

fn sys_yield() {
    syscall(5, 0, 0);
}

fn exit() -> ! {
    syscall(60, 0, 0);
    loop {}
}

fn str_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write(b"GameOS Shell\n");

    loop {
        write(b"> ");

        // Read command
        let mut buffer = [0u8; 64];
        let mut len = 0;

        loop {
            let mut character = [0u8; 1];
            let count = read(&mut character);

            // Nothing read, try again
            if count == 0 {
                continue;
            }

            // Enter detected, process command
            if character[0] == b'\n' {
                write(b"\n");
                break;
            }

            if character[0] == 0x08 {
                if len > 0 {
                    len -= 1;
                    write(b"\x08 \x08");
                }
                continue;
            }

            if len < 64 {
                buffer[len] = character[0];
                len += 1;
                write(&character);  // echo the character
            }
        }

        if len == 0 {
            continue;
        }

        let command = &buffer[..len];

        match command {
            _ => write(b"No commnands")
        }

    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit();
}