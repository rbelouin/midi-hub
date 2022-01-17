extern crate jpeg_decoder;

use std::io::Read;
use jpeg_decoder::Decoder;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub async fn compress_from_url<A, F: FnOnce(u16, u16, Vec<Pixel>) -> Result<A, String>>(url: String, algo: F) -> Result<A, String> {
    let client = reqwest::Client::new();

    println!("[Image] Fetching and compressing {}", url);
    let response = client.get(url)
        .send()
        .await
        .map_err(|err| format!("{}", err))?;

    let bytes = response.bytes()
        .await
        .map_err(|err| format!("{}", err))?;

    let mut decoder = Decoder::new(bytes.as_ref());
    return compress_from_decoder(&mut decoder, algo);
}

pub fn compress_from_decoder<A, R: Read, F: FnOnce(u16, u16, Vec<Pixel>) -> Result<A, String>>(decoder: &mut Decoder<R>, algo: F) -> Result<A, String> { 
    return match decoder.decode() {
        Err(error) => Err(format!("Could not decode the pixels from the given picture: {:?}", error)),
        Ok(pixels) => {
            let mut output = vec![];
            let mut pixel = Pixel { r: 0, g: 0, b: 0 };
            for i in 0..pixels.len() {
                match i % 3 {
                    0 => {
                        pixel = Pixel { r: pixels[i], g: 0, b: 0 };
                    },
                    1 => {
                        pixel.g = pixels[i];
                    },
                    _ => {
                        pixel.b = pixels[i];
                        output.push(pixel.clone());
                    },
                };
            }
            // Assume the pictures have to be 64x64 for now
            return algo(64, 64, output);
        },
    };
}

pub fn compress_8x8(width: u16, height: u16, pixels: Vec<Pixel>) -> Result<Vec<Pixel>, String> {
    if (width as u32) * (height as u32) != pixels.len() as u32 {
        return Err(format!("Number of pixels ({}) not matching width ({}) multiplied by height ({})", pixels.len(), width, height));
    }

    let mut output = vec![Pixel { r: 0, g: 0, b: 0 }; 64];
    for new_y in 0..8u32 {
        for new_x in 0..8u32 {
            let mut section = vec![];
            for y in (new_y * (height as u32) / 8)..((new_y + 1) * (height as u32) / 8) {
                for x in (new_x * (width as u32) / 8)..((new_x + 1) * (width as u32) / 8) {
                    section.push(pixels[((height as u32) * y + x) as usize]);
                }
            }
            output[(new_y * 8 + new_x) as usize] = compress(section);
        }
    }

    return Ok(output);
}

pub fn compress_1x1(_width: u16, _height: u16, pixels: Vec<Pixel>) -> Result<Pixel, String> {
    return Ok(compress(pixels));
}

pub fn compress(pixels: Vec<Pixel>) -> Pixel {
    if pixels.is_empty() {
        return Pixel { r: 0, g: 0, b: 0 };
    }

    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;

    for i in 0..pixels.len() {
        r += pixels[i].r as u32;
        g += pixels[i].g as u32;
        b += pixels[i].b as u32;
    }

    let len = pixels.len() as u32;

    return Pixel {
        r: (r / len) as u8,
        g: (g / len) as u8,
        b: (b / len) as u8,
    };
}

#[cfg(test)]
mod tests {
    extern crate insta;
    extern crate jpeg_encoder;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use std::fs::File;
    use std::io::BufReader;
    use jpeg_encoder::{Encoder, ColorType};

