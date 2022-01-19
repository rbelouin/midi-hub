extern crate portmidi;
pub use portmidi::{MidiEvent, MidiMessage};

use super::{Error, InputPort, OutputPort};

/// MIDI Device that is able to emit MIDI events
pub trait Reader {
    fn read(&mut self) -> Result<Option<MidiEvent>, Error>;
}

impl Reader for InputPort<'_> {
    fn read(&mut self) -> Result<Option<MidiEvent>, Error> {
        return self.read().map_err(|_| Error::ReadError);
    }
}

impl<'a> Reader for (InputPort<'a>, OutputPort<'a>) {
    fn read(&mut self) -> Result<Option<MidiEvent>, Error> {
        return Reader::read(&mut self.0);
    }
}

/// MIDI Device that can expose an unsigned integer
/// The default implementation will follow the chromatic scale, starting from C2,
/// but this gives specific vendors to provide a more relevant implementation.
pub trait IndexReader {
    fn read_index(&mut self) -> Result<Option<u16>, Error>;
}

impl IndexReader for InputPort<'_> {
    fn read_index(&mut self) -> Result<Option<u16>, Error> {
        return Reader::read(self).map(|event| {
            return event.filter(|event| event.message.status == 144)
                // a note above C2 (36) must have been read with a strictly positive velocity
                .filter(|event| event.message.data1 >= 36 && event.message.data2 > 0)
                .map(|event| (event.message.data1 - 36) as u16);
        });
    }
}

impl<'a> IndexReader for (InputPort<'a>, OutputPort<'a>) {
    fn read_index(&mut self) -> Result<Option<u16>, Error> {
        return IndexReader::read_index(&mut self.0);
    }
}

/// MIDI Device that is able to receive MIDI events
pub trait Writer {
    fn write(&mut self, event: &MidiEvent) -> Result<(), Error>;
}

impl Writer for OutputPort<'_> {
    fn write(&mut self, event: &MidiEvent) -> Result<(), Error> {
        return self.write_event(*event).map_err(|_| Error::WriteError);
    }
}

impl<'a> Writer for (InputPort<'a>, OutputPort<'a>) {
    fn write(&mut self, event: &MidiEvent) -> Result<(), Error> {
        return Writer::write(&mut self.1, event);
    }
}

/// MIDI Device that is able to render a picture
pub trait ImageRenderer<P> {
    fn render(&mut self, image: P) -> Result<(), Error>;
}
