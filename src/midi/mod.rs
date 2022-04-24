mod connections;
mod device;
mod error;

pub use connections::Connections;
pub use device::*;
pub use error::Error;

/// MIDI vendors
pub mod launchpadpro;
