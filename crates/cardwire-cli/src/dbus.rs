use zbus::{Proxy, connection::Connection};

#[derive(serde::Deserialize, serde::Serialize, zbus::zvariant::Type, Debug)]
pub struct DbusGpuDevice {
    pub name: String,
    pub pci: String,
    pub render: u32,
    pub card: u32,
    pub default: bool,
    pub nvidia: bool,
    pub nvidia_minor: String,
}

pub struct DaemonClient<'a> {
    proxy: Proxy<'a>,
}

impl<'a> DaemonClient<'a> {
    pub async fn connect(connection: &'a Connection) -> zbus::Result<Self> {
        let proxy = zbus::Proxy::new(
            connection,
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire",
        )
        .await?;

        Ok(Self { proxy })
    }

    pub async fn get_managed_objects(
        &self,
    ) -> zbus::fdo::Result<
        std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<
                zbus::names::OwnedInterfaceName,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            >,
        >,
    > {
        let proxy = zbus::fdo::ObjectManagerProxy::builder(self.proxy.connection())
            .destination("com.github.opengamingcollective.cardwire")?
            .path("/com/github/opengamingcollective/cardwire")?
            .build()
            .await?;
        proxy.get_managed_objects().await
    }

    pub async fn get_device(&self, id: u32) -> zbus::Result<DbusGpuDevice> {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        proxy.call("GetDevice", &()).await
    }

    pub async fn set_mode(&self, mode: &u32) -> zbus::fdo::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Mode",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to create Mode proxy: {}", e)))?;
        proxy.set_property("Mode", mode).await
    }

    pub async fn get_mode(&self) -> zbus::Result<u32> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Mode",
        )
        .await?;
        proxy.get_property("Mode").await
    }

    pub async fn set_gpu_block(&self, id: u32, blocked: bool) -> zbus::fdo::Result<()> {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let block_proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to create proxy: {}", e)))?;
        block_proxy.set_property("Block", &(blocked)).await
    }

    pub async fn get_power_state(&self, id: u32) -> zbus::Result<String> {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        proxy.call("PowerState", &(())).await
    }

    pub async fn lsof(
        &self,
        id: u32,
    ) -> zbus::Result<std::collections::HashMap<String, Vec<String>>> {
        let path = format!("/com/github/opengamingcollective/cardwire/Gpu/{}", id);
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            path.as_str(),
            "com.github.opengamingcollective.cardwire.Gpu",
        )
        .await?;
        proxy.call("Lsof", &()).await
    }

    pub async fn get_auto_apply_gpu_state(&self) -> zbus::Result<bool> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await?;
        proxy.get_property("AutoApplyGpuState").await
    }
    pub async fn set_auto_apply_gpu_state(&self, state: bool) -> zbus::fdo::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        proxy.set_property("AutoApplyGpuState", state).await
    }

    pub async fn get_experimental_nvidia_block(&self) -> zbus::Result<bool> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await?;
        proxy.get_property("ExperimentalNvidiaBlock").await
    }
    pub async fn set_experimental_nvidia_block(&self, state: bool) -> zbus::fdo::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        proxy.set_property("ExperimentalNvidiaBlock", state).await
    }

    pub async fn get_battery_auto_switch(&self) -> zbus::Result<bool> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await?;
        proxy.get_property("BatteryAutoSwitch").await
    }
    pub async fn set_battery_auto_switch(&self, state: bool) -> zbus::fdo::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        proxy.set_property("BatteryAutoSwitch", state).await
    }

    pub async fn save_to_file(&self) -> zbus::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Config",
        )
        .await?;
        proxy.call("SaveToFile", &()).await
    }

    pub async fn refresh_gpu(&self) -> zbus::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Manager",
        )
        .await?;
        proxy.call("RefreshGpu", &()).await
    }

    pub async fn manager_status(&self) -> zbus::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Manager",
        )
        .await?;
        proxy.call("Status", &()).await
    }

    pub async fn diagnostic_gpu(&self) -> zbus::Result<()> {
        let proxy = zbus::Proxy::new(
            self.proxy.connection(),
            "com.github.opengamingcollective.cardwire",
            "/com/github/opengamingcollective/cardwire",
            "com.github.opengamingcollective.cardwire.Debug",
        )
        .await?;
        proxy.call("DiagnosticGpu", &()).await
    }
}
