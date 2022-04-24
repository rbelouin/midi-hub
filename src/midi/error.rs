use std::fmt;

#[derive(Clone, Copy, Debug)]
pub enum Error {
    ConnectionInitializationError,
    DeviceLoadingError,
    DeviceNotFound,
    PortInitializationError,
    ReadError,
    WriteError,
    ImageRenderError,
    OutOfBoundIndexError,
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            Error::ConnectionInitializationError => write!(f, "[midi] error when initializing connections"),
            Error::DeviceLoadingError => write!(f, "[midi] error when loading devices"),
            Error::DeviceNotFound => write!(f, "[midi] could not find device"),
            Error::PortInitializationError => write!(f, "[midi] error when initializing a port"),
            Error::ReadError => write!(f, "[midi] could not read an event"),
            Error::WriteError => write!(f, "[midi] could not write an event"),
            Error::ImageRenderError => write!(f, "[midi] could not render image"),
            Error::OutOfBoundIndexError => write!(f, "[midi] could not handle index"),
            Error::Unsupported => write!(f, "[midi] unsupported operation"),
        }
    }
}
