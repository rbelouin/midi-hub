use std::convert::From;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use super::Image;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    InvalidScaleForImage(usize, usize, usize, usize),
    InvalidImage(usize, usize),
}

impl StdError for Error {}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Error::InvalidScaleForImage(new_w, new_h, old_w, old_h) =>
                write!(
                    f,
                    "invalid scale (width: {}, height: {}) for image (width: {}, height: {})",
                    new_w,
                    new_h,
                    old_w,
                    old_h),
            Error::InvalidImage(w, h) =>
                write!(f, "invalid image (width: {}, height: {})", w, h),
        }
    }
}

/// Coordinate1D is a pointer to a byte in an image perceived as a one-dimensional array of bytes.
///
/// Example given:
/// ╔════╦════╦════╦════╦════╦══
/// ║ B0 ║ B1 ║ B2 ║ B3 ║ B4 ║ …
/// ╚════╩════╩════╩════╩════╩══
///                  ↑ index = 3
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coordinate1D<'a> {
    image: &'a Image,
    index: usize,
}

/// Coordinate3D is a pointer to a byte in an image perceived as a three-dimensional collection of
/// bytes (`x` for horizontal position, `y` for vertical position, and `color` for RGB channel).
/// Example given:
///
/// ╔════╤════╤════╦════╤════╤══
/// ║0,0R│0,0G│0,0B║0,1R│0,1G│ …
/// ╠════╪════╪════╬════╪════╪══
/// ║1,0R│1,0G│1,0B║1,1R│1,1G│ …
/// ╚════╧════╧════╩════╧════╧══
///             ↑ color=2,x=0,y=1
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coordinate3D<'a> {
    image: &'a Image,
    color: usize,
    x: usize,
    y: usize,
}

impl Coordinate3D<'_> {
    fn scale_to<'a, 'b>(&'a self, image: &'b Image) -> Coordinate3D<'b> {
        let x = self.x * image.width / self.image.width;
        let y = self.y * image.height / self.image.height;
        return Coordinate3D { image, color: self.color, x, y };
    }
}

impl<'a> From<Coordinate1D<'a>> for Coordinate3D<'a> {
    fn from(coordinate_1d: Coordinate1D) -> Coordinate3D {
        let Coordinate1D { image, index } = coordinate_1d;
        let color = index % 3;
        let y = (index / 3) / image.width;
        let x = (index / 3) % image.width;
        return Coordinate3D { image, color, x, y };
    }
}

impl<'a> From<Coordinate3D<'a>> for Coordinate1D<'a> {
    fn from(coordinate_3d: Coordinate3D) -> Coordinate1D {
        let Coordinate3D { image, color, x, y } = coordinate_3d;
        let index = 3 * (y * image.width + x) + color;
        return Coordinate1D { image, index };
    }
}

pub fn scale(image: &Image, new_width: usize, new_height: usize) -> Result<Image, Error> {
    let _ = validate_scale_arguments(&image, new_width, new_height)?;

    // Instantiate two vectors of the size of the future image.
    // One that counts the bytes that will be merged together,
    // and the other that sums their values.
    let new_size = 3 * new_width * new_height;
    let mut bytes_counts = Vec::with_capacity(new_size);
    let mut bytes_sums = Vec::with_capacity(new_size);
    for _ in 0..new_size {
        bytes_counts.push(0usize);
        bytes_sums.push(0usize);
    }

    // Prepare the image to be returned.
    let mut new_image = Image {
        width: new_width,
        height: new_height,
        bytes: Vec::with_capacity(new_size),
    };

    // Determine what will the position of the given byte be on the scaled image,
    // and assign it to the corresponding `bytes_counts` and  `bytes_sums`.
    for index in 0..image.bytes.len() {
        let coordinate_3d = Coordinate3D::from(Coordinate1D { image: &image, index });
        let new_coordinate_3d = coordinate_3d.scale_to(&new_image);
        let new_coordinate_1d = Coordinate1D::from(new_coordinate_3d);
        bytes_counts[new_coordinate_1d.index] += 1;
        bytes_sums[new_coordinate_1d.index] += usize::from(image.bytes[index]);
    }

    // Finally, for each "new" byte, calculate the average value of the old bytes assigned to it.
    for index in 0..new_image.bytes.capacity() {
        new_image.bytes.push((bytes_sums[index] / bytes_counts[index]) as u8);
    }

    return Ok(new_image);
}

