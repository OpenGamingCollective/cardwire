#[derive(Clone, Debug)]
pub struct AppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
}
