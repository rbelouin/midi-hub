use crate::midi::features::Features;

pub struct DefaultFeatures {}
impl Features for DefaultFeatures {}
impl DefaultFeatures {
    pub fn new() -> DefaultFeatures {
        DefaultFeatures {}
    }
}
