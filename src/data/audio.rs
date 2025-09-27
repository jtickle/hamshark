use std::{fs::File, io::BufWriter, path::{Path, PathBuf}, sync::Arc};

use hound::{Error, WavReader, WavSpec, WavWriter};
use parking_lot::RwLock;
use rustfft::num_complex::Complex;

pub type Samples = Arc<RwLock<Vec<f32>>>;
pub type Frequencies = Arc<RwLock<Vec<Vec<Complex<f32>>>>>;

pub enum ClipState {
    Closed,
    ReadOnly {
        spec: WavSpec,
        samples: Samples,
    },
    Writable {
        spec: WavSpec,
        samples: Samples,
        writer: WavWriter<BufWriter<File>>,
    }
}

pub struct Clip {
    pub path: PathBuf,
    pub state: ClipState,
}

impl Clip {
    pub fn new(path: &Path, spec: WavSpec) -> Result<Self, Error> {
        let writer = WavWriter::create(path, spec)?;

        let clip = Self {
            path: PathBuf::from(path),
            state: ClipState::Writable {
                spec,
                samples: Default::default(),
                writer: writer,
            },
        };

        Ok(clip)
    }

    pub fn is_writable(&self) -> bool {
        match self.state {
            ClipState::Closed => false,
            ClipState::ReadOnly { spec: _, samples: _ } => false,
            ClipState::Writable { spec: _, samples: _, writer: _ } => true,
        }
    }

    pub fn is_open(&self) -> bool {
        match self.state {
            ClipState::Closed => false,
            ClipState::ReadOnly { spec: _, samples: _ } => true,
            ClipState::Writable { spec: _, samples: _, writer: _ } => true,
        }
    }

    pub fn write_samples(&mut self, data: &[f32]) -> Result<(), Error> {
        if let ClipState::Writable {
            spec: _,
            samples: samples,
            writer: writer
        } = &mut self.state {
            for sample_ref in data {
                let sample = *sample_ref;
                samples.write().push(sample);
                writer.write_sample((sample * i16::MAX as f32) as i16)?;
            }
        } else {
            return Err(Error::IoError(std::io::Error::from(std::io::ErrorKind::AlreadyExists)));
        }
        
        Ok(())
    }

    pub fn open(&mut self) -> Result<(), Error> {
        match self.state {
            ClipState::Closed => {
                let path = self.path.as_path();
                let mut reader = WavReader::open(path)?;
                let spec = reader.spec();
                let mut err: Option<hound::Error> = None;

                let samples = reader.samples::<i16>().map(|ival| {
                    if ival.is_err() {
                        err = ival.err();
                        0f32
                    } else {
                        ival.unwrap() as f32 / (i16::MAX as f32)
                    }
                });

                let _ = std::mem::replace(self, Self {
                    path: path.to_path_buf(),
                    state: ClipState::ReadOnly {
                        spec,
                        samples: Arc::new(RwLock::new(samples.collect())),
                    },
                });
                Ok(())
            },
            ClipState::ReadOnly { spec: _, samples: _ } => Ok(()),
            ClipState::Writable { spec: _, samples: _, writer: _ } => Ok(()),
        }
    }

    pub fn close(&mut self) {
        let closed = Self {
            path: self.path.clone(),
            state: ClipState::Closed,
        };
        match self.state {
            ClipState::Closed => (),
            ClipState::ReadOnly { spec: _, samples: _ } => {
                std::mem::replace(self, closed);
            },
            ClipState::Writable { spec: _, samples: _, writer: _ } => {
                std::mem::replace(self, closed);
            },
        }
    }

    pub fn samples(&self) -> Option<Samples> {
        match &self.state {
            ClipState::Closed => None,
            ClipState::ReadOnly { spec: _, samples } => Some(samples.clone()),
            ClipState::Writable { spec: _, samples, writer: _ } => Some(samples.clone()),
        }
    }
}

impl From<&Path> for Clip {
    fn from(value: &Path) -> Self {
        Self {
            path: PathBuf::from(value),
            state: ClipState::Closed,
        }
    }
}