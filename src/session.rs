use std::{path::{Path, PathBuf}, sync::Arc};
use crate::{config::Settings, data::{audio::Clip, audioinput::AudioInputDevice}};
use chrono::{DateTime, Local};
use cpal::{traits::{DeviceTrait, StreamTrait}, InputStreamTimestamp, Stream};
use hound::{SampleFormat, WavSpec};
use log::{debug, error, info, trace};
use parking_lot::RwLock;
use rand::prelude::*;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::{fs, io};
use thiserror::Error;

const SESSIONFILE: &str = "session.toml";
const FFTSIZE: usize = 128;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Tried to record new clip but was already recording")]
    AlreadyRecording(),
    #[error("Tried to stop but was not running")]
    NotRunning(),
    #[error("No audio configuration provided")]
    NoAudioConfiguration(),
    #[error("Error scanning session directory: {0}")]
    DirectoryRead(#[source] io::Error),
    #[error("Error creating clip: {0}")]
    CreateClip(#[from] hound::Error),
}

pub struct Session {
    pub path: PathBuf,
    clips: Vec<Arc<RwLock<Clip>>>,

    fft: Arc<dyn Fft<f32>>,
    audioconfig: Option<AudioInputDevice>,
    stream: Option<Stream>,

    raw_amplitudes: Arc<RwLock<Vec<f32>>>,
    fft_results: Arc<RwLock<Vec<Vec<Complex<f32>>>>>,
}

fn create_filename_from_now() -> String {
    Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

fn create_base_path_by_datetime(base: &Path) -> Result<PathBuf, io::Error> {
    let formatted = create_filename_from_now();
    let session_path = base.join(formatted);
    info!("Creating session directory {:?}", session_path.as_os_str());
    fs::create_dir_all(session_path.as_path())?;
    return Ok(session_path);
}

impl Session {
    pub fn from_settings(settings: &Settings) -> Result<Session, hound::Error> {
        let base_dir = settings.session_base_dir.as_path();
        let now = Local::now();
        let path = create_base_path_by_datetime(base_dir)?;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFTSIZE);

        let mut session = Session {
            path,
            clips: Default::default(),
            fft,
            audioconfig: None,
            stream: None,
            raw_amplitudes: Arc::new(RwLock::new(Vec::new())),
            fft_results: Arc::new(RwLock::new(Vec::new())),
        };

        session.rescan_clips()?;

        Ok(session)
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

    pub fn configuration(&self) -> Option<AudioInputDevice> {
        self.audioconfig.as_ref().map(|x| x.clone())
    }

    pub fn is_started(&self) -> bool {
        self.stream.is_some()
    }

    pub fn clips(&self) -> &Vec<Arc<RwLock<Clip>>> {
        &self.clips
    }

    pub fn open_clip(&self, clip: &Clip) -> Result<(), hound::Error> {
        for clip_arc in &self.clips {
            let path = {
                let arc_read = clip_arc.read();
                arc_read.path.clone()
            };
            if path == clip.path {
                let mut dest = clip_arc.write();
                dest.open();
            }
        }
        Ok(())
    }

    pub fn update_clip(&mut self, path: &Path) {
        for clip in self.clips() {
            if clip.read().path == path {
                return;
            }
        }
        self.clips.push(Arc::new(RwLock::new(Clip::from(path))));
    }

    pub fn rescan_clips(&mut self) -> Result<(), hound::Error> {
        for result in fs::read_dir(self.path.as_path())? {
            let entry = result?;
            if entry.file_type()?.is_file() {
                let pathbuf = PathBuf::from(entry.file_name());
                if let Some(ext) = pathbuf.extension() {
                    if ext == "wav" {
                        self.update_clip(pathbuf.as_path())
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_recording_clip(&self) -> Option<Arc<RwLock<Clip>>> {
        for clip in &self.clips {
            if clip.read().is_writable() {
                return Some(clip.clone());
            }
        }
        None
    }

    pub fn record_new_clip(&mut self) -> Result<(), Error> {
        if self.get_recording_clip().is_some() {
            return Err(Error::AlreadyRecording());
        }
        if !self.is_configured() {
            return Err(Error::NoAudioConfiguration());
        }
        let cfg = self.audioconfig.as_ref().unwrap();

        let mut clipname = self.path.clone();
        clipname.push(create_filename_from_now());
        clipname.set_extension("wav");
        
        let spec = WavSpec {
            channels: 1,
            sample_rate: cfg.config.sample_rate.0,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int
        };
        let clip = Arc::new(RwLock::new(Clip::new(clipname.as_path(), spec)?));
        self.clips.push(clip.clone());

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), Error> {
        if self.is_started() {
            return Err(Error::AlreadyRecording())
        }

        // Build filename for new clip
        let clip = self.record_new_clip()?;

        if !self.is_configured() {
            return Err(Error::NoAudioConfiguration())
        }
        let cfg = self.audioconfig.as_ref().unwrap();

        /*let mut last_ts: Option<InputStreamTimestamp> = None;
        let mut buffer = vec![Complex{ re: 0.0, im: 0.0}; FFTSIZE];
        let mut buffer_index = 0usize;
        let mut data_index = 0usize;
        let fft = Arc::clone(&self.fft);
        let samples = clip.read().samples();
        let frequencies = clip.read().frequencies();
        self.stream = match cfg.device.build_input_stream(
            &cfg.config,
            move |data: &[f32], info| {
                let mut writable_clip = clip.write();
                let writer = writable_clip.writer().unwrap();

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
                trace!("Input Stream dt_cap={} dt_cb={} ts={:?} Length: {}", dt.0, dt.1, info.timestamp(), data.len());
                trace!("Indexes: Buffer {} Data {}", buffer_index, data_index);

                loop {

                    // Every time through the loop, add the next data to buffer
                    buffer[buffer_index] = Complex::from(data[data_index]);

                    // Every time through the loop, add the next data to the recorded data
                    samples.write().push(data[data_index]);

                    // Write some BS so we can see progress without a mic for devtest only
                    // TODO: temporary
                    // let mut rng = rand::rng();
                    // let rndval: f32 = rng.random_range(-0.5..0.5);
                    // raw_amplitudes.write().push(rndval);

                    // Write a sample
                    writer.write_sample((data[data_index] * i16::MAX as f32) as i16)
                        .expect("to write a sample");

                    buffer_index += 1;
                    data_index += 1;

                    // When the buffer is full, do a FFT, which writes back to our buffer
                    if buffer_index == buffer.len() {
                        buffer_index = 0;
                        fft.process(&mut buffer);
                        // Whenever an FFT is generated, add it to recorded FFTs
                        frequencies.write().push(buffer.clone());
                        trace!("post fft {:?}", buffer);
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
                    Err(error) => return Err(Error::PlayStream(error)),
                }
            }
            Err(error) => {
                return Err(Error::BuildStream(error))
            },
        };*/

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

    pub fn samples(&self) -> Arc<RwLock<Vec<f32>>> {
        self.raw_amplitudes.clone()
    }

    pub fn fft(&self) -> Arc<RwLock<Vec<Vec<Complex<f32>>>>> {
        self.fft_results.clone()
    }
}