use std::convert::From;

use crate::midi::{Reader, Writer, Error};
use crate::midi::features::Features;

pub struct LaunchpadPro<C> where C: Reader + Writer {
    pub connection: C,
    pub features: LaunchpadProFeatures,
}

impl<C> From<C> for LaunchpadPro<C> where C: Reader + Writer {
    fn from(connection: C) -> LaunchpadPro<C> {
        return LaunchpadPro { connection, features: LaunchpadProFeatures::new() };
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

pub struct LaunchpadProFeatures {}
impl LaunchpadProFeatures {
    pub fn new() -> LaunchpadProFeatures {
        LaunchpadProFeatures {}
    }
}

impl Features for LaunchpadProFeatures {}
