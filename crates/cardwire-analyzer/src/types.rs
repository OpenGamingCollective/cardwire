use zbus::zvariant;

#[derive(Clone, Debug)]
pub struct AppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
}

#[derive(Debug, Clone, zvariant::Type, serde::Serialize, serde::Deserialize)]
pub struct DbusAppMetadata {
    pub display_name: String,
    pub desktop_file_id: Option<String>,
    pub icon_name: Option<String>,
    pub gpu_policy: u32,
}

impl DbusAppMetadata {
    pub fn from_app_metadata(meta: &AppMetadata, gpu_policy: u32) -> Self {
        Self {
            display_name: meta.display_name.clone(),
            desktop_file_id: meta.desktop_file_id.clone(),
            icon_name: meta.icon_name.clone(),
            gpu_policy,
        }
    }
}
