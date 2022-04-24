use std::convert::From;
use crate::midi::{Reader, Writer, Error, Event};
use super::LaunchpadProEvent;

pub struct LaunchpadPro<C> where
    C: Reader<Event>,
    C: Writer<Event>,
{
    pub connection: C,
}

impl<C> From<C> for LaunchpadPro<C> where
    C: Reader<Event>,
    C: Writer<Event>,
{
    fn from(connection: C) -> LaunchpadPro<C> {
        return LaunchpadPro { connection };
    }
}

impl<C> Reader<LaunchpadProEvent> for LaunchpadPro<C> where
    C: Reader<Event>,
    C: Writer<Event>,
{
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return Reader::read_midi(&mut self.connection);
    }
}

impl<C> Writer<LaunchpadProEvent> for LaunchpadPro<C> where
    C: Reader<Event>,
    C: Writer<Event>,
{
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return Writer::write_midi(&mut self.connection, event);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return Writer::write_sysex(&mut self.connection, event);
    }
}
