use std::sync::Arc;

use cpal::{
    Stream,
    traits::{DeviceTrait, StreamTrait},
};
use log::error;
use parking_lot::RwLock;

use crate::{
    data::audioinput::AudioInputDevice,
    pipeline::{Element, Error, Pipeline, Sink, Source, State},
};

static NAME: &str = "CpalSource";

pub struct CpalSource {
    audioinputdevice: AudioInputDevice,
    stream: Option<Stream>,
    next: Option<Arc<RwLock<dyn Sink>>>,
}

impl From<AudioInputDevice> for CpalSource {
    fn from(value: AudioInputDevice) -> Self {
        Self {
            audioinputdevice: value,
            stream: None,
            next: None,
        }
    }
}

impl Pipeline for CpalSource {
    fn play(&mut self) -> Result<(), Error> {
        if self.stream.is_some() {
            return Ok(());
        }

        let cfg = &self.audioinputdevice;

        // Make sure pipeline is complete
        if !(self as &dyn Source).is_complete() {
            return Err(Error::Incomplete(NAME.to_string()));
        }

        match cfg.device.build_input_stream(
            &cfg.config,
            {
                // Closure reference to next element
                let next = self.next.clone().unwrap();

                move |data: &[f32], _info| {
                    // Notify next pipeline element
                    next.write().process(data).unwrap();
                }
            },
            move |err| {
                error!("Stream Error: {:?}", err);
            },
            None,
        ) {
            Ok(stream) => match stream.play() {
                Ok(_) => {
                    self.stream = Some(stream);
                    Ok(())
                }
                Err(error) => Err(Error::platform(&error)),
            },
            Err(error) => Err(Error::platform(&error)),
        }
    }

    fn pause(&mut self) -> Result<(), Error> {
        Err(Error::CannotPause(format!("{:?}", self.audioinputdevice)))
    }

    fn stop(&mut self) -> Result<(), Error> {
        if let Some(stream) = self.stream.take() {
            stream.pause().ok();
            drop(stream);
        }
        Ok(())
    }

    fn can_pause(&self) -> bool {
        false
    }

    fn is_recorder(&self) -> bool {
        true
    }

    fn state(&self) -> State {
        match self.stream {
            Some(_) => State::Playing,
            None => State::Paused,
        }
    }
}

impl Source for CpalSource {
    fn next_element(&self) -> Option<Arc<RwLock<dyn Sink>>> {
        self.next.clone()
    }

    fn set_next_element(&mut self, element: Arc<RwLock<dyn Sink>>) {
        self.next = Some(element);
    }
}
