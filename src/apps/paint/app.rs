use std::sync::Arc;

use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps::{App, Image, In, Out};
use crate::midi::features::Features;
use super::config::Config;

pub const NAME: &'static str = "paint";
pub const COLOR: [u8; 3] = [255, 255, 0];

pub const COLOR_PALETTE: [[u8; 3]; 8] = [
    [000, 000, 000],
    [000, 000, 255],
    [000, 255, 000],
    [000, 255, 255],
    [255, 000, 000],
    [255, 000, 255],
    [255, 255, 000],
    [255, 255, 255],
];

pub struct Paint {
    input_features: Arc<dyn Features + Sync + Send>,
    output_features: Arc<dyn Features + Sync + Send>,
    sender: Sender<Out>,
    receiver: Receiver<Out>,
    image: Image,
    color: [u8; 3],
}

impl Paint {
    pub fn new(
        _config: Config,
        input_features: Arc<dyn Features + Sync + Send>,
        output_features: Arc<dyn Features + Sync + Send>,
    ) -> Self {
        let (sender, receiver) = channel::<Out>(32);
        let (width, height) = input_features.get_grid_size().unwrap_or_else(|err| {
            eprintln!("[paint] falling back to a zero-pixel image, as the input device’s grid size cannot be retrieved: {}", err);
            (0, 0)
        });

        let image = Image { width, height, bytes: vec![0; width * height * 3] };

        return Paint {
            input_features,
            output_features,
            sender,
            receiver,
            image,
            color: COLOR_PALETTE[0],
        };
    }

    fn render_color_palette(&self) {
        match self.output_features.from_color_palette(Vec::from(COLOR_PALETTE)) {
            Ok(event) => self.sender.blocking_send(event.into()).unwrap_or_else(|err| {
                eprintln!("[paint] could not send event back to router: {}", err)
            }),
            Err(err) => eprintln!("[paint] could not transform the COLOR_PALETTE into a midi event: {}", err)
        }
    }

    fn render_pixel(&mut self, x: usize, y: usize) {
        if x < self.image.width && y < self.image.height {
            let byte_pos = y * 3 * 8 + x * 3;
            let pixel = &mut self.image.bytes[byte_pos..(byte_pos + 3)];

            // Set the pixel yellow!
            pixel[0] = self.color[0];
            pixel[1] = self.color[1];
            pixel[2] = self.color[2];

            match self.output_features.from_image(self.image.clone()) {
                Ok(event) => self.sender.blocking_send(event.into()).unwrap_or_else(|err| {
                    eprintln!("[paint] could not send event back to the router: {}", err)
                }),
                Err(err) => eprintln!("[paint] could not transform the image into a MIDI event: {}", err),
            }
        } else {
            eprintln!("[paint] ({}, {}) is out of bound", x, y);
        }
    }

    fn select_color(&mut self, index: usize) {
        if index < COLOR_PALETTE.len() {
            self.color = COLOR_PALETTE[index];
            println!("[paint] selected color: {:?}", self.color);
        } else {
            eprintln!("[paint] color {} is out of bound", index);
        }
    }
}

impl App for Paint {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return self.image.clone();
    }

    fn send(&mut self, event: In) -> Result<(), SendError<In>> {
        match event {
            In::Midi(event) => {
                match self.input_features.into_color_palette_index(event.clone()) {
                    Ok(Some(index)) => {
                        self.select_color(index);
                        return Ok(());
                    },
                    Ok(_) => {},
                    Err(e) => eprintln!("[paint] error when transforming incoming event into color index: {}", e),
                }

                match self.input_features.into_coordinates(event) {
                    Ok(Some((x, y))) => self.render_pixel(x, y),
                    Ok(_) => {}, // we ignore events that don’t map to a set of coordinates
                    Err(e) => eprintln!("[paint] error when transforming incoming event: {}", e),
                }
            },
            _ => {}, // we ignore events that are not MIDI events
        }
        Ok(())
    }

    fn receive(&mut self) -> Result<Out, TryRecvError> {
        return self.receiver.try_recv();
    }

    fn on_select(&mut self) {
        self.render_color_palette();
    }
}

