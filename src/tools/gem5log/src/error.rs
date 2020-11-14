use std::fmt;
use std::io;
use std::num;

pub enum Error {
    IoError(io::Error),
    NumError(num::ParseIntError),
    LogLevelError(log::ParseLevelError),
    SetLogError(log::SetLoggerError),
    NmError(i32),
    InvalPath,
}

macro_rules! impl_err {
    ($src:ty, $dst:tt) => {
        impl From<$src> for Error {
            fn from(error: $src) -> Self {
                Error::$dst(error)
            }
        }
    };
}

impl_err!(io::Error, IoError);
impl_err!(num::ParseIntError, NumError);
impl_err!(log::ParseLevelError, LogLevelError);
impl_err!(log::SetLoggerError, SetLogError);

impl fmt::Debug for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Error::IoError(e) => write!(fmt, "I/O error occurred: {}", e),
            Error::NumError(e) => write!(fmt, "Unable to parse number: {}", e),
            Error::SetLogError(e) => write!(fmt, "Setting logger failed: {}", e),
            Error::LogLevelError(e) => write!(fmt, "Parsing log level failed: {}", e),
            Error::NmError(c) => write!(fmt, "nm -SC <bin> failed: {}", c),
            Error::InvalPath => write!(fmt, "path is invalid"),
        }
    }
}
