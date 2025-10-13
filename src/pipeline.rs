use std::sync::Arc;

use log::error;
use parking_lot::RwLock;
use thiserror::Error as ThisError;

pub mod cpalsource;
pub mod samplebuffer;
pub mod wavsink;

#[derive(Clone, Copy, Debug)]
pub enum State {
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Pipeline Source does not support pausing: {0}")]
    CannotPause(String),
    #[error("Error playing input stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
    #[error("Error building input stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("Pipeline is Incomplete - {0} requires a next_element but it is None")]
    Incomplete(String),
    #[error("A more specific platform error occurred: {0}")]
    PlatformError(String),
}

impl Error {
    fn platform(dbg: &dyn std::fmt::Debug) -> Error {
        Error::PlatformError(format!("{:?}", dbg))
    }
}

pub trait Pipeline {
    fn play(&mut self) -> Result<(), Error>;
    fn pause(&mut self) -> Result<(), Error>;
    fn stop(&mut self) -> Result<(), Error>;
    fn can_pause(&self) -> bool;
    fn is_recorder(&self) -> bool;
    fn state(&self) -> State;
}

pub trait Element {
    fn is_complete(&self) -> bool;
}

pub trait Sink: Send + Sync {
    fn process(&mut self, data: &[f32]) -> Result<(), Error>;
    fn cleanup(self) -> Result<(), Error>;
}

pub trait Source {
    fn next_element(&self) -> Option<Arc<RwLock<dyn Sink>>>;
    fn set_next_element(&mut self, element: Arc<RwLock<dyn Sink>>);
}

impl Element for dyn Sink {
    fn is_complete(&self) -> bool {
        true
    }
}

impl Element for dyn Source {
    fn is_complete(&self) -> bool {
        match self.next_element() {
            Some(next) => next.read().is_complete(),
            None => false,
        }
    }
}