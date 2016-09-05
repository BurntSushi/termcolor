/*!
This io module contains various platform specific functions for detecting
how xrep is being used. e.g., Is stdin being piped into it? Is stdout being
redirected to a file? etc... We use this information to tweak various default
configuration parameters such as colors and match formatting.
*/

use std::fs::{File, Metadata};
use std::io;

use libc;

#[cfg(unix)]
pub fn stdin_is_atty() -> bool {
    0 < unsafe { libc::isatty(libc::STDIN_FILENO) }
}

#[cfg(unix)]
pub fn stdout_is_atty() -> bool {
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
        kernel32::GetConsoleMode(handle, &mut out) != 0
    }
}

// Probably everything below isn't actually needed. ---AG

#[cfg(unix)]
pub fn metadata(fd: libc::c_int) -> Result<Metadata, io::Error> {
    use std::os::unix::io::{FromRawFd, IntoRawFd};

    let f = unsafe { File::from_raw_fd(fd) };
    let md = f.metadata();
    // Be careful to transfer ownership back to a simple descriptor. Dropping
    // the File itself would close the descriptor, which would be quite bad!
    drop(f.into_raw_fd());
    md
}

#[cfg(unix)]
pub fn stdin_is_file() -> bool {
    metadata(libc::STDIN_FILENO)
        .map(|md| md.file_type().is_file())
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn stdout_is_file() -> bool {
    metadata(libc::STDOUT_FILENO)
        .map(|md| md.file_type().is_file())
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn stdin_is_char_device() -> bool {
    use std::os::unix::fs::FileTypeExt;

    metadata(libc::STDIN_FILENO)
        .map(|md| md.file_type().is_char_device())
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn stdout_is_char_device() -> bool {
    use std::os::unix::fs::FileTypeExt;

    metadata(libc::STDOUT_FILENO)
        .map(|md| md.file_type().is_char_device())
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn stdin_is_fifo() -> bool {
    use std::os::unix::fs::FileTypeExt;

    metadata(libc::STDIN_FILENO)
        .map(|md| md.file_type().is_fifo())
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn stdout_is_fifo() -> bool {
    use std::os::unix::fs::FileTypeExt;

    metadata(libc::STDOUT_FILENO)
        .map(|md| md.file_type().is_fifo())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn stdin_is_file() -> bool {
    false
}

#[cfg(windows)]
pub fn stdout_is_file() -> bool {
    false
}

#[cfg(windows)]
pub fn stdin_is_char_device() -> bool {
    false
}

#[cfg(windows)]
pub fn stdout_is_char_device() -> bool {
    false
}

#[cfg(windows)]
pub fn stdin_is_fifo() -> bool {
    false
}

#[cfg(windows)]
pub fn stdout_is_fifo() -> bool {
    false
}
