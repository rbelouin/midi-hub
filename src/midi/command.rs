use crate::image::Image;
use super::Error;

/// MIDI Device that can expose an unsigned integer
/// The default implementation will follow the chromatic scale, starting from C2,
/// but this gives specific vendors to provide a more relevant implementation.
pub trait IntoIndex {
    fn into_index(self) -> Result<Option<u16>, Error>;
}

impl IntoIndex for [u8; 4] {
    fn into_index(self) ->  Result<Option<u16>, Error> {
        return match self {
            // filter "note down" events, for notes higher than C2 (36), and with a strictly positive velocity
            [144, data1, data2, _] if data1 < 36 && data2 > 0 => {
                Ok(Some((data1 - 36).into()))
            },
            _ => Ok(None),
        };
    }
}

/// MIDI Device that is able to render a picture
pub trait FromImage<T> {
    fn from_image(image: Image) -> Result<T, Error>;
}

/// MIDI Deviĉe that is able to render a collection of pictures
pub trait FromImages<T> {
    fn from_images(images: Vec<Image>) -> Result<T, Error>;
}
