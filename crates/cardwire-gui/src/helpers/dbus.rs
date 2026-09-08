use std::{
    collections::{BTreeMap, HashMap}, hash::{Hash, Hasher}, sync::Arc, time::Duration
};

use tokio::sync::Notify;
use zbus::{
    self, Connection, connection::Builder, fdo::{self, RequestNameFlags}, names::OwnedInterfaceName, zvariant::{OwnedObjectPath, OwnedValue}
};

use crate::models::{DaemonSettings, DbusAppMetadata, LsofData, Mode};

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct GpuDevice {
    pub id: u32,
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub discrete: bool,
    pub virtual_gpu: bool,
    pub available: bool,
    pub vendor: String,
    pub driver: String,
    pub blocked: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
    pub power_state: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug, Clone)]
pub struct DbusGpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub discrete: bool,
    pub virtual_gpu: bool,
    pub available: bool,
    pub vendor: String,
    pub driver: String,
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
        let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            path.as_str(),
            "org.opengamingcollective.cardwire.Gpu",
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
            .destination("org.opengamingcollective.cardwire")?
            .path("/org/opengamingcollective/cardwire")?
            .build()
            .await?;
        proxy.get_managed_objects().await
    }
    pub async fn get_devices_list(&self) -> zbus::Result<BTreeMap<usize, GpuDevice>> {
        let objects = self.get_managed_objects().await?;
        let mut map = std::collections::BTreeMap::new();
        for (path, interfaces) in objects {
            let path_str = path.as_str();
            if let Some(id_str) = path_str.strip_prefix("/org/opengamingcollective/cardwire/Gpu/")
                && let Ok(id) = id_str.parse::<u32>()
            {
                let mut blocked = false;
                for (iface, props) in interfaces {
                    if iface.as_str() == "org.opengamingcollective.cardwire.Gpu"
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
                        discrete: dbus_dev.discrete,
                        virtual_gpu: dbus_dev.virtual_gpu,
                        available: dbus_dev.available,
                        vendor: dbus_dev.vendor,
                        driver: dbus_dev.driver,
                        blocked,
                        nvidia: dbus_dev.nvidia,
                        nvidia_minor: dbus_dev.nvidia_minor,
                        power_state: None,
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
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.Mode",
        )
        .await?;
        proxy.get_property("Mode").await
    }
    pub async fn get_available_modes(&self) -> zbus::Result<Vec<Mode>> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.Mode",
        )
        .await?;
        proxy.call("AvailableModes", &()).await
    }
    pub async fn set_mode(&self, mode: u32) -> zbus::fdo::Result<()> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.Mode",
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
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.Config",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        match setting {
            DaemonSettings::AutoApplyGpuState
            | DaemonSettings::ExpNvidiaBlock
            | DaemonSettings::BattAutoSwitch
            | DaemonSettings::ExternalDisplayAutoSwitch => {
                proxy.set_property(&setting.to_string(), state).await
            }
            DaemonSettings::BattAutoSwitchMode => {
                if let Some(mode_to_apply) = mode_opt {
                    proxy
                        .set_property(&setting.to_string(), mode_to_apply as u32)
                        .await
                } else {
                    Err(fdo::Error::InvalidArgs("missing mode".to_string()))
                }
            }
        }
    }
    pub async fn set_gpu_block(&self, id: u32, blocked: bool) -> zbus::fdo::Result<()> {
        let connection = Connection::system().await?;
        let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            path.as_str(),
            "org.opengamingcollective.cardwire.Gpu",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to create proxy: {}", e)))?;
        proxy.set_property("Block", &(blocked)).await
    }
    pub async fn lsof(&self, id: u32) -> zbus::Result<LsofData> {
        let connection = Connection::system().await?;
        let path = format!("/org/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            path.as_str(),
            "org.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        let result: HashMap<String, Vec<String>> = proxy.call("Lsof", &()).await?;
        Ok(LsofData {
            gpu_id: id as usize,
            processes: result,
        })
    }
    pub async fn refresh_gpu(&self) -> zbus::Result<()> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.Debug",
        )
        .await?;
        proxy.call("RefreshGpu", &()).await
    }
    pub async fn get_app_policies(&self) -> zbus::Result<HashMap<String, DbusAppMetadata>> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.SmartPolicy",
        )
        .await?;
        proxy.call("GetAppPolicies", &()).await
    }
    pub async fn set_app_policy(&self, app_id: String, policy: i32) -> zbus::Result<()> {
        let connection = Connection::system().await?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire.SmartPolicy",
        )
        .await?;
        proxy.call("SetAppPolicy", &(app_id, policy)).await
    }
}

const BUS_NAME: &str = "org.opengamingcollective.cardwire.Gui";
const OBJECT_PATH: &str = "/org/opengamingcollective/cardwire/Gui";

struct ActivationInterface(Arc<Notify>);

#[zbus::interface(name = "org.opengamingcollective.cardwire.Gui")]
impl ActivationInterface {
    fn activate(&self) {
        // Keep a permit if the GUI has not started listening yet. Repeated
        // requests can be coalesced because they all open the same window.
        self.0.notify_one();
    }
}

/// Owns the session bus name for as long as the GUI is running.
#[derive(Debug, Clone)]
pub struct AppInstance {
    connection: Connection,
    activation: Arc<Notify>,
}

impl Hash for AppInstance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.connection.unique_name().hash(state);
    }
}

impl AppInstance {
    /// Returns the shared notification source for activation requests.
    pub fn activation(&self) -> Arc<Notify> {
        Arc::clone(&self.activation)
    }

    /// Returns `None` when another instance owns the name. Explicit background
    /// launches leave that instance hidden; normal launches ask it to open.
    pub async fn acquire(activate_existing: bool) -> zbus::Result<Option<Self>> {
        let activation = Arc::new(Notify::new());
        let connection = Builder::session()?
            .method_timeout(Duration::from_secs(5))
            // Export before claiming the name so simultaneous launches can
            // immediately call Activate on the winner.
            .serve_at(OBJECT_PATH, ActivationInterface(Arc::clone(&activation)))?
            .build()
            .await?;

        match connection
            .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(_) => Ok(Some(Self {
                connection,
                activation,
            })),
            Err(zbus::Error::NameTaken) => {
                if activate_existing {
                    connection
                        .call_method(Some(BUS_NAME), OBJECT_PATH, Some(BUS_NAME), "Activate", &())
                        .await?;
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "run under dbus-run-session to isolate the GUI bus name"]
    async fn exclusive_ownership_activation_and_release() {
        let primary = AppInstance::acquire(true).await.unwrap().unwrap();

        assert!(AppInstance::acquire(false).await.unwrap().is_none());
        tokio::select! {
            biased;
            _ = primary.activation.notified() => panic!("background launch requested activation"),
            _ = std::future::ready(()) => {}
        }

        // Activation must survive arrival before the GUI subscribes.
        assert!(AppInstance::acquire(true).await.unwrap().is_none());
        tokio::time::timeout(Duration::from_secs(1), primary.activation.notified())
            .await
            .unwrap();

        primary.connection.close().await.unwrap();

        // Concurrent startups elect exactly one owner, without stale locks.
        let (first, second) = tokio::join!(AppInstance::acquire(true), AppInstance::acquire(true));
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first.is_some(), second.is_some());
        let winner = first.or(second).unwrap();
        tokio::time::timeout(Duration::from_secs(1), winner.activation.notified())
            .await
            .unwrap();
        winner.connection.close().await.unwrap();
    }
}
