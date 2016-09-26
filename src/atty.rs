/*!
This atty module contains functions for detecting whether ripgrep is being fed
from (or to) a terminal. Windows and Unix do this differently, so implement
both here.
*/

#[cfg(unix)]
pub fn stdin_is_readable() -> bool {
    use std::fs::File;
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::io::{FromRawFd, IntoRawFd};
    use libc;

    let file = unsafe { File::from_raw_fd(libc::STDIN_FILENO) };
    let md = file.metadata();
    let _ = file.into_raw_fd();
    let ft = match md {
        Err(_) => return false,
        Ok(md) => md.file_type(),
    };
    ft.is_file() || ft.is_fifo()
}

#[cfg(windows)]
pub fn stdin_is_readable() -> bool {
    // ???
    true
}

/// Returns true if there is a tty on stdin.
#[cfg(unix)]
pub fn on_stdin() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDIN_FILENO) }
}

/// Returns true if there is a tty on stdout.
#[cfg(unix)]
pub fn on_stdout() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDOUT_FILENO) }
}

/// Returns true if there is a tty on stdin.
#[cfg(windows)]
pub fn on_stdin() -> bool {
    // BUG: https://github.com/BurntSushi/ripgrep/issues/19
    // It's not clear to me how to determine whether there is a tty on stdin.
    // Checking GetConsoleMode(GetStdHandle(stdin)) != 0 appears to report
    // that stdin is a pipe, even if it's not in a cygwin terminal, for
    // example.
    //
    // To fix this, we just assume there is always a tty on stdin. If Windows
    // users need to search stdin, they'll have to pass -. Ug.
    true
}

/// Returns true if there is a tty on stdout.
#[cfg(windows)]
pub fn on_stdout() -> bool {
    use kernel32;
    use winapi;

    unsafe {
        let fd = winapi::winbase::STD_OUTPUT_HANDLE;
        let mut out = 0;
        kernel32::GetConsoleMode(kernel32::GetStdHandle(fd), &mut out) != 0
    }
}
