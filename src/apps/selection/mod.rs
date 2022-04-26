use crate::apps::App;

pub struct Selection {
    pub apps: Vec<Box<dyn App>>,
    pub selected_app: usize,
}
