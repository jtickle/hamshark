use std::sync::Arc;
use cpal::{traits::{DeviceTrait, StreamTrait}, Stream};
use log::error;
use parking_lot::RwLock;
use thiserror::Error as ThisError;
use crate::data::{audio::{self, Clip}, audioinput::AudioInputDevice};

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Error playing input stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
    #[error("Error building input stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("Error during input stream: {0}")]
    DuringStream(#[from] cpal::StreamError),
    #[error("Error working with audio clip: {0}")]
    Audio(#[from] audio::Error),
}

pub struct SampleRecorder {
    stream: Stream,
    write_error: Arc<RwLock<Option<Error>>>,
}


impl SampleRecorder {
    pub fn new(audioinput: &AudioInputDevice, clip: Clip) -> Result<Self, Error> {
        let write_error = Arc::new(RwLock::new(None));

        let stream = match audioinput.device.build_input_stream(
            &audioinput.config,
            {
                let write_error = write_error.clone();
                move |data: &[f32], _info| {
                    if write_error.read().is_some() { return };

                    let mut clip_guard = clip.write();
                    if let Err(error) = clip_guard.write_samples(data) {
                        *write_error.write() = Some(Error::from(error));
                    }
                }
            },
            {
                let write_error = write_error.clone();
                move |err| {
                    if write_error.read().is_some() { return };
                    *write_error.write() = Some(Error::from(err));
                }
            },
            None
        ) {
            Ok(stream) => {
                match stream.play() {
                    Ok(_) => stream,
                    Err(err) => return Err(Error::from(err)),
                }
            },
            Err(err) => return Err(Error::from(err)),
        };

        Ok(Self {
            stream,
            write_error,
        })
    }

    pub fn close(self) -> Result<(), Error> {
        self.stream.pause().ok();
        drop(self.stream);

        Ok(())
    }
}


pub struct SampleLoader {
    stream: Stream,
    read_error: Arc<RwLock<Option<Error>>>,
}

/*impl SampleLoader {
    pub fn new (clip: Clip) -> Result<Self, Error> {
        let read_error = Arc::new(RwLock::new(None));

        // TODO: build loading thread

        Ok(Self {
            stream,
            read_error,
        })
    }
}*/