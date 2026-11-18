//! The parts of `std::io` borsh uses, for `no_std` builds. Behavior matches `std` where
//! borsh can observe it.
use crate::__private::maybestd::string::String;
use core::fmt;

pub type Result<T> = core::result::Result<T, Error>;

/// An I/O error: a kind, optionally with a message.
pub struct Error {
    kind: ErrorKind,
    message: Option<String>,
}

macro_rules! kinds {
    ($($kind:ident => $text:literal,)*) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[non_exhaustive]
        pub enum ErrorKind { $($kind,)* }
        impl ErrorKind {
            pub(crate) fn as_str(&self) -> &'static str {
                match *self { $(ErrorKind::$kind => $text,)* }
            }
        }
    };
}
kinds! {
    NotFound => "entity not found",
    PermissionDenied => "permission denied",
    ConnectionRefused => "connection refused",
    ConnectionReset => "connection reset",
    ConnectionAborted => "connection aborted",
    NotConnected => "not connected",
    AddrInUse => "address in use",
    AddrNotAvailable => "address not available",
    BrokenPipe => "broken pipe",
    AlreadyExists => "entity already exists",
    WouldBlock => "operation would block",
    InvalidInput => "invalid input parameter",
    InvalidData => "invalid data",
    TimedOut => "timed out",
    WriteZero => "write zero",
    Interrupted => "operation interrupted",
    Other => "other os error",
    UnexpectedEof => "unexpected end of file",
    OutOfMemory => "out of memory",
}

impl From<ErrorKind> for Error {
    #[inline]
    fn from(kind: ErrorKind) -> Error {
        Error {
            kind,
            message: None,
        }
    }
}

impl Error {
    pub fn new<E: Into<String>>(kind: ErrorKind, error: E) -> Error {
        Error {
            kind,
            message: Some(error.into()),
        }
    }

    pub fn other<E: Into<String>>(error: E) -> Error {
        Self::new(ErrorKind::Other, error)
    }

    /// The message, if the error has one.
    #[must_use]
    pub fn get_ref(&self) -> Option<&str> {
        self.message.as_deref()
    }

    #[must_use = "`self` will be dropped if the result is not used"]
    pub fn into_inner(self) -> Option<String> {
        self.message
    }

    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            None => f.debug_tuple("Kind").field(&self.kind).finish(),
            Some(error) => f
                .debug_struct("Custom")
                .field("kind", &self.kind)
                .field("error", error)
                .finish(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(message) => fmt::Display::fmt(message, f),
            None => f.write_str(self.kind.as_str()),
        }
    }
}

fn write_zero() -> Error {
    Error::new(ErrorKind::WriteZero, "failed to write whole buffer")
}

fn unexpected_eof() -> Error {
    Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer")
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    fn flush(&mut self) -> Result<()>;

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(write_zero()),
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<()> {
        struct Adaptor<'a, T: ?Sized> {
            inner: &'a mut T,
            error: Result<()>,
        }
        impl<T: Write + ?Sized> fmt::Write for Adaptor<'_, T> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.inner.write_all(s.as_bytes()).map_err(|e| {
                    self.error = Err(e);
                    fmt::Error
                })
            }
        }
        let mut output = Adaptor {
            inner: self,
            error: Ok(()),
        };
        match fmt::write(&mut output, fmt) {
            Ok(()) => Ok(()),
            Err(..) if output.error.is_err() => output.error,
            Err(..) => Err(Error::new(ErrorKind::Other, "formatter error")),
        }
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

impl<W: Write + ?Sized> Write for &mut W {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }
    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (**self).write_all(buf)
    }
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<()> {
        (**self).write_fmt(fmt)
    }
}

/// The default `write_all` is enough here: one write fills what fits, then the loop returns
/// `WriteZero` ("failed to write whole buffer") if the slice ran out.
impl Write for &mut [u8] {
    #[inline]
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        let amt = core::cmp::min(data.len(), self.len());
        let (a, b) = core::mem::take(self).split_at_mut(amt);
        a.copy_from_slice(&data[..amt]);
        *self = b;
        Ok(amt)
    }
    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// `write` takes the whole buffer, so the default `write_all` is a single call.
impl Write for alloc::vec::Vec<u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }
    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Fill `buf`, retrying `Interrupted`, or fail with `UnexpectedEof`.
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => return Err(unexpected_eof()),
                Ok(n) => buf = &mut core::mem::take(&mut buf)[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

impl<R: Read + ?Sized> Read for &mut R {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_exact(buf)
    }
}

impl Read for &[u8] {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let amt = core::cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);
        buf[..amt].copy_from_slice(a);
        *self = b;
        Ok(amt)
    }
    /// On short input this fails without consuming anything.
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        if buf.len() > self.len() {
            return Err(unexpected_eof());
        }
        let (a, b) = self.split_at(buf.len());
        buf.copy_from_slice(a);
        *self = b;
        Ok(())
    }
}
