/*!
This io module contains various platform specific functions for detecting
how xrep is being used. e.g., Is stdin being piped into it? Is stdout being
redirected to a file? etc... We use this information to tweak various default
configuration parameters such as colors and match formatting.
*/

#[cfg(unix)]
pub fn stdin_is_atty() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDIN_FILENO) }
}

#[cfg(unix)]
pub fn stdout_is_atty() -> bool {
    use libc;
    0 < unsafe { libc::isatty(libc::STDOUT_FILENO) }
}

#[cfg(windows)]
pub fn stdin_is_atty() -> bool {
    use kernel32;
    use winapi;

    unsafe {
        let fd = winapi::winbase::STD_INPUT_HANDLE;
        let mut out = 0;
        kernel32::GetConsoleMode(kernel32::GetStdHandle(fd), &mut out) != 0
    }
}

#[cfg(windows)]
pub fn stdout_is_atty() -> bool {
    use kernel32;
    use winapi;

    unsafe {
        let fd = winapi::winbase::STD_OUTPUT_HANDLE;
        let mut out = 0;
        kernel32::GetConsoleMode(kernel32::GetStdHandle(fd), &mut out) != 0
    }
}
