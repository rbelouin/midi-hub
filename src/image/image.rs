use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

extern crate jpeg_decoder;
use jpeg_decoder::{Decoder, PixelFormat};

use super::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

impl Image {
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
