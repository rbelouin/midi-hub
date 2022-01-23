use std::convert::From;
use crate::image::{Image, scale};
use super::{InputPort, OutputPort, MidiEvent, Error, Reader, Writer, ImageRenderer, IndexReader};

pub struct LaunchpadPro<'a> {
    input_port: InputPort<'a>,
    output_port: OutputPort<'a>,
}

impl LaunchpadPro<'_> {
    pub const WIDTH: usize = 8;
    pub const HEIGHT: usize = 8;
    pub const SIZE: usize = Self::WIDTH * Self::HEIGHT;

    fn render_one_image(&mut self, image: &Image) -> Result<(), Error> {
        let scaled_image = scale(image, Self::WIDTH, Self::HEIGHT).map_err(|_| Error::ImageRenderError)?;
        return self.render_24bit_image_reversed(scaled_image.bytes);
    }

    fn render_one_image_per_pad(&mut self, images: &Vec<Image>) -> Result<(), Error> {
        let fallback_pixel = Image { width: 1, height: 1, bytes: vec![0; 3] };
        let mosaic = images.into_iter()
            .map(|image| scale(image, 1, 1).unwrap_or(fallback_pixel.clone()))
            .flat_map(|image| image.bytes)
            .collect::<Vec<u8>>();
        return self.render_24bit_image(mosaic);
    }

    /// The LaunchpadPro’s coordinate system places the origin at the bottom-left corner, so we
    /// need to give an easy option to render an image with (0,0) being the top-left corner.
    fn render_24bit_image_reversed(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        let reversed_bytes = Self::reverse_rows(bytes)?;
        return self.render_24bit_image(reversed_bytes);
    }

    fn render_24bit_image(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        // one byte for each color
        let size = Self::SIZE * 3;

        if bytes.len() != size {
            println!("[launchpadpro] error when trying to render an image with {} bytes", bytes.len());
            return Err(Error::ImageRenderError);
        }

        let mut picture = Vec::with_capacity(size);
        picture.append(&mut vec![240, 0, 32, 41, 2, 16, 15, 1]);
        for byte in bytes {
            // The LaunchpadPro also only supports values from the [0; 64[ range, so we need to make sure
            // that our 24-bit-RGB-color bytes get transformed.
            picture.push(byte / 4);
        }
        picture.append(&mut vec![247]);

        return self.output_port.write_sysex(0, &picture)
            .map_err(|_| Error::ImageRenderError);
    }

    fn reverse_rows(bytes: Vec<u8>) -> Result<Vec<u8>, Error> {
        // one byte for each color
        let size = Self::SIZE * 3;

        if bytes.len() != size {
            println!("[launchpadpro] error when trying to render an image with {} bytes", bytes.len());
            return Err(Error::ImageRenderError);
        }

        let mut reversed_bytes = vec![0; size];

        for y in 0..Self::HEIGHT {
            for x in 0..Self::WIDTH {
                for c in 0..3 {
                    reversed_bytes[3 * (y * Self::WIDTH + x) + c] = bytes[3 * ((Self::HEIGHT - 1 - y) * Self::WIDTH + x) + c];
                }
            }
        }

        return Ok(reversed_bytes);
    }
}

impl<'a> From<(InputPort<'a>, OutputPort<'a>)> for LaunchpadPro<'a> {
    fn from(ports: (InputPort<'a>, OutputPort<'a>)) -> LaunchpadPro<'a> {
        return LaunchpadPro { input_port: ports.0, output_port: ports.1 };
    }
}

impl Reader for LaunchpadPro<'_> {
    fn read(&mut self) -> Result<Option<MidiEvent>, Error> {
        let event = Reader::read(&mut self.input_port);
        match event {
            Ok(Some(event)) => println!("event: {:?}", event),
            _ => {},
        }
        return event;
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

impl ImageRenderer<Vec<Image>> for LaunchpadPro<'_> {
    fn render(&mut self, images: Vec<Image>) -> Result<(), Error> {
        return match images.len() {
            1 => self.render_one_image(&images[0]),
            Self::SIZE => self.render_one_image_per_pad(&images),
            _ => {
                println!("[launchpadpro] unsupported number of images: {}", images.len());
                return Err(Error::ImageRenderError);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::MidiMessage;
    use super::*;

    #[test]
    fn test_reverse_rows() {
        let input = vec![
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
            1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
            3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,
            4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,
            5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,
            6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,
            7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
        ];

        let actual_output = LaunchpadPro::reverse_rows(input).expect("Test input is expected to be valid");
        assert_eq!(actual_output, vec![
            7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
            6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,
            5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,
            4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,4,
            3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,
            2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
            1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
            0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
        ]);
    }

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
                let mut bytes = vec![0u8; LaunchpadPro::SIZE * 3];

                for y in 0..LaunchpadPro::HEIGHT {
                    for x in 0..LaunchpadPro::WIDTH {
                        let index = x + y;
                        let max = (LaunchpadPro::WIDTH - 1) + (LaunchpadPro::HEIGHT - 1);
                        bytes[3 * (y * LaunchpadPro::WIDTH + x) + 0] = (255 - 255 * index / max) as u8;
                        bytes[3 * (y * LaunchpadPro::WIDTH + x) + 1] = 0;
                        bytes[3 * (y * LaunchpadPro::WIDTH + x) + 2] = (255 * index / max) as u8;
                    }
                }

                let result = launchpadpro.render_24bit_image_reversed(bytes);
                assert!(result.is_ok(), "The LaunchpadPro could not render the given image");
            },
            Err(_) => {
                println!("The LaunchpadPro device may not be connected correctly");
            }
        }
    }
}
