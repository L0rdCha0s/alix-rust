use crate::kernel::user;
use crate::kernel::vfs;
use alloc::string::String;

#[no_mangle]
pub extern "C" fn user_shell() -> ! {
    // Simple userland shell: prompt, read line, echo with String.
    let stdout = vfs::FD_STDOUT as u64;
    let stdin = vfs::FD_STDIN as u64;
    let mut line = String::new();
    loop {
        let _ = user::write(stdout, "$ ");
        let mut saw_cr = false;
        loop {
            let mut byte = [0u8; 1];
            let read = user::read(stdin, &mut byte);
            if read == 0 || read == u64::MAX {
                let _ = user::sleep_ms(10);
                continue;
            }
            let mut b = byte[0];
            if b == b'\r' {
                saw_cr = true;
                b = b'\n';
            } else if b == b'\n' {
                if saw_cr {
                    saw_cr = false;
                    continue;
                }
            } else {
                saw_cr = false;
            }
            if b == b'\n' || b == b'\r' {
                let _ = user::write(stdout, "\n");
                let _ = user::write(stdout, "String: ");
                let _ = user::write(stdout, line.as_str());
                let _ = user::write(stdout, "\n");
                line.clear();
                break;
            }
            if b == 0x08 || b == 0x7f {
                if !line.is_empty() {
                    line.pop();
                    let _ = user::write(stdout, "\u{8} \u{8}");
                }
                continue;
            }
            if line.len() < 128 {
                line.push(b as char);
                let mut echo = [0u8; 1];
                echo[0] = b;
                let _ = user::write_bytes(stdout, &echo);
            }
        }
    }
}
