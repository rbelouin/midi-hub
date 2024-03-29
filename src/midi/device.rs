use std::convert::From;

extern crate portmidi;
use portmidi::{InputPort, OutputPort, MidiEvent, MidiMessage};

pub use crate::image::Image;
use super::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Midi([u8; 4]),
    SysEx(Vec<u8>),
}

/// MIDI Device that is able to emit MIDI events
pub trait Reader {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error>;
    fn read(&mut self) -> Result<Option<Event>, Error> {
        let midi = self.read_midi()?;
        return Ok(midi.map(|m| Event::Midi(m)));
    }
}

impl Reader for InputPort<'_> {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return self.read()
            .map(|event| 
                event.map(|e| [e.message.status, e.message.data1, e.message.data2, e.message.data3]))
            .map_err(|_| Error::ReadError);
    }
}

impl<'a> Reader for (InputPort<'a>, OutputPort<'a>) {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return Reader::read_midi(&mut self.0);
    }
}

/// MIDI Device that is able to receive MIDI events and SysEx MIDI messages
pub trait Writer {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error>;
    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error>;

    fn write(&mut self, event: Event) -> Result<(), Error> {
        return match event {
            Event::Midi(event) => self.write_midi(&event),
            Event::SysEx(event) => self.write_sysex(&event),
        };
    }
}

impl Writer for OutputPort<'_> {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return self.write_event(MidiEvent::from(MidiMessage::from(*event))).map_err(|_| Error::WriteError);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return OutputPort::write_sysex(self, 0, event).map_err(|_| Error::WriteError);
    }
}

impl<'a> Writer for (InputPort<'a>, OutputPort<'a>) {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return Writer::write_midi(&mut self.1, event);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return Writer::write_sysex(&mut self.1, event);
    }
}
