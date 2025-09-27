use std::sync::mpsc::Sender;

use cpal::{traits::{DeviceTrait, StreamTrait}, Stream};
use log::error;
use thiserror::Error as ThisError;

use crate::{data::{audio::Samples, audioinput::AudioInputDevice}, pipeline::{self, Source, State}};

pub struct CpalSource {
    audioinputdevice: AudioInputDevice,
    stream: Option<Stream>,
    samples: Samples,
    on_data: Option<Sender<()>>,
}

impl From<AudioInputDevice> for CpalSource {
    fn from(value: AudioInputDevice) -> Self {
        Self {
            audioinputdevice: value,
            stream: None,
            samples: Default::default(),
            on_data: None,
        }
    }
}

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Pipeline Error: {0}")]
    PipelineError(#[from] pipeline::Error),
}

impl Source for CpalSource {
    type Error = Error;

    fn play(&mut self) -> Result<(), Self::Error> {
        if self.stream.is_some() {
            return Ok(())
        }

        let cfg = &self.audioinputdevice;

        match cfg.device.build_input_stream(
            &cfg.config,
            {

                // Closure reference to samples
                let samples = self.samples.clone();

                // Reference to on_data notify
                let on_data = self.on_data.clone();
                
                move |data: &[f32], info| {
                    // Copy the current samples into local memory
                    for sample in data {
                        samples.write().push(*sample);
                    }

                    // Notify any interested parties
                    if let Some(on_data) = &on_data {
                        let derp = samples.read();
                        let herp = [derp.len() - data.len() .. derp.len()];
                        on_data.send(());
                    }
                }
            },
            move |err| {
                error!("Stream Error: {:?}", err);
            },
            None,
        ) {
            Ok(stream) => {
                match stream.play() {
                    Ok(_) => {
                        self.stream = Some(stream);
                        Ok(())
                    },
                    Err(error) => Err(Self::Error::from(pipeline::Error::from(error))),
                }
            },
            Err(error) => {
                Err(Self::Error::from(pipeline::Error::from(error)))
            }
        }
    }

    fn pause(&mut self) -> Result<(), Self::Error> {
        Err(Self::Error::from(pipeline::Error::CannotPause(format!("{:?}", self.audioinputdevice))))
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        if let Some(stream) = self.stream.take() {
            stream.pause().ok();
            drop(stream);
        }
        Ok(())
    }

    fn can_pause(&self) -> bool {
        false
    }
    
    fn state(&self) -> State {
        match self.stream {
            Some(_) => State::Playing,
            None => State::Paused,
        }
    }
}