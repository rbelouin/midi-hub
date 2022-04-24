use std::convert::{From, Into};

extern crate portmidi;
use portmidi::{InputPort, OutputPort, MidiEvent, MidiMessage};

pub use crate::image::Image;
use super::Error;

#[derive(Clone, Debug)]
pub enum Event {
    Midi([u8; 4]),
    SysEx(Vec<u8>),
}

pub trait RichDevice {
    /// Device that can associate a MIDI event to an unsigned integer,
    /// that can be used to access elements of an indexed collections.
    fn into_index(event: Event) -> Result<Option<u16>, Error>;

    /// Device that can associate a MIDI event to an unsigned integer,
    /// that can be used to select a midi-hub application.
    fn into_app_index(event: Event) -> Result<Option<u16>, Error>;

    /// Device that can render an image.
    fn from_image(image: Image) -> Result<Event, Error>;

    /// Device that can highlight an element corresponding to an unsigned integer,
    /// this method can be used to emphasize the active element of a collection.
    fn from_index_to_highlight(index: u16) -> Result<Event, Error>;

    /// Device that can highlight the app selection elements with the corresponding colors.
    fn from_app_colors(app_colors: Vec<[u8; 3]>) -> Result<Event, Error>;
}

/// Default implementation for RichDevice
impl<'a> RichDevice for (InputPort<'a>, OutputPort<'a>) {
    fn into_index(event: Event) ->  Result<Option<u16>, Error> {
        return match event {
            // filter "note down" events, for notes higher than C2 (36), and with a strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data1 >= 36 && data2 > 0 => {
                Ok(Some((data1 - 36).into()))
            },
            _ => Ok(None),
        };
    }

    fn into_app_index(event: Event) -> Result<Option<u16>, Error> {
        return match event {
            // filter "note down" events, for notes strictly lower than C0 (12),
            // and with a strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data1 < 12 && data2 > 0 => {
                Ok(Some(data1.into()))
            },
            _ => Ok(None),
        }
    }

    fn from_image(_image: Image) -> Result<Event, Error> {
        eprintln!("[midi] rendering an image is not supported by default");
        return Err(Error::Unsupported);
    }

    fn from_index_to_highlight(_index: u16) -> Result<Event, Error> {
        eprintln!("[midi] highlighting an index is not supported by default");
        return Err(Error::Unsupported);
    }

    fn from_app_colors(_app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
        eprintln!("[midi] coloring app selection buttons is not supported by default");
        return Err(Error::Unsupported);
    }
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
