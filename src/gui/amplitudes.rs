use std::sync::Arc;
use cpal::SampleRate;
use egui::{load::SizedTexture, Color32, ColorImage, Image, TextureOptions};
use log::debug;
use parking_lot::RwLock;

pub struct Amplitudes {
    /// The desired screen height of the amplitude control
    height: usize,
    /// The desired horizontal scale (amplitudes:pixel, so a scale of 5 means 5:1)
    scale: f32,
    /// Arc RwLock pointer to the amplitudes from live or prerecorded data
    source: Arc<RwLock<Vec<f32>>>,
    /// The "start" offset within the source amplitudes (unscaled)
    offset: usize,
    /// The sample rate
    sample_rate: SampleRate,
}

impl Amplitudes {    
    pub fn new(source: Arc<RwLock<Vec<f32>>>, sample_rate: SampleRate) -> Self {
        Self {
            source,
            offset: 0,
            height: 256,
            scale: 1024.0,
            sample_rate
        }
    }

    fn scale_by(&self, n: usize) -> usize {
        ((n as f32) * self.scale) as usize
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // In here we will work in usize for integers and f32 for floats.
        // Do math in f32 where possible and then convert to usize.

        // Get the current screen real estate that we have to work with
        let width = ui.available_size().x as f32;
        let height = self.height as f32;

        // I am assuming that egui will scale this properly but it may need to be revisited after
        // experimentation. Look into ui.pixels_per_point() if necessary.

        // The amplitude image is drawn horizontally.
        // The most recent amplitude is on the right.
        // Zero is in the center. Lines drawn at +-128
        let mut amplitude_image = std::vec::from_elem(
            Color32::from_gray(0),
            width as usize * height as usize
        );

        // Acquire read lock on amplitudes
        let amplitudes = self.source.read();

        // self.offset is our begin point for rendering.
        // Determine end point based on width and scale.
        // We're looping over pixels. Aggregation will happen within each pixel.
        let begin = self.offset;
        let end = self.offset + width as usize;
        let f32scale = self.scale as f32 / 2f32;

        // temporary
        let mut maxval = 0.0f32;
        let mut minval = 0.0f32;

        for i in begin..end-1 {
            if amplitudes.len() == 0 {
                break;
            }

            let scaled_start = self.scale_by(i);
            let scaled_end = self.scale_by(i+1);

            if scaled_end > amplitudes.len() {
                break;
            }

            let bucket = &amplitudes[scaled_start .. scaled_end];

            // Take the maximum value over the amplitudes in this bucket
            let (f32max, f32min) = bucket.iter().fold((0f32, 0f32),
                |acc, x| (acc.0.max(*x), acc.1.min(*x))
            );

            // Store the max and min values seen
            maxval = maxval.max(f32max);
            minval = minval.min(f32min);

            let halfheight= height/2f32;

            let displaymax = (f32max * halfheight + halfheight) as usize;
            let displaymin = (f32min * halfheight + halfheight) as usize;
            amplitude_image[displaymax * width as usize + i] = Color32::from_rgb(0, 255, 0);
            amplitude_image[displaymin * width as usize + i] = Color32::from_rgb(255, 0, 0);
        }

        let amplitude_texture = ui.ctx().load_texture(
            "amplitudes",
            ColorImage::new([width as usize, height as usize], amplitude_image),
            TextureOptions::NEAREST,
        );

        let size = amplitude_texture.size_vec2();
        let sized_texture = SizedTexture::new(&amplitude_texture, size);

        ui.label(format!("Scale: {} Max: {} Min: {}", self.scale, maxval, minval));
        ui.add(Image::new(sized_texture));
    }
}