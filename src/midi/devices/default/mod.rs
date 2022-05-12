use crate::midi::{Error, Event, EventTransformer, Image};

pub fn transformer() -> &'static DefaultEventTransformer {
    return &DEFAULT_EVENT_TRANSFORMER;
}

const DEFAULT_EVENT_TRANSFORMER: DefaultEventTransformer = DefaultEventTransformer {};
pub struct DefaultEventTransformer {}
impl EventTransformer for DefaultEventTransformer {
    fn get_grid_size(&self) -> Result<(usize, usize), Error> {
        return Err(Error::Unsupported);
    }

    fn into_index(&self, event: Event) -> Result<Option<u16>, Error> {
         return match event {
            // filter "note down" events, for notes higher than C2 (36), and with strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data1 >= 36 && data2 > 0 => {
                Ok(Some((data1 - 36).into()))
            },
            _ => Ok(None),
        };
    }

    fn into_app_index(&self, event: Event) -> Result<Option<u16>, Error> {
         return match event {
            // filter "note down" events, for notes lower than C0 (12), and with strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data1 < 12 && data2 > 0 => {
                Ok(Some(data1.into()))
            },
            _ => Ok(None),
        };
    }

    fn into_coordinates(&self, _event: Event) -> Result<Option<(u16, u16)>, Error> {
        return Err(Error::Unsupported);
    }

    fn from_image(&self, _image: Image) -> Result<Event, Error> {
        return Err(Error::Unsupported);
    }

    fn from_index_to_highlight(&self, _index: u16) -> Result<Event, Error> {
        return Err(Error::Unsupported);
    }

    fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
        return Err(Error::Unsupported);
    }
}