    // This test relies on network calls, on Spotify’s CDN being up, and on the album cover not to
    // change. There’s a risk it becomes flaky, but I’ll keep it until the cost/benefit balance
    // becomes bad.
    #[test]
    fn test_compress_from_url() {
        let rt  =  tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result =  compress_from_url(String::from("https://i.scdn.co/image/ab67616d00004851ab640839fdacc8f8f4c20ac6"), compress_8x8).await;
            insta::assert_debug_snapshot!(result);

            let encoder = Encoder::new_file("src/image/test-ab67616d00004851ab640839fdacc8f8f4c20ac6.jpg", 100).unwrap();
            let data: Vec<u8> = result.unwrap().iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect();
            let _ = encoder.encode(data.as_ref(), 8, 8, ColorType::Rgb);
        });
    }

    #[test]
    fn test_compress_from_decoder() {
        let file = File::open("src/image/test-cover.jpg").expect("failed to open picture");
        let mut decoder = Decoder::new(BufReader::new(file));
        let result = compress_from_decoder(&mut decoder, compress_8x8);
        insta::assert_debug_snapshot!(result);

        let encoder = Encoder::new_file("src/image/test-cover-output.jpg", 100).unwrap();
        let data: Vec<u8> = result.unwrap().iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect();
        let _ = encoder.encode(data.as_ref(), 8, 8, ColorType::Rgb);
    }

    #[test]
    fn test_compress_8x8_invalid_input_should_return_error() {
        let result = compress_8x8(16, 16, vec![]);
        assert!(result.is_err(), "result should be an error, got {:?}", result);
    }

    #[test]
    fn test_compress_8x8_valid_input_should_compress_picture() {
        // black
        let b = Pixel { r: 0, g: 0, b: 0 };
        // white
        let w = Pixel { r: 255, g: 255, b: 255 };
        // grey
        let g = Pixel { r: 127, g: 127, b: 127 };

        let input = vec![
            b, b, b, b, b, b, b, b, b, b, b, b, b, b, b, b,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            w, w, w, w, w, w, w, w, w, w, w, w, w, w, w, w,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            b, b, b, b, b, b, b, b, b, b, b, b, b, b, b, b,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            w, w, w, w, w, w, w, w, w, w, w, w, w, w, w, w,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            b, b, b, b, b, b, b, b, b, b, b, b, b, b, b, b,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            w, w, w, w, w, w, w, w, w, w, w, w, w, w, w, w,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            b, b, b, b, b, b, b, b, b, b, b, b, b, b, b, b,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
            w, w, w, w, w, w, w, w, w, w, w, w, w, w, w, w,
            w, w, b, b, w, w, b, b, w, w, b, b, w, w, b, b,
        ];

        let expected_output = vec![
            g, b, g, b, g, b, g, b,
            w, g, w, g, w, g, w, g,
            g, b, g, b, g, b, g, b,
            w, g, w, g, w, g, w, g,
            g, b, g, b, g, b, g, b,
            w, g, w, g, w, g, w, g,
            g, b, g, b, g, b, g, b,
            w, g, w, g, w, g, w, g,
        ];

        assert_eq!(compress_8x8(16, 16, input), Ok(expected_output));
    }

    #[test]
    fn test_compress_no_pixels_should_return_black() {
        assert_eq!(compress(vec![]), Pixel { r: 0, g: 0, b: 0 });
    }

    #[test]
    fn test_compress_monochrome_pixels_return_same_pixel() {
        let count = rand::random::<usize>() % 1023 + 1;
        let pixel = Pixel {
            r: rand::random::<u8>(),
            g: rand::random::<u8>(),
            b: rand::random::<u8>(),
        };
        let pixels = vec![pixel; count];

        assert_eq!(compress(pixels), pixel);
    }

    #[test]
    fn test_compress_shades_of_red_return_shade_of_red() {
        let count = rand::random::<usize>() % 1023 + 1;
        let mut pixels = vec![Pixel { r: 0, g: 0, b: 0 }; count];
        pixels[0].r = 30;
        for i in 1..pixels.len() {
            pixels[i].r = rand::random::<u8>();
        }

        let compressed_pixel = compress(pixels);
        assert!(compressed_pixel.r > 0, "red channel should be greater than zero, got {}", compressed_pixel.r);
        assert_eq!(compressed_pixel.g, 0);
        assert_eq!(compressed_pixel.b, 0);
    }

    #[test]
    fn test_compress_shades_of_green_return_shade_of_green() {
        let count = rand::random::<usize>() % 1023 + 1;
        let mut pixels = vec![Pixel { r: 0, g: 0, b: 0 }; count];
        pixels[0].g = 30;
        for i in 1..pixels.len() {
            pixels[i].g = rand::random::<u8>();
        }

        let compressed_pixel = compress(pixels);
        assert_eq!(compressed_pixel.r, 0);
        assert!(compressed_pixel.g > 0, "green channel should be greater than zero, got {}", compressed_pixel.g);
        assert_eq!(compressed_pixel.b, 0);
    }

    #[test]
    fn test_compress_shades_of_blue_return_shade_of_blue() {
        let count = rand::random::<usize>() % 1023 + 1;
        let mut pixels = vec![Pixel { r: 0, g: 0, b: 0 }; count];
        pixels[0].b = 30;
        for i in 1..pixels.len() {
            pixels[i].b = rand::random::<u8>();
        }

        let compressed_pixel = compress(pixels);
        assert_eq!(compressed_pixel.r, 0);
        assert_eq!(compressed_pixel.g, 0);
        assert!(compressed_pixel.b > 0, "blue channel should be greater than zero, got {}", compressed_pixel.b);
    }

    #[test]
    fn test_compress_return_between_extreme_values() {
        let mut pixels = vec![Pixel { r: 0, g: 0, b: 0 }; 2];

        let mut min_r = u8::MAX;
        let mut max_r = u8::MIN;
        let mut min_g = u8::MAX;
        let mut max_g = u8::MIN;
        let mut min_b = u8::MAX;
        let mut max_b = u8::MIN;

        for i in 0..pixels.len() {
            pixels[i].r = rand::random::<u8>();
            pixels[i].g = rand::random::<u8>();
            pixels[i].b = rand::random::<u8>();

            if pixels[i].r < min_r {
                min_r = pixels[i].r;
            }

            if pixels[i].r > max_r {
                max_r = pixels[i].r;
            }

            if pixels[i].g < min_g {
                min_g = pixels[i].g;
            }

            if pixels[i].g > max_g {
                max_g = pixels[i].g;
            }

            if pixels[i].b < min_b {
                min_b = pixels[i].b;
            }

            if pixels[i].b > max_b {
                max_b = pixels[i].b;
            }
        }

        let compressed_pixel = compress(pixels);
        assert!(min_r <= compressed_pixel.r && max_r >= compressed_pixel.r, "red channel should be in [{}; {}], got {}", min_r, max_r, compressed_pixel.r);
        assert!(min_g <= compressed_pixel.g && max_g >= compressed_pixel.g, "green channel should be in [{}; {}], got {}", min_g, max_g, compressed_pixel.g);
        assert!(min_b <= compressed_pixel.b && max_b >= compressed_pixel.b, "blue channel should be in [{}; {}], got {}", min_b, max_b, compressed_pixel.b);
    }
}