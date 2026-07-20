use serde;
use zbus::{self, Connection};

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug, Clone)]
pub struct GpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}

#[derive(Debug, Clone)]
pub struct CardwireDbus {
    connection: zbus::Connection,
}
impl CardwireDbus {
    pub async fn new() -> Self {
        let connection = Connection::system().await.unwrap();
        CardwireDbus { connection }
    }
    pub async fn get_device(&self, id: u32) -> zbus::Result<GpuDevice> {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            &self.connection,
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        proxy.call("GetDevice", &()).await
    }
}
