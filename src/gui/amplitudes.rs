use std::sync::Arc;
use parking_lot::RwLock;

pub struct Amplitudes {
    current_scale: u8,
    source_amplitudes_arc: Arc<RwLock<Vec<f32>>>,
    
}