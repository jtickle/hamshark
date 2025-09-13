use std::{path::{Path, PathBuf}, sync::{Arc}};
use crate::{config::Settings, data::audioinput::AudioInputDevice};
use chrono::{DateTime, Local};
use cpal::{traits::{DeviceTrait, StreamTrait}, InputStreamTimestamp, Stream, StreamInstant};
use log::{debug, error, info};
use parking_lot::RwLock;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::{fs, io};
use thiserror::Error;

const SESSIONFILE: &str = "session.toml";
const FFTSIZE: usize = 128;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Tried to start but was already running")]
    AlreadyRunning(),
    #[error("Tried to stop but was not running")]
    NotRunning(),
    #[error("No audio configuration provided")]
    NoAudioConfiguration(),
    #[error("Error building input stream: {0}")]
    BuildStreamError(#[source] cpal::BuildStreamError),
    #[error("Error playing input stream: {0}")]
    PlayStreamError(#[source] cpal::PlayStreamError),
}

pub struct Session {
    pub path: PathBuf,
    fft: Arc<dyn Fft<f32>>,
    audioconfig: Option<AudioInputDevice>,
    stream: Option<Stream>,

    raw_amplitudes: Arc<RwLock<Vec<f32>>>,
    fft_results: Arc<RwLock<Vec<Vec<Complex<f32>>>>>,
    timestamps: Arc<RwLock<Vec<StreamInstant>>>,
}

fn create_base_path_by_datetime(base: &Path, now: DateTime<Local>) -> Result<PathBuf, io::Error> {
    let formatted = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let session_path = base.join(formatted);
    info!("Creating session directory {:?}", session_path.as_os_str());
    fs::create_dir_all(session_path.as_path())?;
    return Ok(session_path);
}

impl Session {
    pub fn from_settings(settings: &Settings) -> Result<Session, io::Error> {
        let base_dir = settings.session_base_dir.as_path();
        let now = Local::now();
        let path = create_base_path_by_datetime(base_dir, now)?;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFTSIZE);

        Ok(Session {
            path,
            fft,
            audioconfig: None,
            stream: None,
            raw_amplitudes: Arc::new(RwLock::new(Vec::new())),
            fft_results: Arc::new(RwLock::new(Vec::new())),
            timestamps: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub fn configure(&mut self, newconfig: AudioInputDevice) -> Result<(), Error> {
        if let Some(config) = &self.audioconfig {
            if config == &newconfig {
                return Ok(());
            }
        }
        let was_started = self.is_started();

        if was_started {
            self.stop()?;
        }

        self.audioconfig = Some(newconfig);
        debug!("Session configured with audio input device {:?}", self.audioconfig);

        if was_started {
            self.start()?;
        }

        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.audioconfig.is_some()
    }

    pub fn is_started(&self) -> bool {
        self.stream.is_some()
    }

    pub fn start(&mut self) -> Result<(), Error> {
        if self.is_started() {
            return Err(Error::AlreadyRunning())
        }

        if !self.is_configured() {
            return Err(Error::NoAudioConfiguration())
        }

        let cfg = self.audioconfig.as_ref().unwrap();

        let mut last_ts: Option<InputStreamTimestamp> = None;
        let mut buffer = vec![Complex{ re: 0.0, im: 0.0}; FFTSIZE];
        let mut buffer_index = 0usize;
        let mut data_index = 0usize;
        let fft = Arc::clone(&self.fft);
        let raw_amplitudes = Arc::clone(&self.raw_amplitudes);
        let fft_results = Arc::clone(&self.fft_results);
        self.stream = match cfg.device.build_input_stream(
            &cfg.config,
            move |data: &[f32], info| {

                // Profiling Data
                let cur_ts = info.timestamp();
                let dt = if let Some(last_ts) = last_ts {
                    (
                        cur_ts.capture.duration_since(&last_ts.capture).unwrap().as_secs_f64(),
                        cur_ts.callback.duration_since(&last_ts.callback).unwrap().as_secs_f64()
                    )
                } else {
                    (0.0f64, 0.0f64)
                };
                last_ts = Some(cur_ts);
                debug!("Input Stream dt_cap={} dt_cb={} ts={:?} Length: {}", dt.0, dt.1, info.timestamp(), data.len());
                debug!("Indexes: Buffer {} Data {}", buffer_index, data_index);

                loop {

                    // Every time through the loop, add the next data to buffer
                    buffer[buffer_index] = Complex::from(data[data_index]);

                    // Every time through the loop, add the next data to the recorded data
                    raw_amplitudes.write().push(data[data_index]);

                    buffer_index += 1;
                    data_index += 1;

                    // When the buffer is full, do a FFT, which writes back to our buffer
                    if buffer_index == buffer.len() {
                        buffer_index = 0;
                        fft.process(&mut buffer);
                        // Whenever an FFT is generated, add it to recorded FFTs
                        fft_results.write().push(buffer.clone());
                        debug!("post fft {:?}", buffer);
                    }

                    // When we have consumed all the data, reset counter and wait for the next callback
                    if data_index == data.len() {
                        data_index = 0;
                        break;
                    } 
                }
            },
            move |err| {
                error!("Stream Error: {:?}", err);
            },
            None
        ) {
            Ok(stream) => {
                // Start the stream and store it if successful
                match stream.play() {
                    Ok(_) => Some(stream),
                    Err(error) => return Err(Error::PlayStreamError(error)),
                }
            }
            Err(error) => {
                return Err(Error::BuildStreamError(error))
            },
        };

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        if !self.is_started() {
            return Err(Error::NotRunning())
        }

        let stream = self.stream.take().unwrap();
        stream.pause().unwrap();
        drop(stream);

        Ok(())
    }
}