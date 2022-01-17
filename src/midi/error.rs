use std::fmt;

#[derive(Clone, Copy, Debug)]
pub enum Error {
    ConnectionInitializationError,
    DeviceLoadingError,
    DeviceNotFound,
    PortInitializationError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            Error::ConnectionInitializationError => write!(f, "[midi] error when initializing connections"),
            Error::DeviceLoadingError => write!(f, "[midi] error when loading devices"),
            Error::DeviceNotFound => write!(f, "[midi] could not find device"),
            Error::PortInitializationError => write!(f, "[midi] error when initializing a port"),
        }
    }
}
