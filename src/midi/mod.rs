mod connections;
mod device;
mod error;

pub use connections::{Connections, InputPort, OutputPort};
pub use device::*;
pub use error::Error;

/// MIDI vendors
pub mod launchpadpro;