fn validate_scale_arguments(image: &Image, new_width: usize, new_height: usize) -> Result<(), Error> {
    // The algorithm only knows how to shrink an image for now
    if new_width > image.width
    || new_width == 0
    || new_height > image.height
    || new_height == 0 {
        return Err(Error::InvalidScaleForImage(new_width, new_height, image.width, image.height));
    }

    // Make sure that the number of bytes matches the claimed dimensions of the given image.
    if 3 * image.width * image.height != image.bytes.len() {
        return Err(Error::InvalidImage(3* image.width * image.height, image.bytes.len()));
    }

    return Ok(());
}

#[cfg(test)]
mod test {
    use rand::random;
    use super::*;

    #[test]
    fn test_coordinate_conversions() {
        let image = Image { width: 3, height: 4, bytes: Vec::with_capacity(12) };

        // Each tuple contains the expected (y, x, color) for the corresponding index.
        let bytes = vec![
            (0, 0, 0), (0, 0, 1), (0, 0, 2), (0, 1, 0), (0, 1, 1), (0, 1, 2), (0, 2, 0), (0, 2, 1), (0, 2, 2),
            (1, 0, 0), (1, 0, 1), (1, 0, 2), (1, 1, 0), (1, 1, 1), (1, 1, 2), (1, 2, 0), (1, 2, 1), (1, 2, 2),
            (2, 0, 0), (2, 0, 1), (2, 0, 2), (2, 1, 0), (2, 1, 1), (2, 1, 2), (2, 2, 0), (2, 2, 1), (2, 2, 2),
            (3, 0, 0), (3, 0, 1), (3, 0, 2), (3, 1, 0), (3, 1, 1), (3, 1, 2), (3, 2, 0), (3, 2, 1), (3, 2, 2),
        ];

        for index in 0..bytes.len() {
            let coordinate_1d = Coordinate1D { image: &image, index };
            let coordinate_3d = Coordinate3D::from(coordinate_1d);
            assert_eq!(coordinate_3d.color, bytes[index].2, "Coordinate3D for byte {} does not have the expected color.", index);
            assert_eq!(coordinate_3d.x, bytes[index].1, "Coordinate3D for byte {} does not have the expected x.", index);
            assert_eq!(coordinate_3d.y, bytes[index].0, "Coordinate3D for byte {} does not have the expected y.", index);
            assert_eq!(coordinate_1d, Coordinate1D::from(coordinate_3d), "Coordinate3D for byte {} did not convert back to the original Coordinate1D", index);
        }
    }

