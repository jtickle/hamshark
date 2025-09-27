use log::error;
use thiserror::Error as ThisError;

pub mod cpalsource;

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
}
pub trait Source {
    type Error;

    fn play(&mut self) -> Result<(), Self::Error>;
    fn pause(&mut self) -> Result<(), Self::Error>;
    fn stop(&mut self) -> Result<(), Self::Error>;
    fn can_pause(&self) -> bool;
    fn state(&self) -> State;
}