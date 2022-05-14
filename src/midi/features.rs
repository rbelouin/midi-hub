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

/// An app selector is a device that provides a UI to switch between different midi-hub apps.
pub trait AppSelector {
    /// Convert a MIDI event into an index, triggering the selection of the corresponding app.
    fn into_app_index(&self, event: Event) -> R<Option<usize>>;

    /// If the device supports it, it will be passed a vector of colors,
    /// to light the "app-selection" UI elements with their corresponding color.
    fn from_app_colors(&self, app_colors: Vec<[u8; 3]>) -> R<Event>;
}

impl<T> AppSelector for T {
    /// This default implementation uses note-down events for notes from the C-1/B-1 octave.
    default fn into_app_index(&self, event: Event) -> R<Option<usize>> {
        match event {
            // 144: note-down
            // data1 < 12: corresponds to the C-1/B-1 octave
            // data2 > 0: corresponds to the velocity (the key really needs to be pressed)
            Event::Midi([144, data1, data2, _]) if data1 < 12 && data2 > 0 => {
                Ok(Some(data1.into()))
            },
            _ => Ok(None),
        }
    }

    default fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> R<Event> {
        Err(Box::new(UnsupportedFeatureError::from("app-selector:from_app_colors")))
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
