use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

extern crate jpeg_decoder;
use jpeg_decoder::{Decoder, PixelFormat};

use super::{Error, Pixel};

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

impl Image {
    pub fn from(width: u16, height: u16, pixels: Vec<Pixel>) -> Image {
        let bytes = &mut Vec::with_capacity(pixels.len() * 3);
        for pixel in pixels {
            bytes.push(pixel.r);
            bytes.push(pixel.g);
            bytes.push(pixel.b);
        }

        return Image { width: width.into(), height: height.into(), bytes: bytes.to_vec() };
    }

    pub fn from_decoder<R: Read>(decoder: &mut Decoder<R>) -> Result<Image, Error> {
        let bytes = decoder.decode().map_err(|_| Error::JpegDecodingError)?;
        let info = decoder.info().ok_or(Error::JpegInfoError)?;
        if info.pixel_format != PixelFormat::RGB24 {
            return Err(Error::JpegPixelFormatError);
        }
        return Ok(Image {
            width: info.width.into(),
            height: info.height.into(),
            bytes,
        });
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Image, Error> {
        let file = File::open(path).map_err(|_| Error::FileOpenError)?;
        let mut decoder = Decoder::new(BufReader::new(file));
        return Image::from_decoder(&mut decoder);
    }

    pub async fn from_url(url: &String) -> Result<Image, Error> {
        let client = reqwest::Client::new();
        let response = client.get(url)
            .send()
            .await
            .map_err(|_| Error::HttpRequestError)?;

        let bytes = response.bytes()
            .await
            .map_err(|_| Error::HttpParseError)?;

        let mut decoder = Decoder::new(bytes.as_ref());
        return Image::from_decoder(&mut decoder);
    }
}

#[cfg(test)]
pub mod tests {
    use std::fs::File;
    use super::*;

    #[test]
    fn test_from_pixels() {
        let output = Image::from(3, 3, vec![
            Pixel { r: 00, g: 00, b: 00 }, Pixel { r: 10, g: 10, b: 10 }, Pixel { r: 20, g: 20, b: 20 },
            Pixel { r: 30, g: 30, b: 30 }, Pixel { r: 40, g: 40, b: 40 }, Pixel { r: 50, g: 50, b: 50 },
            Pixel { r: 60, g: 60, b: 60 }, Pixel { r: 70, g: 70, b: 70 }, Pixel { r: 80, g: 80, b: 80 },
        ]);

        assert_eq!(output, Image { width: 3, height: 3, bytes: vec![
            0, 0, 0, 10, 10, 10, 20, 20, 20, 30, 30, 30, 40, 40, 40, 50, 50, 50, 60, 60, 60, 70, 70, 70, 80, 80, 80
        ] });
    }

    pub fn given_cover_image_decoder() -> Decoder<BufReader<File>> {
        let file = File::open(Path::new(file!()).with_file_name("test/cover.jpg")).expect("failed to open picture");
        return Decoder::new(BufReader::new(file));
    }

    pub fn given_random_image_decoder() -> Decoder<BufReader<File>> {
        let file = File::open(Path::new(file!()).with_file_name("test/random.jpg")).expect("failed to open picture");
        return Decoder::new(BufReader::new(file));
    }

    #[test]
    fn test_from_decoder_given_cover_image_should_return_correct_width() {
        let mut decoder = given_cover_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.width, 64, "Expected the resulting image to have a width of 64px");
    }

    #[test]
    fn test_from_decoder_given_random_image_should_return_correct_width() {
        let mut decoder = given_random_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.width, 240, "Expected the resulting image to have a width of 240px");
    }

    #[test]
    fn test_from_decoder_given_cover_image_should_return_correct_height() {
        let mut decoder = given_cover_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.height, 64, "Expected the resulting image to have a height of 64px");
    }

    #[test]
    fn test_from_decoder_given_random_image_should_return_correct_height() {
        let mut decoder = given_random_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.height, 240, "Expected the resulting image to have a height of 240px");
    }

    #[test]
    fn test_from_decoder_given_cover_image_should_return_correct_number_of_bytes() {
        let mut decoder = given_cover_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.bytes.len(), 64 * 64 * 3, "Expected the resulting image to have 3 bytes per pixel, and 64×64 pixels");
    }

    #[test]
    fn test_from_decoder_given_random_image_should_return_correct_number_of_bytes() {
        let mut decoder = given_random_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert_eq!(image.bytes.len(), 240 * 240 * 3, "Expected the resulting image to have 3 bytes per pixel, and 240×240 pixels");
    }

    #[test]
    fn test_from_decoder_given_cover_image_should_return_image_with_non_zero_bytes() {
        let mut decoder = given_cover_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert!(image.bytes.into_iter().any(|byte| byte != 0), "Expected the resulting image to contain some non-zero bytes");
    }

    #[test]
    fn test_from_decoder_given_random_image_should_return_image_with_non_zero_bytes() {
        let mut decoder = given_random_image_decoder();
        let image = Image::from_decoder(&mut decoder).expect("Expected Image::from_decoder to succeed");
        assert!(image.bytes.into_iter().any(|byte| byte != 0), "Expected the resulting image to contain some non-zero bytes");
    }

    #[test]
    fn test_from_url_given_local_copy_should_return_same_image() {
        let rt  =  tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut decoder = given_cover_image_decoder();
            let local_image = Image::from_decoder(&mut decoder);
            let url = "https://i.scdn.co/image/ab67616d00004851a5c51e96d2583bfb3e45d504".to_string();
            let remote_image = Image::from_url(&url).await;
            assert_eq!(local_image, remote_image, "Expected the resulting image to match the local copy");
        });
    }
}
