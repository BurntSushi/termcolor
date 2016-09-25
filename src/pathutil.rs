/*!
The pathutil module provides platform specific operations on paths that are
typically faster than the same operations as provided in std::path. In
particular, we really want to avoid the costly operation of parsing the path
into its constituent components. We give up on Windows, but on Unix, we deal
with the raw bytes directly.

On large repositories (like chromium), this can have a ~25% performance
improvement on just listing the files to search (!).
*/
use std::ffi::OsStr;
use std::path::Path;

/// Strip `prefix` from the `path` and return the remainder.
///
/// If `path` doesn't have a prefix `prefix`, then return `None`.
#[cfg(unix)]
pub fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    use std::os::unix::ffi::OsStrExt;

    let prefix = prefix.as_ref().as_os_str().as_bytes();
    let path = path.as_os_str().as_bytes();
    if prefix.len() > path.len() || prefix != &path[0..prefix.len()] {
        None
    } else {
        Some(&Path::new(OsStr::from_bytes(&path[prefix.len()..])))
    }
}

/// Strip `prefix` from the `path` and return the remainder.
///
/// If `path` doesn't have a prefix `prefix`, then return `None`.
#[cfg(not(unix))]
pub fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    path.strip_prefix(prefix).ok()
}

/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(unix)]
pub fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    use std::os::unix::ffi::OsStrExt;
    use memchr::memrchr;

    let path = path.as_ref().as_os_str().as_bytes();
    if path.is_empty() {
        return None;
    } else if path.len() == 1 && path[0] == b'.' {
        return None;
    } else if path.last() == Some(&b'.') {
        return None;
    } else if path.len() >= 2 && &path[path.len() - 2..] == &b".."[..] {
        return None;
    }
    let last_slash = memrchr(b'/', path).map(|i| i + 1).unwrap_or(0);
    Some(OsStr::from_bytes(&path[last_slash..]))
}

/// The final component of the path, if it is a normal file.
///
/// If the path terminates in ., .., or consists solely of a root of prefix,
/// file_name will return None.
#[cfg(not(unix))]
pub fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    path.as_ref().file_name()
}

/// Returns true if and only if this file path is considered to be hidden.
#[cfg(unix)]
pub fn is_hidden<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::ffi::OsStrExt;

    if let Some(name) = file_name(path.as_ref()) {
        name.as_bytes().get(0) == Some(&b'.')
    } else {
        false
    }
}

/// Returns true if and only if this file path is considered to be hidden.
#[cfg(not(unix))]
pub fn is_hidden<P: AsRef<Path>>(path: P) -> bool {
    if let Some(name) = file_name(path.as_ref()) {
        name.to_str().map(|s| s.starts_with(".")).unwrap_or(false)
    } else {
        false
    }
}

/// Returns true if this file path is just a file name. i.e., Its parent is
/// the empty string.
#[cfg(unix)]
pub fn is_file_name<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::ffi::OsStrExt;
    use memchr::memchr;

    let path = path.as_ref().as_os_str().as_bytes();
    memchr(b'/', path).is_none()
}

/// Returns true if this file path is just a file name. i.e., Its parent is
/// the empty string.
#[cfg(not(unix))]
pub fn is_file_name<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().parent().map(|p| p.as_os_str().is_empty()).unwrap_or(false)
}
