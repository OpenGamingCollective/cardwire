use std::collections::{BTreeMap, HashMap};

use cardwire_core::modes::types::Modes;
use cardwire_dbus::{
    config::CardwireConfigProxy, debug::CardwireDebugProxy, gpus::{CardwireGpuIntProxy, CardwireGpuProxy}, manager::CardwireManagerProxy, mode::CardwireModeProxy, smart::CardwireSmartPolicyProxy, types::{DbusAppMetadata, DbusGpuDevice, DbusPciDevice}
};
use zbus::{
    Connection, Proxy, names::OwnedInterfaceName, zvariant::{OwnedObjectPath, OwnedValue}
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

    async fn dbus_proxy(&self) -> Result<CardwireDebugProxy<'_>> {
        CardwireDebugProxy::new(self.proxy.connection()).await
    }

    pub async fn get_pci_devices(&self) -> zbus::Result<BTreeMap<String, DbusPciDevice>> {
        self.dbus_proxy().await?.get_pci_devices().await
    }

    pub async fn refresh_gpus(&self) -> Result<()> {
        self.dbus_proxy().await?.refresh_gpu().await
    }

    // MODE INTERFACE

    async fn mode_proxy(&self) -> Result<CardwireModeProxy<'_>> {
        CardwireModeProxy::new(self.proxy.connection()).await
    }

    pub async fn get_mode(&self) -> Result<Modes> {
        self.mode_proxy().await?.mode().await
    }

    pub async fn set_mode(&self, mode: Modes) -> Result<()> {
        self.mode_proxy().await?.set_mode(mode).await
    }

    pub async fn get_available_modes(&self) -> Result<Vec<Modes>> {
        self.mode_proxy().await?.available_modes().await
    }

    // CONFIG INTERFACE

    async fn config_proxy(&self) -> Result<CardwireConfigProxy<'_>> {
        CardwireConfigProxy::new(self.proxy.connection()).await
    }

    pub async fn experimental_nvidia_block(&self) -> Result<bool> {
        self.config_proxy().await?.experimental_nvidia_block().await
    }
    pub async fn set_experimental_nvidia_block(&self, state: bool) -> Result<()> {
        self.config_proxy()
            .await?
            .set_experimental_nvidia_block(state)
            .await
    }

    pub async fn auto_apply_gpu_state(&self) -> Result<bool> {
        self.config_proxy().await?.auto_apply_gpu_state().await
    }
    pub async fn set_auto_apply_gpu_state(&self, state: bool) -> Result<()> {
        self.config_proxy()
            .await?
            .set_auto_apply_gpu_state(state)
            .await
    }

    pub async fn battery_auto_switch(&self) -> Result<bool> {
        self.config_proxy().await?.battery_auto_switch().await
    }

    pub async fn set_battery_auto_switch(&self, state: bool) -> Result<()> {
        self.config_proxy()
            .await?
            .set_battery_auto_switch(state)
            .await
    }

    pub async fn battery_auto_switch_mode(&self) -> Result<Modes> {
        self.config_proxy().await?.battery_auto_switch_mode().await
    }
    pub async fn set_battery_auto_switch_mode(&self, state: Modes) -> Result<()> {
        self.config_proxy()
            .await?
            .set_battery_auto_switch_mode(state)
            .await
    }

    pub async fn external_display_auto_switch(&self) -> Result<bool> {
        self.config_proxy()
            .await?
            .external_display_auto_switch()
            .await
    }

    pub async fn set_external_display_auto_switch(&self, state: bool) -> Result<()> {
        self.config_proxy()
            .await?
            .set_external_display_auto_switch(state)
            .await
    }

    // Smart Policy

    async fn smart_proxy(&self) -> Result<CardwireSmartPolicyProxy<'_>> {
        CardwireSmartPolicyProxy::new(self.proxy.connection()).await
    }

    #[allow(unused)]
    pub async fn get_app_policies(&self) -> Result<HashMap<String, DbusAppMetadata>> {
        self.smart_proxy().await?.get_app_policies().await
    }

    // GPU INTERFACE

    async fn build_gpu_proxy(&self, id: u32) -> Result<CardwireGpuProxy<'_>> {
        CardwireGpuProxy::builder(self.proxy.connection())
            .path(format!("/org/opengamingcollective/cardwire/Gpu/{}", id))?
            .build()
            .await
    }

    pub async fn get_gpu_objects(
        &self,
    ) -> Result<HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>>
    {
        let proxy = CardwireGpuIntProxy::new(self.proxy.connection()).await?;
        proxy.get_managed_objects().await
    }

    pub async fn get_gpu_device(&self, id: u32) -> zbus::Result<DbusGpuDevice> {
        self.build_gpu_proxy(id).await?.get_device().await
    }

    #[allow(unused)]
    pub async fn block(&self, id: u32) -> zbus::Result<bool> {
        self.build_gpu_proxy(id).await?.block().await
    }

    pub async fn set_block(&self, id: u32, state: bool) -> zbus::Result<()> {
        self.build_gpu_proxy(id).await?.set_block(state).await
    }

    pub async fn lsof(&self, id: u32) -> zbus::Result<HashMap<String, Vec<String>>> {
        self.build_gpu_proxy(id).await?.lsof().await
    }

    pub async fn power_state(&self, id: u32) -> zbus::Result<String> {
        self.build_gpu_proxy(id).await?.power_state().await
    }

    pub async fn env(&self, id: u32) -> zbus::Result<Vec<String>> {
        self.build_gpu_proxy(id).await?.env().await
    }

    #[allow(unused)]
    pub async fn launchable(&self, id: u32) -> zbus::Result<bool> {
        self.build_gpu_proxy(id).await?.launchable().await
    }

    // MANAGER INTERFACE

    pub async fn get_daemon_status(&self) -> zbus::Result<()> {
        let proxy = CardwireManagerProxy::new(self.proxy.connection()).await?;
        proxy.status().await
    }
}
