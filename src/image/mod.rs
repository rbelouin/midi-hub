extern crate jpeg_decoder;

use std::convert::{From, Into};
use std::io::Read;
use jpeg_decoder::Decoder;

mod scale;
use scale::scale;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

impl Image {
    fn from(width: u16, height: u16, pixels: Vec<Pixel>) -> Image {
        let bytes = &mut Vec::with_capacity(pixels.len() * 3);
        for pixel in pixels {
            bytes.push(pixel.r);
            bytes.push(pixel.g);
            bytes.push(pixel.b);
        }

        return Image { width: width.into(), height: height.into(), bytes: bytes.to_vec() };
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<&Pixel> for [u8; 3] {
    fn from(pixel: &Pixel) -> [u8; 3] {
        return [pixel.r, pixel.g, pixel.b];
    }
}

impl From<[u8; 3]> for Pixel {
    fn from(bytes: [u8; 3]) -> Pixel {
        return Pixel { r: bytes[0], g: bytes[1], b: bytes[2] };
    }
}

impl From<Image> for Vec<Pixel> {
    fn from(image: Image) -> Vec<Pixel> {
        let mut pixels = Vec::with_capacity(image.width * image.height);
        let mut pixel = Pixel { r: 0, g: 0, b: 0 };
        for n in 0..image.bytes.len() {
            match n % 3 {
                0 => { pixel.r = image.bytes[n]; },
                1 => { pixel.g = image.bytes[n]; },
                _ => {
                    pixel.b = image.bytes[n];
                    pixels.push(pixel.clone());
                },
            }
        }
        return pixels;
    }
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
    return scale(&Image::from(width, height, pixels), 8, 8).map_err(|err| format!("Error: {:?}", err))
        .map(|image| Vec::from(image));
}

pub fn compress_1x1(width: u16, height: u16, pixels: Vec<Pixel>) -> Result<Pixel, String> {
    return scale(&Image::from(width, height, pixels), 1, 1).map_err(|err| format!("Error: {:?}", err))
        .map(|image| Vec::from(image)[0]);
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
}
