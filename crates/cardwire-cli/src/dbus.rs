use std::collections::{BTreeMap, HashMap};

use cardwire_core::modes::types::Modes;
use cardwire_dbus::{
    config::CardwireConfigProxy, debug::CardwireDebugProxy, gpus::{CardwireGpuIntProxy, CardwireGpuProxy}, manager::CardwireManagerProxy, mode::CardwireModeProxy, smart::CardwireSmartPolicyProxy, types::{DbusAppMetadata, DbusGpuDevice, DbusPciDevice}
};
use zbus::{
    Connection, Proxy, names::OwnedInterfaceName, proxy::Defaults, zvariant::{OwnedObjectPath, OwnedValue}
};

use zbus::Result;

pub struct DaemonClient<'a> {
    proxy: Proxy<'a>,
}

impl<'a> DaemonClient<'a> {
    pub async fn connect(connection: &'a Connection) -> zbus::Result<Self> {
        let proxy = zbus::Proxy::new(
            connection,
            "org.opengamingcollective.cardwire",
            "/org/opengamingcollective/cardwire",
            "org.opengamingcollective.cardwire",
        )
        .await?;

        Ok(Self { proxy })
    }

    // DEBUG INTERFACE

    pub async fn get_pci_devices(&self) -> zbus::Result<BTreeMap<String, DbusPciDevice>> {
        let proxy = CardwireDebugProxy::new(self.proxy.connection()).await?;
        proxy.get_pci_devices().await
    }

    pub async fn refresh_gpus(&self) -> Result<()> {
        let proxy = CardwireDebugProxy::new(self.proxy.connection()).await?;
        proxy.refresh_gpu().await
    }

    // MODE INTERFACE

    pub async fn get_mode(&self) -> Result<Modes> {
        let proxy = CardwireModeProxy::new(self.proxy.connection()).await?;
        proxy.mode().await
    }

    pub async fn set_mode(&self, mode: Modes) -> Result<()> {
        let proxy = CardwireModeProxy::new(self.proxy.connection()).await?;
        proxy.set_mode(mode).await
    }

    pub async fn get_available_modes(&self) -> Result<Vec<Modes>> {
        let proxy = CardwireModeProxy::new(self.proxy.connection()).await?;
        proxy.available_modes().await
    }

    // CONFIG INTERFACE

    pub async fn experimental_nvidia_block(&self) -> Result<bool> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.experimental_nvidia_block().await
    }
    pub async fn set_experimental_nvidia_block(&self, state: bool) -> Result<()> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.set_experimental_nvidia_block(state).await
    }

    pub async fn auto_apply_gpu_state(&self) -> Result<bool> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.auto_apply_gpu_state().await
    }
    pub async fn set_auto_apply_gpu_state(&self, state: bool) -> Result<()> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.set_auto_apply_gpu_state(state).await
    }

    pub async fn battery_auto_switch(&self) -> Result<bool> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.battery_auto_switch().await
    }
    pub async fn set_battery_auto_switch(&self, state: bool) -> Result<()> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.set_battery_auto_switch(state).await
    }

    pub async fn battery_auto_switch_mode(&self) -> Result<Modes> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.battery_auto_switch_mode().await
    }
    pub async fn set_battery_auto_switch_mode(&self, state: Modes) -> Result<()> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.set_battery_auto_switch_mode(state).await
    }

    pub async fn external_display_auto_switch(&self) -> Result<bool> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.external_display_auto_switch().await
    }
    pub async fn set_external_display_auto_switch(&self, state: bool) -> Result<()> {
        let proxy = CardwireConfigProxy::new(self.proxy.connection()).await?;
        proxy.set_external_display_auto_switch(state).await
    }

    // Smart Policy

    pub async fn get_app_policies(&self) -> Result<HashMap<String, DbusAppMetadata>> {
        let proxy = CardwireSmartPolicyProxy::new(self.proxy.connection()).await?;
        proxy.get_app_policies().await
    }

    // GPU INTERFACE

    pub async fn get_gpu_objects(
        &self,
    ) -> Result<HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>>
    {
        let proxy = CardwireGpuIntProxy::new(self.proxy.connection()).await?;
        proxy.get_managed_objects().await
    }

    pub async fn get_gpu_device(&self, id: u32) -> zbus::Result<DbusGpuDevice> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.get_device().await
    }

    pub async fn block(&self, id: u32) -> zbus::Result<bool> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.block().await
    }

    pub async fn set_block(&self, id: u32, state: bool) -> zbus::Result<()> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.set_block(state).await
    }

    pub async fn lsof(&self, id: u32) -> zbus::Result<HashMap<String, Vec<String>>> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.lsof().await
    }

    pub async fn power_state(&self, id: u32) -> zbus::Result<String> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.power_state().await
    }

    pub async fn env(&self, id: u32) -> zbus::Result<Vec<String>> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.env().await
    }

    pub async fn launchable(&self, id: u32) -> zbus::Result<bool> {
        let proxy = CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await?;
        proxy.launchable().await
    }

    // MANAGER INTERFACE

    pub async fn get_daemon_status(&self) -> zbus::Result<()> {
        let proxy = CardwireManagerProxy::new(self.proxy.connection()).await?;
        proxy.status().await
    }
}
