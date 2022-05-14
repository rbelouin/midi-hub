use std::convert::From;

use crate::midi::{Reader, Writer, Error, Event, EventTransformer, Image};

pub struct LaunchpadPro<C> where C: Reader + Writer {
    pub connection: C,
    pub transformer: &'static LaunchpadProEventTransformer,
}

impl<C> From<C> for LaunchpadPro<C> where C: Reader + Writer {
    fn from(connection: C) -> LaunchpadPro<C> {
        return LaunchpadPro { connection, transformer: &LAUNCHPADPRO_EVENT_TRANSFORMER };
    }
}

impl<C> Reader for LaunchpadPro<C> where C: Reader + Writer {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return Reader::read_midi(&mut self.connection);
    }
}

impl<C> Writer for LaunchpadPro<C> where C: Reader + Writer {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return Writer::write_midi(&mut self.connection, event);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return Writer::write_sysex(&mut self.connection, event);
    }
}

pub fn transformer() -> &'static LaunchpadProEventTransformer {
    return &LAUNCHPADPRO_EVENT_TRANSFORMER;
}

static LAUNCHPADPRO_EVENT_TRANSFORMER: LaunchpadProEventTransformer = LaunchpadProEventTransformer::new();
pub struct LaunchpadProEventTransformer {}
impl LaunchpadProEventTransformer {
    const fn new() -> LaunchpadProEventTransformer {
        LaunchpadProEventTransformer {}
    }
}

impl EventTransformer for LaunchpadProEventTransformer {
    fn into_index(&self, event: Event) -> Result<Option<u16>, Error> {
        return super::index::into_index(event);
    }

    fn into_color_palette_index(&self, event: Event) -> Result<Option<u16>, Error> {
        return super::index::into_color_palette_index(event);
    }

    fn from_image(&self, image: Image) -> Result<Event, Error> {
        return super::image::from_image(image);
    }

    fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
        return super::index::from_index_to_highlight(index);
    }

    fn from_color_palette(&self, color_palette: Vec<[u8; 3]>) -> Result<Event, Error> {
        return super::index::from_color_palette(color_palette);
    }
}
