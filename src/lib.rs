use std::thread;
use std::sync::mpsc;

use log::{debug, info};

use crate::data::audioinput::AudioInput;

pub mod data;

enum HamSharkCommand {
    Pause,
    Resume,
    Stop,
}

#[derive(Default)]
pub struct HamShark {
    join_handle: Option<thread::JoinHandle<()>>,
    cmd_chan: Option<mpsc::Sender<HamSharkCommand>>,
    audio_input: Option<AudioInput>,
}

#[derive(Debug, Clone)]
enum HamSharkCommandError {
    ChannelDisconnected,
    ChannelGone
}

type HamSharkCommandResult = Result<(), HamSharkCommandError>;

#[derive(Debug, Clone)]
pub enum HamSharkError {
    NotStarted
}
pub type HamSharkResult = Result<(), HamSharkError>;

fn main_loop(cmd_chan: mpsc::Receiver<HamSharkCommand>) {
    info!("Hamshark Main Loop Started");
    let mut running = true;
    let mut count = 0;
    loop {
        // Process command queue
        match cmd_chan.try_recv() {
            Ok(HamSharkCommand::Pause) => {
                running = false;
            },
            Ok(HamSharkCommand::Resume) => {
                running = true;
            },
            Ok(HamSharkCommand::Stop) => {
                // TODO: cleanup
                break;
            },
            Err(mpsc::TryRecvError::Disconnected) => {
                // TODO: cleanup
            },
            Err(mpsc::TryRecvError::Empty) => ()
        }

        if running {
            count += 1;
            debug!("Inside Thread, count is {}", count);

            //let host = cpal::available_hosts
            // TODO: draw the rest of the owl
        }
    }
    info!("Hamshark Main Loop Terminated");
}

impl HamShark {
    pub fn new() -> Self {
        Self::default()
    }

    fn issue_command(&self, cmd: HamSharkCommand) -> HamSharkCommandResult {
        match &self.cmd_chan {
            Some(chan) => chan.send(cmd).or(
                Result::Err(HamSharkCommandError::ChannelDisconnected)
            ),
            None => Result::Err(HamSharkCommandError::ChannelGone)
        }
    }

    pub fn is_started(&self) -> bool {
        self.join_handle.is_some() && self.cmd_chan.is_some()
    }

    pub fn start(&mut self) {
        let (tx, rx) = mpsc::channel::<HamSharkCommand>();
        self.cmd_chan.replace(tx);
        self.join_handle.replace(thread::spawn(move || main_loop(rx)));
    }

    pub fn pause(&self) -> HamSharkResult {
        self.issue_command(HamSharkCommand::Pause).or(
            Result::Err(HamSharkError::NotStarted)
        )
    }

    pub fn resume(&self) -> HamSharkResult {
        self.issue_command(HamSharkCommand::Resume).or(
            Result::Err(HamSharkError::NotStarted)
        )
    }

    pub fn stop(&mut self) -> HamSharkResult {
        match self.issue_command(HamSharkCommand::Stop) {
            Ok(()) => {
                // At this point if we can't join, it's a panic anyway.
                self.join_handle
                    .take()
                    .expect("Did not have a join_handle when expected")
                    .join()
                    .expect("Could not join thread when expected");
                Ok(())
            },
            Err(HamSharkCommandError::ChannelDisconnected) =>
                // This should not have happened, panic
                panic!("Audio channel disconnected unexpectedly from inside the thread"),
            Err(HamSharkCommandError::ChannelGone) =>
                // This could be a user error although bad UI for not checking
                Result::Err(HamSharkError::NotStarted),
        }
    }

    pub fn update_audio_input(&mut self, audio_input: AudioInput) {
        let was_started = self.is_started();
        if was_started  {
            self.stop().expect("was_started true but failed to stop");
        }

        self.audio_input = Some(audio_input);

        if was_started {
            self.start();
        }
    }
}