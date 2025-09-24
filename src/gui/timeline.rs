use std::{ops::Range, sync::Arc};
use cpal::SampleRate;
use egui::{load::SizedTexture, Color32, ColorImage, DragValue, Image, Sense, TextureOptions};
use log::debug;
use parking_lot::RwLock;

pub struct Timeline {
    /// The desired screen height of the timeline control
    height: usize,
    /// The desired horizontal scale (samples:pixel, so a scale of 5 means 5:1)
    scale: f32,
    /// Arc RwLock pointer to the samples from live or prerecorded data
    samples: Arc<RwLock<Vec<f32>>>,
    /// The "start" offset in sample space
    offset: usize,
    /// The sample rate
    sample_rate: SampleRate,
    /// Keep up with live
    live: bool,
}

impl Timeline {    
    pub fn new(source: Arc<RwLock<Vec<f32>>>, sample_rate: SampleRate) -> Self {
        Self {
            samples: source,
            offset: 0,
            height: 256,
            scale: 1024.0,
            sample_rate,
            live: true,
        }
    }

    /// Screen Space to Sample Space (scale only, do not apply offset)
    fn screen_to_sample_scale(&self, n: usize) -> usize {
        (n as f32 * self.scale) as usize
    }

    /// Screen Space to Sample Space
    fn screen_to_sample(&self, n: usize) -> usize {
        self.screen_to_sample_scale(n) + self.offset
    }

    /// Get the range of samples contained by a single pixel (this is always >=1 and usually >1)
    fn screen_to_sample_range(&self, n: usize, clamp: usize) -> Range<usize> {
        self.screen_to_sample(n)..self.screen_to_sample(n+1).clamp(0, clamp)
    }

    fn screen_to_image_idx(&self, width: usize, x: usize, y: usize) -> usize {
        ((y * width) + x) as usize
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Show the timeline controls
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.live, "Live")
                .on_hover_text("If checked, the timeline will auto-scroll to keep up with live data.");
            ui.add(DragValue::new(&mut self.scale)
                .range(1.0f32..=44100.0f32)
                .prefix("Scale: ")
            ).on_hover_text("Scales the timeline view to N samples per 1 pixel.");
        });

        // Get the current screen real estate that we have to work with
        let width = ui.available_size().x.floor() as usize;
        let height = self.height;

        // I am assuming that egui will scale this properly but it may need to be revisited after
        // experimentation. Look into ui.pixels_per_point() if necessary.

        // The amplitude image is drawn horizontally.
        // The most recent sample is on the right.
        // Zero is in the center. Lines drawn at +-128
        let mut amplitude_image = std::vec::from_elem(
            Color32::from_gray(0),
            width * height
        );

        // Acquire read lock on amplitudes
        let samples = self.samples.read();

        // If live, move with the live data
        if self.live {
            self.offset = samples.len() - self.screen_to_sample_scale(width as usize).clamp(0, samples.len());
        }

        // Loop over the width of the timeline control
        // The relative positions within the sample vector can be derived from those indexes
        for i in 0..(width as usize) {
            if samples.len() == 0 {
                break;
            }

            // Derive sample range for the current pixel
            let sample_range = self.screen_to_sample_range(i, samples.len());

            // amp_range will be empty if the beginning is beyond the end of the amplitude Vec
            if sample_range.is_empty() {
                break;
            }

            let bucket = &samples[sample_range];

            // Take the maximum and minimum values over the samples in this bucket
            let (f32max, f32min) = bucket.iter().fold((0f32, 0f32),
                |acc, x| (acc.0.max(*x), acc.1.min(*x))
            );

            let halfheight = (height/2) as f32;

            let displaymax = (f32max * halfheight + halfheight).floor() as usize;
            let displaymin = (f32min * halfheight + halfheight).floor() as usize;
            amplitude_image[self.screen_to_image_idx(width, i, displaymax)] = Color32::from_rgb(0, 255, 0);
            amplitude_image[self.screen_to_image_idx(width, i, displaymin)] = Color32::from_rgb(255, 0, 0);
        }

        // Draw a vertical line for the current pointer position, if any
        if let Some(pointer_pos) = ui.input(|i| i.pointer.latest_pos()) {
            let mut bounds = ui.cursor();
            bounds.max.y = bounds.min.y + height as f32;
            bounds.max.x = bounds.max.x - 1f32;
            if bounds.contains(pointer_pos) {
                for i in 0..(height as usize) {
                    let idx = self.screen_to_image_idx(width, (pointer_pos.x - bounds.min.x).floor() as usize, i);
                    amplitude_image[idx] = Color32::from_rgb(0, 0, 255);
                }
            }
        }

        drop(samples);

        let amplitude_texture = ui.ctx().load_texture(
            "samples",
            ColorImage::new([width as usize, height as usize], amplitude_image),
            TextureOptions::NEAREST,
        );

        // Show the timeline
        let size = amplitude_texture.size_vec2();
        let sized_texture = SizedTexture::new(&amplitude_texture, size);
        let image = Image::new(sized_texture)
            .sense(Sense::click_and_drag());
        let response = ui.add(image);
        if response.clicked() {
            debug!("Clicked!!");
        }
        if response.dragged() {
            self.live = false;
            debug!("offset {} delta {} scaled {}", self.offset, response.drag_delta().x, self.screen_to_sample_scale(response.drag_delta().x as usize));
            self.offset = self.offset.checked_sub(
                self.screen_to_sample_scale(response.drag_delta().x as usize)
            ).unwrap_or_default();
        }
    }
}