use std::sync::Arc;

use parking_lot::RwLock;

use crate::data::audio::Samples;

#[derive(Default)]
pub struct SampleBuffer {
    samples: Samples,
    next: Option<Arc<RwLock<dyn super::Sink>>>,
}

impl super::Sink for SampleBuffer {
    fn process(&mut self, data: &[f32]) -> Result<(), super::Error> {
        self.samples.extend(data);

        self.next.clone().unwrap().write().process(data)
    }
    
    fn cleanup(self) -> Result<(), super::Error> {
        todo!()
    }
}

impl super::Source for SampleBuffer {
    fn next_element(&self) -> Option<Arc<RwLock<dyn super::Sink>>> {
        self.next.clone()
    }

    fn set_next_element(&mut self, element: Arc<RwLock<dyn super::Sink>>) {
        self.next = Some(element);
    }
}