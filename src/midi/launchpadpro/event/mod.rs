use std::convert::From;

use crate::image::Image;
use crate::midi::{Error, Event, RichEvent};

mod image;
mod index;

#[derive(Clone, Debug)]
pub struct LaunchpadProEvent {
    event: Event,
}

impl From<Event> for LaunchpadProEvent {
    fn from(event: Event) -> LaunchpadProEvent {
        return LaunchpadProEvent { event };
    }
}

impl From<LaunchpadProEvent> for Event {
    fn from(event: LaunchpadProEvent) -> Event {
        return event.event;
    }
}

impl RichEvent<LaunchpadProEvent> for LaunchpadProEvent {
    fn into_index(self) -> Result<Option<u16>, Error> {
        return index::into_index(self);
    }

    fn into_app_index(self) -> Result<Option<u16>, Error> {
        return index::into_app_index(self);
    }

    fn from_image(image: Image) -> Result<Self, Error> {
        return image::from_image(image);
    }

    fn from_index_to_highlight(index: u16) -> Result<Self, Error> {
        return index::from_index_to_highlight(index);
    }

    fn from_app_colors(app_colors: Vec<[u8; 3]>) -> Result<Self, Error> {
        return index::from_app_colors(app_colors);
    }
}
