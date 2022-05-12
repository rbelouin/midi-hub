use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps::{App, EventTransformer, Image, In, Out};
use super::config::Config;

pub const NAME: &'static str = "paint";
pub const COLOR: [u8; 3] = [255, 255, 0];

pub struct Paint {
    input_transformer: &'static (dyn EventTransformer + Sync),
}

impl Paint {
    pub fn new(
        _config: Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        _output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        Paint {
            input_transformer,
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
                    Ok(Some((x, y))) => println!("[paint] received coordinates: ({}, {})", x, y),
                    Ok(_) => {}, // we ignore events that don’t map to a set of coordinates
                    Err(e) => eprintln!("[paint] error when transforming incoming event: {}", e),
                }
            },
            _ => {}, // we ignore events that are not MIDI events
        }
        Ok(())
    }

    fn receive(&mut self) -> Result<Out, TryRecvError> {
        // Our application will remain silent for now
        Err(TryRecvError::Empty)
    }
}
