use tokio::sync::mpsc;

use crate::apps::{App, In, Out};
use crate::image::Image;
use crate::midi::EventTransformer;

use super::config::Config;

pub struct Forward {
    sender: mpsc::Sender<In>,
    receiver: mpsc::Receiver<In>,
}

pub const NAME: &'static str = "forward";
pub const COLOR: [u8; 3] = [0, 0, 255];

impl Forward {
    pub fn new(
        _config: Config,
        _input_transformer: &'static (dyn EventTransformer + Sync),
        _output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<In>(32);

        Forward {
            sender,
            receiver,
        }
    }
}

impl App for Forward {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&mut self, event: In) -> Result<(), mpsc::error::SendError<In>> {
        match event {
            In::Midi(event) => self.sender.blocking_send(In::Midi(event)),
            _ => Ok(()),
        }
    }

    fn receive(&mut self) -> Result<Out, mpsc::error::TryRecvError> {
        return self.receiver.try_recv().and_then(|event| match event {
            In::Midi(event) => Ok(Out::Midi(event)),
            _ => Err(mpsc::error::TryRecvError::Empty),
        });
    }
}

pub fn get_logo() -> Image {
    return Image {
        width: 0,
        height: 0,
        bytes: vec![],
    };
}
