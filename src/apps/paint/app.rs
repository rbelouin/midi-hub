use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps::{App, EventTransformer, Image, In, Out};
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
    input_transformer: &'static (dyn EventTransformer + Sync),
    output_transformer: &'static (dyn EventTransformer + Sync),
    sender: Sender<Out>,
    receiver: Receiver<Out>,
    image: Image,
    color: [u8; 3],
}

impl Paint {
    pub fn new(
        _config: Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (sender, receiver) = channel::<Out>(32);
        let (width, height) = input_transformer.get_grid_size().unwrap_or_else(|err| {
            eprintln!("[paint] falling back to a zero-pixel image, as the input device’s grid size cannot be retrieved: {}", err);
            (0, 0)
        });

        let image = Image { width, height, bytes: vec![0; 64*3] };

        let paint = Paint {
            input_transformer,
            output_transformer,
            sender,
            receiver,
            image,
            color: COLOR_PALETTE[0],
        };

        paint.render_color_palette();
        return paint;
    }

    fn render_color_palette(&self) {
        match self.output_transformer.from_color_palette(Vec::from(COLOR_PALETTE)) {
            Ok(event) => self.sender.blocking_send(event.into()).unwrap_or_else(|err| {
                eprintln!("[paint] could not send event back to router: {}", err)
            }),
            Err(err) => eprintln!("[paint] could not transformer the COLOR_PALETTE into a midi event: {}", err)
        }
    }

    fn render_pixel(&mut self, x: u16, y: u16) {
        let x = x as usize;
        let y = y as usize;

        if x < self.image.width && y < self.image.height {
            let byte_pos = y * 3 * 8 + x * 3;
            let pixel = &mut self.image.bytes[byte_pos..(byte_pos + 3)];

            // Set the pixel yellow!
            pixel[0] = self.color[0];
            pixel[1] = self.color[1];
            pixel[2] = self.color[2];

            match self.output_transformer.from_image(self.image.clone()) {
                Ok(event) => self.sender.blocking_send(event.into()).unwrap_or_else(|err| {
                    eprintln!("[paint] could not send event back to the router: {}", err)
                }),
                Err(err) => eprintln!("[paint] could not transform the image into a MIDI event: {}", err),
            }
        } else {
            eprintln!("[paint] ({}, {}) is out of bound", x, y);
        }
    }

    fn select_color(&mut self, index: u16) {
        let index = index as usize;
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
                match self.input_transformer.into_color_palette_index(event.clone()) {
                    Ok(Some(index)) => {
                        self.select_color(index);
                        return Ok(());
                    },
                    Ok(_) => {},
                    Err(e) => eprintln!("[paint] error when transforming incoming event into color index: {}", e),
                }

                match self.input_transformer.into_coordinates(event) {
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
