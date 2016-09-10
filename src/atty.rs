/*!
This atty module contains functions for detecting whether ripgrep is being fed
from (or to) a terminal. Windows and Unix do this differently, so implement
both here.
*/

#[cfg(unix)]
pub fn on_stdin() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDIN_FILENO) }
}

#[cfg(unix)]
pub fn on_stdout() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDOUT_FILENO) }
}

#[cfg(windows)]
pub fn on_stdin() -> bool {
    use kernel32;
    use winapi;

    unsafe {
        let fd = winapi::winbase::STD_INPUT_HANDLE;
        let mut out = 0;
        kernel32::GetConsoleMode(kernel32::GetStdHandle(fd), &mut out) != 0
    }
}

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
