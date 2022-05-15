use crate::midi::EventTransformer;

pub fn transformer() -> &'static DefaultEventTransformer {
    return &DEFAULT_EVENT_TRANSFORMER;
}

const DEFAULT_EVENT_TRANSFORMER: DefaultEventTransformer = DefaultEventTransformer {};
pub struct DefaultEventTransformer {}
impl EventTransformer for DefaultEventTransformer {}
