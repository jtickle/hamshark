use std::{fs::File, io::BufWriter, path::PathBuf};

use hound::{WavSpec, WavWriter};
use log::info;

use crate::pipeline::{Element, Error, Sink};

pub struct WavSink {
    writer: WavWriter<BufWriter<File>>,
}

impl WavSink {
    pub fn new(path: PathBuf, spec: WavSpec) -> Result<Self, Error> {
        // TODO: there may be a reason to have more control over the BufWriter,
        // ie it doesn't seem to write shit until close even with a flush
        match WavWriter::create(path, spec) {
            Ok(writer) => Ok(WavSink { writer }),
            Err(err) => Err(Error::platform(&err)),
        }
    }
}

impl Sink for WavSink {
    fn process(&mut self, data: &[f32]) -> Result<(), super::Error> {
        info!("Writing {} samples", data.len());
        for sample_ref in data {
            let sample = *sample_ref;
            if let Err(err) = self.writer.write_sample((sample * i16::MAX as f32) as i16) {
                return Err(super::Error::platform(&err));
            }
        }
        if let Err(err) = self.writer.flush() {
            return Err(super::Error::platform(&err));
        }
        Ok(())
    }
    
    fn cleanup(self) -> Result<(), Error> {
        if let Err(err) = self.writer.finalize() {
            return Err(super::Error::platform(&err));
        }
        Ok(())
    }
}