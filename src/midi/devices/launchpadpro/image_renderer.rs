use std::error::Error as StdError;
use std::fmt::{Display, Error, Formatter};

use crate::image::{Image, scale};
use crate::midi::Event;
use crate::midi::features::{R, GridController, ImageRenderer};

use super::device::LaunchpadProFeatures;

#[derive(Debug)]
struct UnexpectedNumberOfBytes {
    actual_bytes: usize,
    expected_bytes: usize,
}

impl StdError for UnexpectedNumberOfBytes {}
impl Display for UnexpectedNumberOfBytes {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "expected number of bytes: {}; got: {}", self.expected_bytes, self.actual_bytes)
    }
}

impl ImageRenderer for LaunchpadProFeatures {
    fn from_image(&self, image: Image) -> R<Event> {
        let (width, height) = self.get_grid_size()?;
        let scaled_image = scale(&image, width, height)
            .map_err(|err| {
                let err: Box<dyn StdError + Send> = Box::new(err);
                return err;
            })?;
        return self.render_24bit_image_reversed(scaled_image.bytes);
    }
}

impl LaunchpadProFeatures {
    fn get_size(&self) -> R<usize> {
        let (width, height) = self.get_grid_size()?;
        // one byte for each red/green/blue color
        return Ok(width * height * 3);
    }

    /// The LaunchpadPro’s coordinate system places the origin at the bottom-left corner, so we
    /// need to give an easy option to render an image with (0,0) being the top-left corner.
    fn render_24bit_image_reversed(&self, bytes: Vec<u8>) -> R<Event> {
        let reversed_bytes = self.reverse_rows(bytes)?;
        return self.render_24bit_image(reversed_bytes);
    }

    fn render_24bit_image(&self, bytes: Vec<u8>) -> R<Event> {
        let size = self.get_size()?;

        if bytes.len() != size {
            return Err(Box::new(UnexpectedNumberOfBytes { actual_bytes: bytes.len(), expected_bytes: size }));
        }

        let mut picture = Vec::with_capacity(size);
        picture.append(&mut vec![240, 0, 32, 41, 2, 16, 15, 1]);
        for byte in bytes {
            // The LaunchpadPro also only supports values from the [0; 64[ range, so we need to make sure
            // that our 24-bit-RGB-color bytes get transformed.
            picture.push(byte / 4);
        }
        picture.append(&mut vec![247]);

        return Ok(Event::SysEx(picture));
    }

    fn reverse_rows(&self, bytes: Vec<u8>) -> R<Vec<u8>> {
        let (width, height) = self.get_grid_size()?;
        let size = self.get_size()?;

        if bytes.len() != size {
            return Err(Box::new(UnexpectedNumberOfBytes { actual_bytes: bytes.len(), expected_bytes: size }));
        }

        let mut reversed_bytes = vec![0; size];

        for y in 0..height {
            for x in 0..width {
                for c in 0..3 {
                    reversed_bytes[3 * (y * width + x) + c] = bytes[3 * ((height - 1 - y) * width + x) + c];
                }
            }
        }

        return Ok(reversed_bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_rows() {
        let features = super::super::LaunchpadProFeatures::new();
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

        let actual_output = features.reverse_rows(input).expect("Test input is expected to be valid");
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
    fn test_from_image_should_reverse_rows_and_divide_color_values_by_four() {
        let features = super::super::LaunchpadProFeatures::new();

        // This image will be scaled to fit on a 8x8 grid
        let image = Image { width: 16, height: 16, bytes: vec![
            Vec::from([000; 16 * 3]),
            Vec::from([000; 16 * 3]),
            Vec::from([032; 16 * 3]),
            Vec::from([032; 16 * 3]),
            Vec::from([064; 16 * 3]),
            Vec::from([064; 16 * 3]),
            Vec::from([096; 16 * 3]),
            Vec::from([096; 16 * 3]),
            Vec::from([128; 16 * 3]),
            Vec::from([128; 16 * 3]),
            Vec::from([160; 16 * 3]),
            Vec::from([160; 16 * 3]),
            Vec::from([192; 16 * 3]),
            Vec::from([192; 16 * 3]),
            Vec::from([224; 16 * 3]),
            Vec::from([224; 16 * 3]),
        ].concat() };

        let event = features.from_image(image).unwrap();
        assert_eq!(event, Event::SysEx(vec![
            // Launchpad Pro prefix for lighting pixels
            Vec::from([240, 0, 32, 41, 2, 16, 15, 1]),
            // Bottom row should be light
            Vec::from([56; 8 * 3]),
            // And rows should get darker and darker...
            Vec::from([48; 8 * 3]),
            Vec::from([40; 8 * 3]),
            Vec::from([32; 8 * 3]),
            Vec::from([24; 8 * 3]),
            Vec::from([16; 8 * 3]),
            Vec::from([08; 8 * 3]),
            // And the top one should be black
            Vec::from([00; 8 * 3]),
            // Launchpad Pro suffix at the end of SysEx events
            Vec::from([247]),
        ].concat()));
    }
}
