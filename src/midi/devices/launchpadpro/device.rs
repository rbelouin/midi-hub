use std::convert::From;

use crate::midi::{Reader, Writer, Error, EventTransformer};

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

impl EventTransformer for LaunchpadProEventTransformer {}