    #[test]
    fn test_scale_given_bigger_width_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 30000] };
        assert_eq!(Err(Error::InvalidScaleForImage(101, 50, 100, 100)), scale(&image, 101, 50));
        assert_eq!(Err(Error::InvalidScaleForImage(200, 100, 100, 100)), scale(&image, 200, 100));

        let image = Image { width: 50, height: 50, bytes: vec![0; 7500] };
        assert_eq!(Err(Error::InvalidScaleForImage(51, 25, 50, 50)), scale(&image, 51, 25));
        assert_eq!(Err(Error::InvalidScaleForImage(100, 50, 50, 50)), scale(&image, 100, 50));
    }

    #[test]
    fn test_scale_given_bigger_height_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 30000] };
        assert_eq!(Err(Error::InvalidScaleForImage(50, 101, 100, 100)), scale(&image, 50, 101));
        assert_eq!(Err(Error::InvalidScaleForImage(100, 200, 100, 100)), scale(&image, 100, 200));

        let image = Image { width: 50, height: 50, bytes: vec![0; 7500] };
        assert_eq!(Err(Error::InvalidScaleForImage(25, 51, 50, 50)), scale(&image, 25, 51));
        assert_eq!(Err(Error::InvalidScaleForImage(50, 100, 50, 50)), scale(&image, 50, 100));
    }

    #[test]
    fn test_scale_given_empty_width_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 30000] };
        assert_eq!(Err(Error::InvalidScaleForImage(0, 100, 100, 100)), scale(&image, 0, 100));
        assert_eq!(Err(Error::InvalidScaleForImage(0, 200, 100, 100)), scale(&image, 0, 200));

        let image = Image { width: 50, height: 50, bytes: vec![0; 7500] };
        assert_eq!(Err(Error::InvalidScaleForImage(0, 50, 50, 50)), scale(&image, 0, 50));
        assert_eq!(Err(Error::InvalidScaleForImage(0, 100, 50, 50)), scale(&image, 0, 100));
    }

    #[test]
    fn test_scale_given_empty_height_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 30000] };
        assert_eq!(Err(Error::InvalidScaleForImage(100, 0, 100, 100)), scale(&image, 100, 0));
        assert_eq!(Err(Error::InvalidScaleForImage(200, 0, 100, 100)), scale(&image, 200, 0));

        let image = Image { width: 50, height: 50, bytes: vec![0; 7500] };
        assert_eq!(Err(Error::InvalidScaleForImage(50, 0, 50, 50)), scale(&image, 50, 0));
        assert_eq!(Err(Error::InvalidScaleForImage(100, 0, 50, 50)), scale(&image, 100, 0));
    }

    #[test]
    fn test_scale_given_image_with_too_many_bytes_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 50000] };
        assert_eq!(Err(Error::InvalidImage(30000, 50000)), scale(&image, 50, 50));
    }

    #[test]
    fn test_scale_given_image_with_too_few_bytes_should_return_err() {
        let image = Image { width: 100, height: 100, bytes: vec![0; 5000] };
        assert_eq!(Err(Error::InvalidImage(30000, 5000)), scale(&image, 50, 50));
    }

    #[test]
    fn test_scale_given_indentical_size_should_return_same_image() {
        let mut image = Image { width: 100, height: 100, bytes: vec![0; 30000] };
        for byte in &mut image.bytes {
            *byte = random::<u8>();
        }

        let result = scale(&image, 100, 100);
        assert!(result.is_ok(), "scale did not suceed in scaling the image {:?}: {:?}", image, result);

        assert_eq!(image.width, result.as_ref().unwrap().width, "scale did not conserve the image’s width");
        assert_eq!(image.height, result.as_ref().unwrap().height, "scale did not conserve the image’s width");
        assert_eq!(image.bytes, result.as_ref().unwrap().bytes, "scale did not conserve the image’s bytes");
    }

    #[test]
    fn test_scale_given_simple_squared_image_should_return_smaller_image() {
        let image = Image { width: 4, height: 4, bytes: vec![
            255,0,0,  255,0,0,  0,255,0,  0,255,0,
            255,0,0,  255,0,0,  0,255,0,  0,255,0,
            0,0,255,  0,0,255,  99,0,99,  99,0,99,
            0,0,255,  0,0,255,  99,0,99,  99,0,99,
        ] };

        let result = scale(&image, 2, 2);
        assert_eq!(Ok(Image { width:  2, height: 2, bytes: vec![
            255,0,0,  0,255,0,
            0,0,255,  99,0,99,
        ] }), result);
    }

    #[test]
    fn test_scale_given_simple_rectangle_image_should_return_smaller_image() {
        let image = Image { width: 6, height: 4, bytes: vec![
            255,0,0,  255,0,0,  255,0,0,  0,255,0,  0,255,0,  0,255,0,
            255,0,0,  255,0,0,  255,0,0,  0,255,0,  0,255,0,  0,255,0,
            0,0,255,  0,0,255,  0,0,255,  99,0,99,  99,0,99,  99,0,99,
            0,0,255,  0,0,255,  0,0,255,  99,0,99,  99,0,99,  99,0,99,
        ] };

        let result = scale(&image, 2, 2);
        assert_eq!(Ok(Image { width:  2, height: 2, bytes: vec![
            255,0,0,  0,255,0,
            0,0,255,  99,0,99,
        ] }), result);
    }

    #[test]
    fn test_scale_given_complex_squared_image_should_return_image_with_averaged_pixels() {
        let image = Image { width: 4, height: 4, bytes: vec![
            100,0,0,  0,100,0,  20,0,20,  40,0,40,
            0,100,0,  100,0,0,  60,0,60,  80,0,80,
            10,10,0,  20,20,0,  0,10,20,  30,40,0,
            30,30,0,  40,40,0,  50,60,0,  0,70,80,
        ] };

        let result = scale(&image, 2, 2);
        assert_eq!(Ok(Image { width:  2, height: 2, bytes: vec![
            50,50,0,   50,0,50,
            25,25,0,  20,45,25,
        ] }), result);
    }
}
