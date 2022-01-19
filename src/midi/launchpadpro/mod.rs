use std::convert::From;
use crate::image::Pixel;
use super::{InputPort, OutputPort, MidiEvent, Error, Reader, Writer, ImageRenderer, IndexReader};

pub struct LaunchpadPro<'a> {
    input_port: InputPort<'a>,
    output_port: OutputPort<'a>,
}

impl LaunchpadPro<'_> {
    /// TODO let consumers assume they can render a picture from the top-left corner and keep this
    /// logic internal.
    pub fn map_index(i: u8) -> u8 {
        let index = if i > 63 {
            63
        } else {
            i
        };

        let row = index / 4;

        if row < 8 {
            return (7 - row) * 8 + (index % 4);
        } else {
            return (15 - row) * 8 + 4 + (index % 4);
        }
    }
}

impl<'a> From<(InputPort<'a>, OutputPort<'a>)> for LaunchpadPro<'a> {
    fn from(ports: (InputPort<'a>, OutputPort<'a>)) -> LaunchpadPro<'a> {
        return LaunchpadPro { input_port: ports.0, output_port: ports.1 };
    }
}

impl Reader for LaunchpadPro<'_> {
    fn read(&mut self) -> Result<Option<MidiEvent>, Error> {
        return Reader::read(&mut self.input_port);
    }
}

impl IndexReader for LaunchpadPro<'_> {
    fn read_index(&mut self) -> Result<Option<u16>, Error> {
        return Reader::read(self).map(|event| event.and_then(|event| map_event_to_index(event)));
    }
}

pub fn map_event_to_index(event: MidiEvent) -> Option<u16> {
    return Some(event)
        // event must be a "note down"
        .filter(|event| event.message.status == 144)
        // event must have a strictly positive velocity
        .filter(|event| event.message.data2 > 0)
        // event must correspond to a square pad
        // then map the index by starting from the grid’s bottom-left corner
        .and_then(|event| {
            let value = event.message.data1;
            let row = value / 10;
            let column  = value % 10;

            if row >= 1 && row <= 8 && column >= 1 && column <= 8 {
                return Some((row - 1) * 8 + (column - 1)).map(|index| index.into());
            } else {
                return None;
            }
        });
}

impl Writer for LaunchpadPro<'_> {
    fn write(&mut self, event: &MidiEvent) -> Result<(), Error> {
        return Writer::write(&mut self.output_port, event);
    }
}

impl ImageRenderer<Vec<Pixel>> for LaunchpadPro<'_> {
    fn render(&mut self, pixels: Vec<Pixel>) -> Result<(), Error> {
        if pixels.len() != 64 {
            println!("Error: the number of pixels is not 64: {}", pixels.len());
            return Err(Error::ImageRenderError);
        }

        let mut reversed_pixels = vec![Pixel { r: 0, g: 0, b: 0 }; 64];
        for y in 0..8 {
            for x in 0..8 {
                reversed_pixels[y * 8 + x] = pixels[(7 - y) * 8 + x];
            }
        }

        let mut transformed_pixels = reversed_pixels
            .iter()
            .flat_map(|pixel| vec![pixel.r / 4, pixel.g / 4, pixel.b / 4])
            .collect();

        let mut picture = vec![240, 0, 32, 41, 2, 16, 15, 1];
        picture.append(&mut transformed_pixels);
        picture.append(&mut vec![247]);

        return self.output_port.write_sysex(0, &picture)
            .map_err(|_| Error::ImageRenderError);
    }
}

#[cfg(test)]
mod tests {
    use super::super::MidiMessage;
    use super::*;

    #[test]
    fn map_event_to_index_given_incorrect_status_should_return_none() {
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([128, 53, 10, 0]))));
    }

    #[test]
    fn map_event_to_index_given_low_velocity_should_return_none() {
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 53, 0, 0]))));
    }

    #[test]
    fn map_event_to_index_given_out_of_grid_value_should_return_none() {
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 00, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 01, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 08, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 08, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 10, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 19, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 80, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 89, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 90, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 91, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 98, 10, 0]))));
        assert_eq!(None, map_event_to_index(MidiEvent::from(MidiMessage::from([144, 99, 10, 0]))));
    }

    #[test]
    fn map_event_to_index_should_correct_value() {
        let actual_output = vec![
            81, 82, 83, 84, 85, 86, 87, 88,
            71, 72, 73, 74, 75, 76, 77, 78,
            61, 62, 63, 64, 65, 66, 67, 68,
            51, 52, 53, 54, 55, 56, 57, 58,
            41, 42, 43, 44, 45, 46, 47, 48,
            31, 32, 33, 34, 35, 36, 37, 38,
            21, 22, 23, 24, 25, 26, 27, 28,
            11, 12, 13, 14, 15, 16, 17, 18,
        ]
            .iter()
            .map(|code| map_event_to_index(MidiEvent::from(MidiMessage::from([144, *code, 10, 0]))))
            .collect::<Vec<Option<u16>>>();

        let expected_output = vec![
            56, 57, 58, 59, 60, 61, 62, 63,
            48, 49, 50, 51, 52, 53, 54, 55,
            40, 41, 42, 43, 44, 45, 46, 47,
            32, 33, 34, 35, 36, 37, 38, 39,
            24, 25, 26, 27, 28, 29, 30, 31,
            16, 17, 18, 19, 20, 21, 22, 23,
            08, 09, 10, 11, 12, 13, 14, 15,
            00, 01, 02, 03, 04, 05, 06, 07,
        ]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<u16>>>();

        assert_eq!(expected_output, actual_output);
    }

    #[test]
    #[cfg(feature = "launchpadpro")]
    fn render_rainbow() {
        use crate::midi::Connections;

        let connections = Connections::new().unwrap();
        let ports = connections.create_bidirectional_ports(&"Launchpad Pro Standalone Port".to_string());
        match ports {
            Ok(ports) => {
                let mut launchpadpro = LaunchpadPro::from(ports);
                let mut pixels = vec![Pixel { r: 0, g: 0, b: 0 }; 64];

                for n in 0..64 {
                    let row: u16 = n / 8;
                    let column: u16 = n % 8;
                    let index = row + column;
                    pixels[n as usize] = Pixel {
                        r: 255 - (index * 255 / 14) as u8,
                        g: 0,
                        b: (index * 255 / 14) as u8,
                    };
                }

                let result = launchpadpro.render(pixels);
                assert!(result.is_ok(), "The LaunchpadPro could not render the given image");
            },
            Err(_) => {
                println!("The LaunchpadPro device may not be connected correctly");
            }
        }
    }
}
