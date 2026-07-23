use std::collections::{BTreeMap, HashMap};

use zbus::{
    self, Connection, fdo, names::OwnedInterfaceName, zvariant::{OwnedObjectPath, OwnedValue}
};

use crate::models::{DaemonSettings, Mode};

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug, Clone)]
pub struct GpuDevice {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub blocked: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug, Clone)]
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
pub struct CardwireDbus {}
impl CardwireDbus {
    pub fn new() -> Self {
        CardwireDbus {}
    }
    pub async fn get_device(&self, id: u32) -> zbus::Result<DbusGpuDevice> {
        let connection = Connection::system().await?;
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            &connection,
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        proxy.call("GetDevice", &()).await
    }

    // Return a list of gpu interfaces
    async fn get_managed_objects(
        &self,
    ) -> zbus::fdo::Result<
        HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>,
    > {
        let connection = Connection::system().await?;
        let proxy = zbus::fdo::ObjectManagerProxy::builder(&connection)
            .destination("com.github.opengamingcollective.cardwire")?
            .path("/com/github/opengamingcollective/cardwire")?
            .build()
            .await?;
        proxy.get_managed_objects().await
    }
    pub async fn get_devices_list(&self) -> zbus::Result<BTreeMap<usize, GpuDevice>> {
        let objects = self.get_managed_objects().await?;
        let mut map = std::collections::BTreeMap::new();
        for (path, interfaces) in objects {
            let path_str = path.as_str();
            if let Some(id_str) =
                path_str.strip_prefix("/com/github/opengamingcollective/cardwire/Gpu/")
                && let Ok(id) = id_str.parse::<u32>()
            {
                let mut blocked = false;
                for (iface, props) in interfaces {
                    if iface.as_str() == "com.github.opengamingcollective.cardwire.Gpu"
                        && let Some(block_val) = props.get("Block")
                    {
                        blocked = block_val.downcast_ref::<bool>().unwrap_or(false);
                    }
                }
                if let Ok(dbus_dev) = self.get_device(id).await {
                    let dev = GpuDevice {
                        id,
                        name: dbus_dev.name,
                        pci: dbus_dev.pci,
                        render: dbus_dev.render,
                        card: dbus_dev.card,
                        default: dbus_dev.default,
                        blocked,
                        nvidia: dbus_dev.nvidia,
                        nvidia_minor: dbus_dev.nvidia_minor,
                    };
                    map.insert(id as usize, dev);
                }
            }
        }
        Ok(map)
    }
    pub async fn get_mode(&self) -> zbus::Result<u32> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Mode",
        )
        .await?;
        proxy.get_property("Mode").await
    }
    pub async fn set_mode(&self, mode: u32) -> zbus::fdo::Result<()> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Mode",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to create Mode proxy: {}", e)))?;
        proxy.set_property("Mode", mode).await
    }
    pub async fn set_setting(
        &self,
        setting: DaemonSettings,
        state: bool,
        mode_opt: Option<Mode>,
    ) -> zbus::fdo::Result<()> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        match setting {
            DaemonSettings::AutoApplyGpuState
            | DaemonSettings::ExpNvidiaBlock
            | DaemonSettings::BattAutoSwitch => {
                proxy.set_property(&setting.to_string(), state).await
            }
            DaemonSettings::BattAutoSwitchMode => {
                if let Some(mode_to_apply) = mode_opt {
                    println!("setting mode: {:?}", mode_to_apply as u32);
                    proxy
                        .set_property(&setting.to_string(), mode_to_apply as u32)
                        .await
                } else {
                    Err(fdo::Error::InvalidArgs("missing mode".to_string()))
                }
            }
        }
    }
}
