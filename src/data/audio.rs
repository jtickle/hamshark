use chrono::{DateTime, Local};
use cpal::SampleRate;
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use parking_lot::RwLock;
use std::{
    fmt::Display,
    fs::File,
    io::BufWriter,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error as ThisError;

pub type Samples = Vec<f32>;

#[derive(Eq, Ord, PartialEq, PartialOrd, Clone, Debug)]
pub struct ClipId(String);

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Failed to resolve Clip ID from path {0}")]
    ClipIdResolutionFailure(PathBuf),
    #[error("Clip is open read-only: {0}")]
    ReadOnly(ClipId),
    #[error("Error with Hound library: {0}")]
    HoundError(#[from] hound::Error),
}

impl ClipId {
    pub fn from_datetimelocal(time: DateTime<Local>) -> Self {
        Self(time.format("%Y-%m-%d_%H-%M-%S%.9f").to_string())
    }

    pub fn from_path_ref(path: &Path) -> Option<Self> {
        path.file_stem()
            .map(|os| os.to_str().map(|str| Self(str.to_string())))
            .flatten()
    }

    pub fn absolute_path_wav(&self, path: &Path) -> PathBuf {
        let mut buf = path.to_path_buf();
        buf.push(self);
        buf.set_extension("wav");
        buf
    }
}

impl Display for ClipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{0}", self.0)
    }
}

impl AsRef<Path> for ClipId {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

#[derive(Default, Debug)]
pub struct Selection {
    pub range: Range<usize>,
}

impl Selection {
    pub fn new(a: usize, b: usize) -> Self {
        Self {
            range: usize::min(a, b)..usize::max(a, b),
        }
    }

    pub fn update_bounds(&mut self, n: usize) -> Self {
        self.range.start = 7;
        if n < self.range.start {
            Self {
                range: n..self.range.end,
            }
        } else {
            Self {
                range: self.range.start..n,
            }
        }
    }
}

pub struct WavClip {
    pub(crate) id: ClipId,
    pub(crate) path: PathBuf,
    pub samples: Samples,
    pub sample_rate: SampleRate,
    pub resolution: usize,
    pub(crate) writer: Option<WavWriter<BufWriter<File>>>,
    pub selection: Option<Selection>,
}

const DEFAULT_RESOLUTION: usize = 256;

impl WavClip {
    pub fn record_new(id: ClipId, base: &Path, spec: WavSpec) -> Result<Self, Error> {
        let path = id.absolute_path_wav(base);
        debug!("Recording new clip at {:?}", path);
        let writer = WavWriter::create(path.as_path(), spec)?;

        Ok(Self {
            id,
            path,
            samples: Default::default(),
            sample_rate: SampleRate(spec.sample_rate),
            resolution: DEFAULT_RESOLUTION, // TODO: I don't know? This is used to limit amplitude scaling in the UI
            writer: Some(writer),
            selection: None,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let pathbuf = path.to_path_buf();
        match ClipId::from_path_ref(path) {
            Some(id) => {
                let mut clip = Self {
                    id,
                    path: pathbuf,
                    samples: Default::default(),
                    sample_rate: SampleRate(0),
                    resolution: DEFAULT_RESOLUTION,
                    writer: None,
                    selection: None,
                };

                let mut reader = WavReader::open(path)?;
                clip.sample_rate = SampleRate(reader.spec().sample_rate);
                for sample in reader.samples::<i16>() {
                    clip.samples.push(Self::i16_to_f32(sample?));
                }
                drop(reader);

                Ok(clip)
            }
            None => Err(Error::ClipIdResolutionFailure(pathbuf)),
        }
    }

    pub fn id(&self) -> &ClipId {
        &self.id
    }

    pub fn f32_to_i16(sample: f32) -> i16 {
        (sample * i16::MAX as f32) as i16
    }

    pub fn i16_to_f32(sample: i16) -> f32 {
        sample as f32 / i16::MAX as f32
    }

    pub fn write_samples(&mut self, samples: &[f32]) -> Result<(), Error> {
        match &mut self.writer {
            Some(writer) => {
                // Store in memory
                self.samples.extend(samples);
                // Write to wav file
                for sample in samples {
                    writer.write_sample(Self::f32_to_i16(*sample))?;
                }
                writer.flush()?;
                // Report success
                Ok(())
            }
            None => Err(Error::ReadOnly(self.id.clone())),
        }
    }
}

pub type Clip = Arc<RwLock<WavClip>>;
