extern crate portmidi;
use portmidi::PortMidi;

use super::error::Error;

/// This structure manages all MIDI connections
///
/// On macOS, hot-reload does not work and you will have to restart the program after plugging or
/// unplugging a device.
///
/// On linux, dropping the instance of PortMidi and re-instantiating one should be enough to
/// reflect the changes.
pub struct Connections {
    /// PortMidi’s base structure
    /// Keep a precious reference of it, as the input and output ports will have the same lifetime
    context: PortMidi
}

impl Connections {
    pub fn new() -> Result<Connections, Error> {
        return PortMidi::new()
            .map(|context| Connections { context })
            .map_err(|_| Error::ConnectionInitializationError);
    }

    /// TODO remove this getter once ports are correctly exposed
    pub fn context(self) -> PortMidi {
        return self.context;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Can’t really think of a scenario where initializition would fail
    fn new_should_return_ok() {
        assert!(Connections::new().is_ok());
    }
}
