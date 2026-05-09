//! helper to manage cardwired configs, include the user config .toml, and the .json states like
//! gpu, mode or pci
use crate::{
    file::common::{FileKind, create_default_file}, models::Modes
};
use anyhow::{Context, Ok};
use cardwire_core::gpu::{GpuBlocker, GpuDevice, is_gpu_blocked};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};
const STATE_PATH: &str = "/var/lib/cardwire";

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct CardwireModeState {
    mode: Modes,
}
impl Default for CardwireModeState {
    fn default() -> Self {
        CardwireModeState {
            mode: Modes::Manual,
        }
    }
}

impl CardwireModeState {
    /// Read a mode.json file and return into a struct
    pub fn build() -> anyhow::Result<CardwireModeState> {
        let mode_file = format!("{STATE_PATH}/mode.json");

        let mode = Self::parse_mode_state(&mode_file);
        if let Err(e) = mode {
            warn!("mode.json could not get parsed {e}, overwriting with default one...");
            Self::create_default_mode()?;
        }
        let mode = Self::parse_mode_state(&mode_file).context("couldn't fix mode.json")?;
        Ok(mode)
    }
    fn parse_mode_state(mode_file: &str) -> anyhow::Result<CardwireModeState> {
        if !(fs::exists(mode_file)?) {
            Self::create_default_mode()?;
        }
        let mode_state = fs::read_to_string(mode_file)?;
        let string_content: CardwireModeState =
            serde_json::from_str(&mode_state).context("Failed to parse json to str")?;
        Ok(string_content)
    }
    fn create_default_mode() -> anyhow::Result<()> {
        create_default_file(FileKind::ModeState)?;
        Ok(())
    }
    pub fn mode(&self) -> Modes {
        self.mode
    }
    /// Save the new mode into the daemon and to the mode_state.json file
    pub async fn save_state(&mut self, new_mode: Modes) -> anyhow::Result<()> {
        // Save to daemon state
        self.mode = new_mode;
        // Save the whole state into the json
        let state_file = serde_json::to_string_pretty(&self)?;
        tokio::fs::write(format!("{STATE_PATH}/mode.json"), state_file).await?;
        Ok(())
    }
}

// GPU PART
// This is the easiest way i found to have a good looking json, might change later
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct CardwireGpuState {
    gpu: BTreeMap<String, CardwireGpuUnit>,
}
impl Default for CardwireGpuState {
    fn default() -> Self {
        let mut map: BTreeMap<String, CardwireGpuUnit> = BTreeMap::new();
        map.insert("Null".to_string(), CardwireGpuUnit::default());
        Self { gpu: map }
    }
}
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CardwireGpuUnit {
    block: bool,
}

impl CardwireGpuState {
    /// Build a CardwireGpuState struct
    pub fn build() -> anyhow::Result<CardwireGpuState> {
        let state_file = format!("{STATE_PATH}/gpu_state.json");

        let gpu_hash = Self::parse_gpu_state(&state_file);
        if let Err(e) = gpu_hash {
            warn!("gpu_hash.json could not get parsed {e}, overwriting with default one...");
            Self::create_default_state()?;
        }
        let gpu_hash = Self::parse_gpu_state(&state_file).context("couldn't fix gpu_hash.json")?;
        let gpu_state = CardwireGpuState { gpu: gpu_hash };
        Ok(gpu_state)
    }
    // Parse directly into CardwireGpuState
    fn parse_gpu_state(state_file: &str) -> anyhow::Result<BTreeMap<String, CardwireGpuUnit>> {
        if !(fs::exists(state_file)?) {
            Self::create_default_state().context("Could not create default gpu_state.json")?;
        }
        let gpu_state = fs::read_to_string(state_file)
            .with_context(|| format!("Could not read file {}", state_file))?;

        let content: BTreeMap<String, CardwireGpuUnit> =
            serde_json::from_str(&gpu_state).context("Could not parse string into json")?;
        Ok(content)
    }
    /// Create default gpu_state.json, including folders if missing
    fn create_default_state() -> anyhow::Result<()> {
        create_default_file(FileKind::GpuState)?;
        Ok(())
    }
    /// Save the new state into the daemon and to the gpu_state.json file
    pub async fn save_state(
        &mut self,
        gpu_list: &BTreeMap<usize, GpuDevice>,
        blocker: &GpuBlocker,
    ) -> anyhow::Result<()> {
        // Prevent overwriting default config if it's not replaceable
        if self.gpu.contains_key("Null") {
            info!("detected default gpu_state file, overwriting it...");
            self.gpu.clear();
        }
        // Save to daemon state
        for gpu in gpu_list.values() {
            self.gpu.insert(
                gpu.pci.pci_address().to_string(),
                CardwireGpuUnit {
                    block: is_gpu_blocked(blocker, gpu)?,
                },
            );
        }
        // Save the whole hashmap into json
        let state_file = serde_json::to_string_pretty(&self.gpu)?;
        tokio::fs::write(format!("{STATE_PATH}/gpu_state.json"), state_file).await?;
        Ok(())
    }
    /// return true if it was generated by default func
    pub fn is_default_state(&self) -> bool {
        self.gpu.contains_key("Null")
    }
    /// search key in gpu hashmap,
    pub fn gpu_block_state(&self, pci: &String) -> bool {
        match self.gpu.get_key_value(pci) {
            Some(value) => value.1.block,
            None => false,
        }
    }
}
