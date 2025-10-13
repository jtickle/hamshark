use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc};
use crate::{config::Settings, data::{audio::{self, Clip, ClipId, WavClip}, audioinput::AudioInputDevice}, pipeline, tools::{self, SampleRecorder}};
use chrono::{Local};
use hound::{SampleFormat, WavSpec};
use log::{debug, error, info};
use parking_lot::RwLock;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::{fs, io};
use thiserror::Error as ThisError;

const SESSIONFILE: &str = "session.toml";
const FFTSIZE: usize = 128;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Tried to record new clip but was already recording")]
    AlreadyRecording(),
    #[error("No audio configuration provided")]
    NoAudioConfiguration(),
    #[error("No such clip ID {0}")]
    NoSuchClip(ClipId),
    #[error("Clip Already Exists {0}")]
    ClipAlreadyExists(ClipId),
    #[error("Error scanning session directory: {0}")]
    DirectoryRead(#[source] io::Error),
    #[error("Error creating clip: {0}")]
    CreateClip(#[from] hound::Error),
    #[error("Pipeline Error: {0}")]
    Pipeline(#[from] pipeline::Error),
    #[error("Recording Error: {0}")]
    Recording(#[from] tools::Error),
    #[error("Audio Error: {0}")]
    Audio(#[from] audio::Error),
    #[error("IO Error: {0}")]
    IO(#[from] io::Error),
}

pub type Frequencies = Arc<RwLock<Vec<Vec<Complex<f32>>>>>;

pub struct Session {
    pub path: PathBuf,
    clips: BTreeMap<ClipId, Clip>,

    recorder: Option<SampleRecorder>,

    fft: Arc<dyn Fft<f32>>,
    audioconfig: Option<AudioInputDevice>,
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
    pub fn from_settings(settings: &Settings) -> Result<Session, Error> {
        let base_dir = settings.session_base_dir.as_path();
        let path = create_base_path_by_datetime(base_dir)?;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFTSIZE);

        let mut session = Session {
            path,
            clips: Default::default(),
            recorder: None,
            fft,
            audioconfig: None,
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
        let was_recording = self.is_recording();

        if was_recording {
            self.stop_recording()?;
        }

        self.audioconfig = Some(newconfig);
        debug!("Session configured with audio input device {:?}", self.audioconfig);

        if was_recording {
            self.record_new_clip()?;
        }

        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.audioconfig.is_some()
    }

    pub fn configuration(&self) -> Option<AudioInputDevice> {
        self.audioconfig.as_ref().map(|x| x.clone())
    }

    pub fn clip_ids(&self) -> Vec<ClipId> {
        self.clips.keys().cloned().collect()
    }

    pub fn clips(&self) -> Vec<Clip> {
        self.clips.values().cloned().collect()
    }

    pub fn clip_id_to_abs_path(&self, clip_id: &ClipId) -> PathBuf {
        clip_id.absolute_path_wav(&self.path)
    }

    pub fn rescan_clips(&mut self) -> Result<(), Error> {
        for result in fs::read_dir(self.path.as_path())? {
            let entry = result?;
            if entry.file_type()?.is_file() {
                if let Some(clip_id) = ClipId::from_path_ref(&entry.path()) {
                    if self.clip(&clip_id).is_err() {
                        let clip = Arc::new(
                        RwLock::new(
                            WavClip::from_file(&entry.path())?
                            )
                        );
                        self.clips.insert(clip_id, clip);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    pub fn record_new_clip(&mut self) -> Result<(), Error> {
        if self.is_recording() {
            return Err(Error::AlreadyRecording());
        }
        if !self.is_configured() {
            return Err(Error::NoAudioConfiguration());
        }

        let cfg = self.audioconfig.as_ref().unwrap();

        let clip_id = ClipId::from_datetimelocal(Local::now());

        if self.clip(&clip_id).is_ok() {
            return Err(Error::ClipAlreadyExists(clip_id))
        }

        let spec = WavSpec {
            channels: 1,
            sample_rate: cfg.config.sample_rate.0,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int
        };
        let clip = Arc::new(
            RwLock::new(
                WavClip::record_new(
                    clip_id.clone(), 
                    self.path.as_path(), 
                    spec)?
                )
            );

        self.clips.insert(clip_id, clip.clone());

        // SampleRecorder starts as soon as it is created
        self.recorder = Some(SampleRecorder::new(cfg, clip)?);

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<(), Error> {
        if let Some(recorder) = self.recorder.take() {
            recorder.close()?;
        }
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), Error> {
        /*if self.is_started() {
            return Err(Error::AlreadyRecording())
        }

        // Build filename for new clip
        let clip = self.record()?;

        if !self.is_configured() {
            return Err(Error::NoAudioConfiguration())
        }
        let cfg = self.audioconfig.as_ref().unwrap();

        let mut last_ts: Option<InputStreamTimestamp> = None;
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

    pub fn clip(&self, clip_id: &ClipId) -> Result<Clip, Error> {
        if let Some(clip) = self.clips.get(clip_id) {
            Ok(clip.clone())
        } else {
            Err(Error::NoSuchClip(clip_id.clone()))
        }
    }
}