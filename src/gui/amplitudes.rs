use std::{ops::Range, sync::Arc};
use cpal::SampleRate;
use egui::{load::SizedTexture, Color32, ColorImage, DragValue, Image, TextureOptions};
use log::debug;
use parking_lot::RwLock;

pub struct Amplitudes {
    /// The desired screen height of the amplitude control
    height: usize,
    /// The desired horizontal scale (amplitudes:pixel, so a scale of 5 means 5:1)
    scale: f32,
    /// Arc RwLock pointer to the amplitudes from live or prerecorded data
    source: Arc<RwLock<Vec<f32>>>,
    /// The "start" offset (amplitude space, not screen space)
    offset: usize,
    /// The sample rate
    sample_rate: SampleRate,
    /// Keep up with live
    live: bool,
}

impl Amplitudes {    
    pub fn new(source: Arc<RwLock<Vec<f32>>>, sample_rate: SampleRate) -> Self {
        Self {
            source,
            offset: 0,
            height: 256,
            scale: 1024.0,
            sample_rate,
            live: true,
        }
    }

    /// Screen Space to Amplitude Space (scale only, do not apply offset)
    fn ss_to_as_scale(&self, n: usize) -> usize {
        (n as f32 * self.scale) as usize
    }

    /// Screen Space to Amplitude Space
    fn ss_to_as(&self, n: usize) -> usize {
        self.ss_to_as_scale(n) + self.offset
    }

    /// Get the range of amplitudes contained by a single pixel (this is always >=1 and usually >1)
    fn ss_to_as_range(&self, n: usize, clamp: usize) -> Range<usize> {
        self.ss_to_as(n)..self.ss_to_as(n+1).clamp(0, clamp)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Get the current screen real estate that we have to work with
        let width = ui.available_size().x;
        let height = self.height as f32;

        // I am assuming that egui will scale this properly but it may need to be revisited after
        // experimentation. Look into ui.pixels_per_point() if necessary.

        // The amplitude image is drawn horizontally.
        // The most recent amplitude is on the right.
        // Zero is in the center. Lines drawn at +-128
        let mut amplitude_image = std::vec::from_elem(
            Color32::from_gray(0),
            (width * height) as usize
        );

        // Acquire read lock on amplitudes
        let amplitudes = self.source.read();

        // If live, move with the live data
        if self.live {
            self.offset = amplitudes.len() - self.ss_to_as_scale(width as usize).clamp(0, amplitudes.len());
            debug!("Offset: {}", self.offset);
        }

        // We will loop over the width of the timeline control
        // The relative positions within the raw amplitudes can be derived from those indexes

        for i in 0..(width as usize) {
            if amplitudes.len() == 0 {
                break;
            }

            let amp_range = self.ss_to_as_range(i, amplitudes.len());

            // amp_range will be empty if the beginning is beyond the end of the amplitude Vec
            if amp_range.is_empty() {
                break;
            }

            let bucket = &amplitudes[amp_range];

            // Take the maximum value over the amplitudes in this bucket
            let (f32max, f32min) = bucket.iter().fold((0f32, 0f32),
                |acc, x| (acc.0.max(*x), acc.1.min(*x))
            );

            let halfheight= height/2f32;

            let displaymax = (f32max * halfheight + halfheight) as usize;
            let displaymin = (f32min * halfheight + halfheight) as usize;
            amplitude_image[displaymax * width as usize + i] = Color32::from_rgb(0, 255, 0);
            amplitude_image[displaymin * width as usize + i] = Color32::from_rgb(255, 0, 0);
        }

        drop(amplitudes);

        let amplitude_texture = ui.ctx().load_texture(
            "amplitudes",
            ColorImage::new([width as usize, height as usize], amplitude_image),
            TextureOptions::NEAREST,
        );

        let size = amplitude_texture.size_vec2();
        let sized_texture = SizedTexture::new(&amplitude_texture, size);

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.live, "Live")
                .on_hover_text("If checked, the timeline will auto-scroll to keep up with live data.");
            ui.add(DragValue::new(&mut self.scale)
                .range(1.0f32..=44100.0f32)
                .prefix("Scale: ")
            ).on_hover_text("Scales the timeline view to N samples per 1 pixel.");
        });
        ui.add(Image::new(sized_texture));
    }
}