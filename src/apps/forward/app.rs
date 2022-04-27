use tokio::sync::mpsc;

use crate::apps::{App, Out};
use crate::image::Image;
use crate::midi::{Event, EventTransformer};

use super::config::Config;

pub struct Forward {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

pub const NAME: &'static str = "forward";
pub const COLOR: [u8; 3] = [0, 0, 255];

impl Forward {
    pub fn new(
        _config: Config,
        _input_transformer: &'static (dyn EventTransformer + Sync),
        _output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<Event>(32);

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

    fn send(&mut self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        return self.sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out, mpsc::error::TryRecvError> {
        return self.receiver.try_recv().map(|event| event.into());
    }
}

pub fn get_logo() -> Image {
    return Image {
        width: 0,
        height: 0,
        bytes: vec![],
    };
}
