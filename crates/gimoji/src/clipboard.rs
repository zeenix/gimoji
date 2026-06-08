use std::process::exit;

use arboard::Clipboard as ArboardClipboard;
use gimoji_core::Clipboard;

pub struct ArboardImpl;

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error {
    fn new<E: std::fmt::Display>(e: E) -> Self {
        Self(e.to_string())
    }
}

impl std::error::Error for Error {}

impl Clipboard for ArboardImpl {
    type Error = Error;

    fn copy(&mut self, text: &str) -> Result<(), Self::Error> {
        let owned = text.to_owned();
        copy_and_exit(owned)
    }
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
fn copy_and_exit(text: String) -> Result<(), Error> {
    use arboard::SetExtLinux;
    nix::unistd::daemon(false, false).map_err(Error::new)?;
    ArboardClipboard::new()
        .map_err(Error::new)?
        .set()
        .wait()
        .text(text)
        .map_err(Error::new)?;
    exit(0);
}

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
)))]
fn copy_and_exit(text: String) -> Result<(), Error> {
    ArboardClipboard::new()
        .map_err(Error::new)?
        .set()
        .text(text)
        .map_err(Error::new)?;
    exit(0);
}
