use crate::display::{GpuDevice, PciDevice};
use std::collections::BTreeMap;

use zbus::{Proxy, connection::Connection};
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

    pub async fn set_mode(&self, mode: &String) -> zbus::Result<()> {
        self.proxy.call("SetMode", &(mode,)).await
    }

    pub async fn get_mode(&self) -> zbus::Result<String> {
        self.proxy.call("GetMode", &()).await
    }

    pub async fn list_devices(&self) -> zbus::Result<BTreeMap<usize, GpuDevice>> {
        self.proxy.call("ListDevices", &()).await
    }
    pub async fn list_devices_pci(&self) -> zbus::Result<BTreeMap<String, PciDevice>> {
        self.proxy.call("ListDevicesPci", &()).await
    }

    pub async fn set_gpu_block(&self, id: u32, blocked: bool) -> zbus::Result<()> {
        self.proxy.call("SetGpuBlock", &(id, blocked)).await
    }
}
