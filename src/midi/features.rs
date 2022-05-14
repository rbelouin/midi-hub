use std::convert::From;
use std::error::Error as StdError;
use std::fmt::{Debug, Display, Error, Formatter};

use super::Event;

pub type R<A> = Result<A, Box<dyn StdError>>;

#[derive(Debug)]
pub struct UnsupportedFeatureError {
    name: &'static str,
}

impl StdError for UnsupportedFeatureError {}
impl Display for UnsupportedFeatureError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "feature unsupported by the device: {}", self.name)
    }
}
impl From<&'static str> for UnsupportedFeatureError {
    fn from(name: &'static str) -> UnsupportedFeatureError {
        UnsupportedFeatureError { name }
    }
}

/// A grid controller is typically a MIDI device with pads arranged on a grid layout.
/// It _must_ be able to expose its size and transform MIDI events into coordinates.
pub trait GridController {
    /// The width must be specified first when exposing the size of the grid layout.
    fn get_grid_size(&self) -> R<(usize, usize)>;

    /// The x-coordinate must be specified first when exposing the position.
    /// (0, 0) must correspond to the top-left corner of the grid layout.
    fn into_coordinates(&self, event: Event) -> R<Option<(usize, usize)>>;
}

impl<T> GridController for T {
    default fn get_grid_size(&self) -> R<(usize, usize)> {
        Err(Box::new(UnsupportedFeatureError::from("grid-controller:get_grid_size")))
    }

    default fn into_coordinates(&self, _event: Event) -> R<Option<(usize, usize)>> {
        Err(Box::new(UnsupportedFeatureError::from("grid-controller:into_coordinates")))
    }
}
