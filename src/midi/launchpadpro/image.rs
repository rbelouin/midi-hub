use crate::image::{Image, scale};
use crate::midi::{Error, Event};

const WIDTH: usize = 8;
const HEIGHT: usize = 8;
const SIZE: usize = WIDTH * HEIGHT;

pub fn from_image(image: Image) -> Result<Event, Error> {
    let scaled_image = scale(&image, WIDTH, HEIGHT).map_err(|_| Error::ImageRenderError)?;
    return render_24bit_image_reversed(scaled_image.bytes);
}

/// The LaunchpadPro’s coordinate system places the origin at the bottom-left corner, so we
/// need to give an easy option to render an image with (0,0) being the top-left corner.
fn render_24bit_image_reversed(bytes: Vec<u8>) -> Result<Event, Error> {
    let reversed_bytes = reverse_rows(bytes)?;
    return render_24bit_image(reversed_bytes);
}

fn render_24bit_image(bytes: Vec<u8>) -> Result<Event, Error> {
    // one byte for each color
    let size = SIZE * 3;

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

    return Ok(Event::SysEx(picture));
}

fn reverse_rows(bytes: Vec<u8>) -> Result<Vec<u8>, Error> {
    // one byte for each color
    let size = SIZE * 3;

    if bytes.len() != size {
        println!("[launchpadpro] error when trying to render an image with {} bytes", bytes.len());
        return Err(Error::ImageRenderError);
    }

    let mut reversed_bytes = vec![0; size];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            for c in 0..3 {
                reversed_bytes[3 * (y * WIDTH + x) + c] = bytes[3 * ((HEIGHT - 1 - y) * WIDTH + x) + c];
            }
        }
    }

    return Ok(reversed_bytes);
}

#[cfg(test)]
mod tests {
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

        let actual_output = reverse_rows(input).expect("Test input is expected to be valid");
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
}