#[cfg(test)]
mod test {
    use crate::image::Image;
    use crate::midi::Event;
    use crate::midi::features::{R, ColorPalette, GridController, ImageRenderer};
    use super::*;

    #[test]
    fn on_select_when_app_starts_then_render_color_palette() {
        let mut paint = get_paint();
        paint.on_select();

        // We expect to receive:
        // 1. the "palette" prefix, from the fake features implementation
        // 2. 3 bytes for each pixel of the color palette
        let event = paint.receive().unwrap();
        assert_eq!(event, Out::Midi(Event::SysEx(vec![
            b'p', b'a', b'l', b'e', b't', b't', b'e',
            000, 000, 000,
            000, 000, 255,
            000, 255, 000,
            000, 255, 255,
            255, 000, 000,
            255, 000, 255,
            255, 255, 000,
            255, 255, 255,
        ])));

        // We don’t expect any additional event
        let event = paint.receive();
        assert!(event.is_err());
    }

    #[test]
    fn get_logo_when_app_starts_then_return_a_black_image_of_the_size_of_the_grid() {
        let paint = get_paint();
        let image = paint.get_logo();
        assert_eq!(image, Image {
            width: 2,
            height: 2,
            bytes: vec![
                // 3 bytes per pixel, 2 pixels on top and 2 at the bottom = 12 bytes
                0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0,
            ],
        });
    }

    #[test]
    fn when_user_selects_color_and_presses_one_pixel_then_draw_the_pixel_on_the_image() {
        let mut paint = get_paint();

        // select cyan (as per our fake implementation of features)
        paint.send(In::Midi(Event::Midi([176, 3, 0, 0]))).unwrap();

        // press (1, 0) (as per our fake implementation of features
        paint.send(In::Midi(Event::Midi([144, 1, 0, 0]))).unwrap();

        // We expect to receive:
        // 1. the "image" prefix, written by our fake features
        // 2. black pixels, except for the top-right one (1, 0)
        let event = paint.receive().unwrap();
        assert_eq!(event, Out::Midi(Event::SysEx(vec![
            b'i', b'm', b'a', b'g', b'e',
            000, 000, 000, 000, 255, 255,
            000, 000, 000, 000, 000, 000,
        ])));

        // We don’t expect any additional event
        let event = paint.receive();
        assert!(event.is_err());
    }

    fn get_paint() -> Paint {
        return Paint::new(
            Config {},
            Arc::new(FakeFeatures {}),
            Arc::new(FakeFeatures {}),
        );
    }

    struct FakeFeatures {}
    impl GridController for FakeFeatures {
        fn get_grid_size(&self) -> R<(usize, usize)> {
            Ok((2, 2))
        }

        fn into_coordinates(&self, event: Event) -> R<Option<(usize, usize)>> {
            Ok(match event {
                Event::Midi([144, x, y, _]) => Some((x as usize, y as usize)),
                _ => None,
            })
        }
    }
    impl ColorPalette for FakeFeatures {
        fn into_color_palette_index(&self, event: Event) -> R<Option<usize>> {
            Ok(match event {
                Event::Midi([176, index, _, _]) => Some(index.into()),
                _ => None,
            })
        }

        fn from_color_palette(&self, color_palette: Vec<[u8; 3]>) -> R<Event> {
            let mut bytes = Vec::from("palette".as_bytes());
            for color in color_palette {
                bytes.append(&mut color.into());
            }
            return Ok(Event::SysEx(bytes));
        }
    }
    impl ImageRenderer for FakeFeatures {
        fn from_image(&self, mut image: Image) -> R<Event> {
            let mut bytes = Vec::from("image".as_bytes());
            bytes.append(&mut image.bytes);
            return Ok(Event::SysEx(bytes));
        }
    }
    impl Features for FakeFeatures {}
}
