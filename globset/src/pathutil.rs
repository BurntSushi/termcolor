use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

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

/// Return a file extension given a path's file name.
///
/// Note that this does NOT match the semantics of std::path::Path::extension.
/// Namely, the extension includes the `.` and matching is otherwise more
/// liberal. Specifically, the extenion is:
///
/// * None, if the file name given is empty;
/// * None, if there is no embedded `.`;
/// * Otherwise, the portion of the file name starting with the final `.`.
///
/// e.g., A file name of `.rs` has an extension `.rs`.
///
/// N.B. This is done to make certain glob match optimizations easier. Namely,
/// a pattern like `*.rs` is obviously trying to match files with a `rs`
/// extension, but it also matches files like `.rs`, which doesn't have an
/// extension according to std::path::Path::extension.
pub fn file_name_ext(name: &OsStr) -> Option<&OsStr> {
    // Yes, these functions are awful, and yes, we are completely violating
    // the abstraction barrier of std::ffi. The barrier we're violating is
    // that an OsStr's encoding is *ASCII compatible*. While this is obviously
    // true on Unix systems, it's also true on Windows because an OsStr uses
    // WTF-8 internally: https://simonsapin.github.io/wtf-8/
    //
    // We should consider doing the same for the other path utility functions.
    // Right now, we don't break any barriers, but Windows users are paying
    // for it.
    //
    // Got any better ideas that don't cost anything? Hit me up. ---AG
    unsafe fn os_str_as_u8_slice(s: &OsStr) -> &[u8] {
        ::std::mem::transmute(s)
    }
    unsafe fn u8_slice_as_os_str(s: &[u8]) -> &OsStr {
        ::std::mem::transmute(s)
    }
    if name.is_empty() {
        return None;
    }
    let name = unsafe { os_str_as_u8_slice(name) };
    for (i, &b) in name.iter().enumerate().rev() {
        if b == b'.' {
            return Some(unsafe { u8_slice_as_os_str(&name[i..]) });
        }
    }
    None
}

/// Return raw bytes of a path, transcoded to UTF-8 if necessary.
pub fn path_bytes(path: &Path) -> Cow<[u8]> {
    os_str_bytes(path.as_os_str())
}

/// Return the raw bytes of the given OS string, transcoded to UTF-8 if
/// necessary.
#[cfg(unix)]
pub fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(s.as_bytes())
}

/// Return the raw bytes of the given OS string, transcoded to UTF-8 if
/// necessary.
#[cfg(not(unix))]
pub fn os_str_bytes(s: &OsStr) -> Cow<[u8]> {
    // TODO(burntsushi): On Windows, OS strings are probably UTF-16, so even
    // if we could get at the raw bytes, they wouldn't be useful. We *must*
    // convert to UTF-8 before doing path matching. Unfortunate, but necessary.
    match s.to_string_lossy() {
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::file_name_ext;

    macro_rules! ext {
        ($name:ident, $file_name:expr, $ext:expr) => {
            #[test]
            fn $name() {
                let got = file_name_ext(OsStr::new($file_name));
                assert_eq!($ext.map(OsStr::new), got);
            }
        };
    }

    ext!(ext1, "foo.rs", Some(".rs"));
    ext!(ext2, ".rs", Some(".rs"));
    ext!(ext3, "..rs", Some(".rs"));
    ext!(ext4, "", None::<&str>);
    ext!(ext5, "foo", None::<&str>);
}
