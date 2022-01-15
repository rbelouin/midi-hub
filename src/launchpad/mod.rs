extern crate portmidi as pm;

use portmidi::OutputPort;
use crate::image::Pixel;

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

pub fn render_pixels(output_port: &OutputPort, pixels: Vec<Pixel>) {
    if pixels.len() != 64 {
        println!("Error: the number of pixels is not 64: {}", pixels.len());
        return;
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

    match output_port.write_sysex(0, &picture) {
        Ok(()) => println!("Worked!"),
        Err(e) => println!("Error: {}",  e),
    }
}
