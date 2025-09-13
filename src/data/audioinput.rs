use std::path::PathBuf;
use std::fmt;

use cpal::{available_hosts, default_host, host_from_id, traits::HostTrait, Device, Host, HostId};

pub struct AudioInputDevice {
    pub host: Host,
    pub device: Device,
}

pub enum AudioInput {
    FromFile(PathBuf),
    FromDevice(AudioInputDevice)
}

#[derive(Copy, Clone, PartialEq)]
pub enum AudioInputBuilderSource {
    FromFile,
    FromDevice,
}

impl fmt::Display for AudioInputBuilderSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::FromFile => write!(f, "From File"),
            Self::FromDevice => write!(f, "From Device")
        }
    }
}

pub struct AudioInputBuilder {
    pub source: AudioInputBuilderSource,
    pub file: Option<PathBuf>,
    pub host: HostId,
    pub device: Option<Device>,
    pub file_base_path: Option<PathBuf>,
    pub hosts: Vec<HostId>,
    pub devices: Vec<Device>,
}

impl Clone for AudioInputBuilder {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            file: self.file.clone(),
            host: self.host.clone(),
            device: self.device.clone(),
            file_base_path:
            self.file_base_path.clone(),
            hosts: self.hosts.clone(),
            devices: self.devices.clone() }
    }
}

impl Default for AudioInputBuilder {
    fn default() -> Self {
        let host = default_host();
        let device = host.default_input_device();
        let hosts = available_hosts();
        let devices = match host.input_devices() {
            Ok(devices) => devices.collect(),
            Err(_) => Vec::<Device>::new(),
        };
        Self {
            source: AudioInputBuilderSource::FromDevice,
            file: None,
            host: host.id(),
            device,
            file_base_path: None,
            hosts,
            devices,
        }
    }
}

#[derive(Debug)]
pub struct AudioInputBuilderIncomplete;

impl AudioInputBuilder {
    pub fn build(&self) -> Result<AudioInput, AudioInputBuilderIncomplete> {
        match self.source {
            AudioInputBuilderSource::FromFile => match &self.file {
                Some(file) => Result::Ok(
                    AudioInput::FromFile(file.clone())
                ),
                None => {
                    Result::Err(AudioInputBuilderIncomplete{})
                },
            },
            AudioInputBuilderSource::FromDevice => match &self.device {
                Some(device) =>
                    match host_from_id(self.host) {
                        Ok(host) => Result::Ok(
                            AudioInput::FromDevice(
                                AudioInputDevice {
                                    host,
                                    device: device.clone()
                                }
                            )
                        ),
                        Err(_) => {
                            Result::Err(AudioInputBuilderIncomplete{})
                        },
                    },
                None => {
                    Result::Err(AudioInputBuilderIncomplete{})
                },
            },
        }
    }
}