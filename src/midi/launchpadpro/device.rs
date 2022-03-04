use std::convert::From;
use crate::midi::{InputPort, OutputPort, Reader, Writer, Error};
use super::LaunchpadProEvent;

pub struct LaunchpadPro<'a> {
    device: (InputPort<'a>, OutputPort<'a>),
}

impl<'a> From<(InputPort<'a>, OutputPort<'a>)> for LaunchpadPro<'a> {
    fn from(ports: (InputPort<'a>, OutputPort<'a>)) -> LaunchpadPro<'a> {
        return LaunchpadPro { device: ports };
    }
}

impl<'a> From<LaunchpadPro<'a>> for (InputPort<'a>, OutputPort<'a>) {
    fn from(launchpadpro: LaunchpadPro<'a>) -> (InputPort<'a>, OutputPort<'a>) {
        return launchpadpro.device;
    }
}

impl Reader<LaunchpadProEvent> for LaunchpadPro<'_> {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return Reader::read_midi(&mut self.device);
    }
}

impl Writer<LaunchpadProEvent> for LaunchpadPro<'_> {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return Writer::write_midi(&mut self.device, event);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return Writer::write_sysex(&mut self.device, event);
    }
}
