use std::convert::{From, Into};

extern crate portmidi;
use portmidi::{MidiEvent, MidiMessage};

use super::{Error, InputPort, OutputPort};

#[derive(Clone, Debug)]
pub enum Event {
    Midi([u8; 4]),
    SysEx(Vec<u8>),
}

/// MIDI Device that is able to emit MIDI events
pub trait Reader<E> where E: From<Event> {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error>;
    fn read(&mut self) -> Result<Option<E>, Error> {
        let midi = self.read_midi()?;
        return Ok(midi.map(|m| Event::Midi(m).into()));
    }
}

impl Reader<Event> for InputPort<'_> {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return self.read()
            .map(|event| 
                event.map(|e| [e.message.status, e.message.data1, e.message.data2, e.message.data3]))
            .map_err(|_| Error::ReadError);
    }
}

impl<'a> Reader<Event> for (InputPort<'a>, OutputPort<'a>) {
    fn read_midi(&mut self) -> Result<Option<[u8; 4]>, Error> {
        return Reader::read_midi(&mut self.0);
    }
}

/// MIDI Device that is able to receive MIDI events and SysEx MIDI messages
pub trait Writer<E> where E: Into<Event> {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error>;
    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error>;

    fn write(&mut self, event: E) -> Result<(), Error> {
        return match event.into() {
            Event::Midi(event) => self.write_midi(&event),
            Event::SysEx(event) => self.write_sysex(&event),
        };
    }
}

impl Writer<Event> for OutputPort<'_> {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return self.write_event(MidiEvent::from(MidiMessage::from(*event))).map_err(|_| Error::WriteError);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return OutputPort::write_sysex(self, 0, event).map_err(|_| Error::WriteError);
    }
}

impl<'a> Writer<Event> for (InputPort<'a>, OutputPort<'a>) {
    fn write_midi(&mut self, event: &[u8; 4]) -> Result<(), Error> {
        return Writer::write_midi(&mut self.1, event);
    }

    fn write_sysex(&mut self, event: &[u8]) -> Result<(), Error> {
        return Writer::write_sysex(&mut self.1, event);
    }
}
