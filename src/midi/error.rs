use std::fmt;

#[derive(Clone, Copy, Debug)]
pub enum Error {
    ConnectionInitializationError,
    DeviceLoadingError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            Error::ConnectionInitializationError => write!(f, "[midi] error when initializing connections"),
            Error::DeviceLoadingError => write!(f, "[midi] error when loading devices"),
        }
    }
}
