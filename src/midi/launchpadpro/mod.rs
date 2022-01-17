use std::convert::From;
use crate::image::Pixel;
use super::{InputPort, OutputPort, MidiEvent, Error, Reader, Writer, ImageRenderer};

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
#[cfg(feature = "launchpadpro")]
mod tests {
    use crate::midi::Connections;
    use super::*;

    #[test]
    fn render_rainbow() {
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
