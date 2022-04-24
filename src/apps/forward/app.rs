use tokio::sync::mpsc;

use crate::apps::{App, Out};
use crate::image::Image;

use super::config::Config;

pub struct Forward<E> {
    sender: mpsc::Sender<E>,
    receiver: mpsc::Receiver<E>,
}

pub const NAME: &'static str = "forward";
pub const COLOR: [u8; 3] = [0, 0, 255];

impl<E: 'static> Forward<E> where
    E: Clone,
    E: std::fmt::Debug,
    E: std::marker::Send,
{
    pub fn new(_config: Config) -> Self {
        let (sender, receiver) = mpsc::channel::<E>(32);

        Forward {
            sender,
            receiver,
        }
    }
}

impl<E: 'static> App<E, Out<E>> for Forward<E> where
    E: Clone,
    E: std::fmt::Debug,
    E: std::marker::Send,
{

    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&self, event: E) -> Result<(), mpsc::error::SendError<E>> {
        return self.sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out<E>, mpsc::error::TryRecvError> {
        return self.receiver.try_recv().map(|event| Out::Event(event));
    }
}

pub fn get_logo() -> Image {
    return Image {
        width: 0,
        height: 0,
        bytes: vec![],
    };
}
