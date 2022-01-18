extern crate portmidi;
pub use portmidi::MidiEvent;

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
