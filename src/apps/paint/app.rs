use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps::{App, EventTransformer, Image, In, Out};
use super::config::Config;

pub const NAME: &'static str = "paint";
pub const COLOR: [u8; 3] = [255, 255, 0];

pub struct Paint {
    input_transformer: &'static (dyn EventTransformer + Sync),
    output_transformer: &'static (dyn EventTransformer + Sync),
    sender: Sender<Out>,
    receiver: Receiver<Out>,
}

impl Paint {
    pub fn new(
        _config: Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (sender, receiver) = channel::<Out>(32);
        Paint {
            input_transformer,
            output_transformer,
            sender,
            receiver,
        }
    }

    fn render_yellow_pixel(&self, x: u16, y: u16) {
        // Creating a 8x8 image, because the app will only work on a Launchpad Pro for now
        if x < 8 && y < 8 {
            let mut image = Image { width: 8, height: 8, bytes: vec![0; 64*3] };
            let byte_pos = (y * 3 * 8 + x * 3) as usize;
            let pixel = &mut image.bytes[byte_pos..(byte_pos + 3)];

            // Set the pixel yellow!
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 0;

            match self.output_transformer.from_image(image) {
                Ok(event) => self.sender.blocking_send(event.into()).unwrap_or_else(|err| {
                    eprintln!("[paint] could not send event back to the router: {}", err)
                }),
                Err(err) => eprintln!("[paint] could not transform the image into a MIDI event: {}", err),
            }
        } else {
            eprintln!("[paint] ({}, {}) is out of bound", x, y);
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
        return Image { width: 0, height: 0, bytes: vec![] };
    }

    fn send(&mut self, event: In) -> Result<(), SendError<In>> {
        match event {
            In::Midi(event) => {
                match self.input_transformer.into_coordinates(event) {
                    Ok(Some((x, y))) => self.render_yellow_pixel(x, y),
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
}
