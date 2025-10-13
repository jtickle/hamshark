use std::{fmt::Display, fs::File, io::BufWriter, path::{Path, PathBuf}, sync::Arc};
use chrono::{DateTime, Local};
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use parking_lot::RwLock;
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
        path.file_stem().map(|os| {
            os.to_str().map(|str| {
                Self(str.to_string())
            })
        }).flatten()
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


pub struct WavClip {
    pub(crate) id: ClipId,
    pub(crate) path: PathBuf,
    pub samples: Samples,
    pub(crate) writer: Option<WavWriter<BufWriter<File>>>,
}

impl WavClip {
    pub fn record_new(id: ClipId, base: &Path, spec: WavSpec) -> Result<Self, Error> {
        let path = id.absolute_path_wav(base);
        debug!("Recording new clip at {:?}", path);
        let writer = WavWriter::create(path.as_path(), spec)?;

        Ok(Self {
            id,
            path,
            samples: Default::default(),
            writer: Some(writer),
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
                    writer: None,
                };

                let mut reader = WavReader::open(path)?;
                for sample in reader.samples::<i16>() {
                    clip.samples.push(Self::i16_to_f32(sample?));
                }
                drop(reader);

                Ok(clip)
            },
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
            },
            None => Err(Error::ReadOnly(self.id.clone())),
        }
    }
}

pub type Clip = Arc<RwLock<WavClip>>;