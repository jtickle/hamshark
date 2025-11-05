use cpal::{
    BufferSize, Device, Host, HostId, StreamConfig, default_host, host_from_id,
    traits::{DeviceTrait, HostTrait},
};
use std::fmt::Debug;

pub struct AudioInputDevice {
    pub host: Host,
    pub device: Device,
    pub config: StreamConfig,
}

impl Clone for AudioInputDevice {
    fn clone(&self) -> Self {
        Self {
            host: host_from_id(self.host.id()).expect("host id to exist"),
            device: self.device.clone(),
            config: self.config.clone(),
        }
    }
}

impl Debug for AudioInputDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioInputDevice")
            .field("host", &self.host.id())
            .field("device", &self.device.name())
            .field("config", &self.config)
            .finish()
    }
}

impl PartialEq for AudioInputDevice {
    fn eq(&self, other: &Self) -> bool {
        self.host.id() == other.host.id()
            && self.device.name() == other.device.name()
            && self.config == other.config
    }
}

#[derive(Clone)]
pub struct AudioInputDeviceBuilder {
    pub host_id: HostId,
    pub device: Option<Device>,
    pub config: Option<StreamConfig>,
}

impl Default for AudioInputDeviceBuilder {
    fn default() -> Self {
        Self {
            host_id: default_host().id(),
            device: None,
            config: None,
        }
        .with_default_device()
        .with_default_config()
    }
}

#[derive(Debug)]
pub struct AudioInputBuilderIncomplete;

impl From<AudioInputDevice> for AudioInputDeviceBuilder {
    fn from(value: AudioInputDevice) -> Self {
        AudioInputDeviceBuilder {
            host_id: value.host.id(),
            device: Some(value.device.clone()),
            config: Some(value.config.clone()),
        }
    }
}

impl AudioInputDeviceBuilder {
    pub fn get_default_device(&self) -> Option<Device> {
        let host = host_from_id(self.host_id).expect("the host ID to have been set sensibly");
        host.default_input_device()
    }
    pub fn with_default_device(mut self) -> Self {
        self.device = self.get_default_device();
        self
    }

    pub fn get_default_config(&self) -> Option<StreamConfig> {
        self.device.clone().map(|device| {
            let mut config = device
                .default_input_config()
                .expect("device has not default input config")
                .config();
            config.buffer_size = BufferSize::Fixed(128);
            config
        })
    }
    pub fn with_default_config(mut self) -> Self {
        self.config = self.get_default_config();
        self
    }

    pub fn input_devices(&self) -> Vec<Device> {
        let host = host_from_id(self.host_id).expect("host must be set at this point");
        host.input_devices()
            .expect("host must have some input devices")
            .collect()
    }

    pub fn build(&self) -> Result<AudioInputDevice, AudioInputBuilderIncomplete> {
        let host = match host_from_id(self.host_id) {
            Ok(host) => host,
            Err(_) => return Result::Err(AudioInputBuilderIncomplete {}),
        };
        let device = match self.device.clone() {
            Some(device) => device,
            None => return Result::Err(AudioInputBuilderIncomplete {}),
        };
        let config = match self.config.clone() {
            Some(config) => config,
            None => return Result::Err(AudioInputBuilderIncomplete {}),
        };
        Ok(AudioInputDevice {
            host,
            device,
            config,
        })
    }
}
