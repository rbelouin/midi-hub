use std::convert::From;
use crate::midi::Event;

mod image;
mod index;

#[derive(Clone, Debug)]
pub struct LaunchpadProEvent {
    event: Event,
}

impl From<Event> for LaunchpadProEvent {
    fn from(event: Event) -> LaunchpadProEvent {
        return LaunchpadProEvent { event };
    }
}

impl From<LaunchpadProEvent> for Event {
    fn from(event: LaunchpadProEvent) -> Event {
        return event.event;
    }
}
