use anyhow::Result;
use rust_async_tuyapi::tuyadevice::TuyaDevice;
use rust_async_tuyapi::{Payload, PayloadStruct};
use serde_json::json;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct BulbConfig {
    pub device_id: String,
    pub local_key: String,
    pub ip: String,
    pub version: String,
}

impl BulbConfig {
    pub fn new(device_id: impl Into<String>, local_key: impl Into<String>, ip: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            local_key: local_key.into(),
            ip: ip.into(),
            version: version.into(),
        }
    }
}

pub struct BulbController {
    device: TuyaDevice,
    device_id: String,
}

impl BulbController {
    /// Create a new bulb controller
    pub fn new(config: BulbConfig) -> Result<Self> {
        let device = TuyaDevice::new(
            &config.version,
            &config.device_id,
            Some(&config.local_key),
            IpAddr::from_str(&config.ip)?,
        )?;

        Ok(Self {
            device,
            device_id: config.device_id,
        })
    }

    /// Connect to the bulb
    pub async fn connect(&mut self) -> Result<()> {
        let _rx = self.device.connect().await?;
        Ok(())
    }

    /// Set the bulb color using HSV values
    ///
    /// h - Hue (0-360)
    /// s - Saturation (0-1000)
    /// v - Brightness (0-1000)
    /// immediate - If true, set the color immediately without transition
    pub async fn set_color(&mut self, h: u16, s: u16, v: u16, immediate: bool) -> Result<()> {
        let immediate_num = if immediate { 0 } else { 1 };

        let mut dps = HashMap::new();
        dps.insert("20".to_string(), json!(true)); // make sure it's on
        dps.insert("28".to_string(), json!(format!("{}{}00000000", immediate_num, hsv_to_hex(h, s, v)))); // real time set color to avoid gradient transition

        self.send_commands(dps).await
    }

    fn create_payload(&self, dps: &HashMap<String, serde_json::Value>) -> Payload {
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        Payload::Struct(PayloadStruct {
            dev_id: self.device_id.clone(),
            gw_id: Some(self.device_id.clone()),
            uid: None,
            t: Some(current_time.to_string()),
            dp_id: None,
            dps: Some(serde_json::to_value(dps).unwrap()),
        })
    }

    /// Send commands to the bulb
    /// Automatically reconnects and retries once if the command fails
    pub async fn send_commands(&mut self, dps: HashMap<String, serde_json::Value>) -> Result<()> {
        if let Err(_) = self.device.set(self.create_payload(&dps)).await {
            println!("Reconnecting to bulb...");
            // connection likely failed or was dropped. reconnect and try again
            self.connect().await?;
            println!("Reconnected. Retrying command...");
            self.device.set(self.create_payload(&dps)).await?;
        }

        Ok(())
    }
}

fn hsv_to_hex(h: u16, s: u16, v: u16) -> String {
    format!("{:04x}{:04x}{:04x}", h, s, v)
}

/// Maps MIDI CC value (0-127) to Hue (0-360)
pub fn midi_to_hue(midi_value: u8) -> u16 {
    (midi_value as u16 * 360) / 127
}
