use std::collections::BTreeMap;

use zbus::{Connection, fdo::ObjectManagerProxy, proxy};

use crate::{applet::GpuInfo, config::TrayMode};

pub const SERVICE: &str = "com.github.opengamingcollective.cardwire";
pub const ROOT_PATH: &str = "/com/github/opengamingcollective/cardwire";
const GPU_INTERFACE: &str = "com.github.opengamingcollective.cardwire.Gpu";

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Manager",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
trait CardwireManager {
    fn status(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Mode",
    default_service = "com.github.opengamingcollective.cardwire",
    default_path = "/com/github/opengamingcollective/cardwire"
)]
pub trait CardwireMode {
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn set_mode(&self, mode: u32) -> zbus::Result<()>;
}

#[proxy(
    interface = "com.github.opengamingcollective.cardwire.Gpu",
    default_service = "com.github.opengamingcollective.cardwire"
)]
pub trait CardwireGpu {
    #[zbus(property)]
    fn block(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_block(&self, block: bool) -> zbus::Result<()>;

    fn get_device(&self) -> zbus::Result<DbusGpuDevice>;

    fn power_state(&self) -> zbus::Result<String>;

    #[zbus(signal)]
    fn power_state_changed(&self, state: String) -> zbus::Result<()>;
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, zbus::zvariant::Type)]
pub struct DbusGpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}

#[derive(Debug, Clone)]
pub struct CardwireClient {
    connection: Connection,
}

impl CardwireClient {
    pub async fn connect() -> zbus::Result<Self> {
        let client = Self {
            connection: Connection::system().await?,
        };
        client.status().await?;
        Ok(client)
    }

    pub async fn status(&self) -> zbus::Result<()> {
        CardwireManagerProxy::new(&self.connection)
            .await?
            .status()
            .await
    }

    pub async fn mode_proxy(&self) -> zbus::Result<CardwireModeProxy<'static>> {
        CardwireModeProxy::new(&self.connection).await
    }

    pub async fn gpu_proxy(&self, id: u32) -> zbus::Result<CardwireGpuProxy<'static>> {
        CardwireGpuProxy::builder(&self.connection)
            .path(format!("{ROOT_PATH}/Gpu/{id}"))?
            .build()
            .await
    }

    pub async fn mode(&self) -> zbus::Result<TrayMode> {
        let value = self.mode_proxy().await?.mode().await?;
        TrayMode::from_value(value).ok_or_else(|| {
            zbus::Error::Failure(format!("daemon returned unknown mode value {value}"))
        })
    }

    pub async fn set_mode(&self, mode: TrayMode) -> zbus::Result<()> {
        self.mode_proxy().await?.set_mode(mode.value()).await
    }

    pub async fn set_gpu_block(&self, id: u32, blocked: bool) -> zbus::Result<()> {
        self.gpu_proxy(id).await?.set_block(blocked).await
    }

    pub async fn snapshot(&self) -> zbus::Result<(TrayMode, BTreeMap<u32, GpuInfo>)> {
        Ok((self.mode().await?, self.gpus().await?))
    }

    pub async fn gpus(&self) -> zbus::Result<BTreeMap<u32, GpuInfo>> {
        let object_manager = ObjectManagerProxy::builder(&self.connection)
            .destination(SERVICE)?
            .path(ROOT_PATH)?
            .build()
            .await?;
        let objects = object_manager.get_managed_objects().await?;
        let prefix = format!("{ROOT_PATH}/Gpu/");
        let mut gpus = BTreeMap::new();

        for (path, interfaces) in objects {
            if !interfaces
                .keys()
                .any(|interface| interface.as_str() == GPU_INTERFACE)
            {
                continue;
            }
            let Some(id) = path
                .as_str()
                .strip_prefix(&prefix)
                .and_then(|value| value.parse::<u32>().ok())
            else {
                continue;
            };
            let proxy = self.gpu_proxy(id).await?;
            let device = proxy.get_device().await?;
            gpus.insert(
                id,
                GpuInfo {
                    id,
                    name: device.name,
                    default: device.default,
                    blocked: proxy.block().await?,
                    power_state: proxy
                        .power_state()
                        .await
                        .unwrap_or_else(|_| "Unknown".to_string()),
                },
            );
        }
        Ok(gpus)
    }
}
