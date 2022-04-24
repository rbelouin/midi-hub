use std::convert::From;

use crate::midi::{Reader, Writer, Error, Event, RichDevice, Image};

pub struct LaunchpadPro<C> where C: Reader + Writer {
    pub connection: C,
}

impl<C> From<C> for LaunchpadPro<C> where C: Reader + Writer {
    fn from(connection: C) -> LaunchpadPro<C> {
        return LaunchpadPro { connection };
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

impl<C> RichDevice for LaunchpadPro<C> where C: Reader + Writer {
    fn into_index(event: Event) -> Result<Option<u16>, Error> {
        return super::index::into_index(event);
    }

    fn into_app_index(event: Event) -> Result<Option<u16>, Error> {
        return super::index::into_app_index(event);
    }

    fn from_image(image: Image) -> Result<Event, Error> {
        return super::image::from_image(image);
    }

    fn from_index_to_highlight(index: u16) -> Result<Event, Error> {
        return super::index::from_index_to_highlight(index);
    }

    fn from_app_colors(app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
        return super::index::from_app_colors(app_colors);
    }
}
