use std::{ops::Range, sync::Arc};
use cpal::SampleRate;
use egui::{load::SizedTexture, Color32, ColorImage, DragValue, Image, Pos2, Response, Sense, TextureOptions, Vec2};
use log::debug;
use parking_lot::RwLock;
use rustfft::num_complex::Complex;

pub struct Timeline {
    /// The desired screen height of the timeline control
    height: usize,
    /// The desired horizontal scale (samples:pixel, so a scale of 5 means 5:1)
    scale: f32,
    /// How many samples per FFT
    samples_per_fft: usize,
    /// Arc RwLock pointer to the samples from live or prerecorded data
    samples: Arc<RwLock<Vec<f32>>>,
    /// Arc RwLock pointer to the fft data
    fft: Arc<RwLock<Vec<Vec<Complex<f32>>>>>,
    /// The "start" offset in screen space
    offset: usize,
    /// The sample rate
    sample_rate: SampleRate,
    /// Keep up with live
    live: bool,
}

impl Timeline {    
    pub fn new(samples: Arc<RwLock<Vec<f32>>>, fft: Arc<RwLock<Vec<Vec<Complex<f32>>>>>, sample_rate: SampleRate) -> Self {
        Self {
            samples,
            fft,
            offset: 0,
            samples_per_fft: 128,
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

    /// Sample Space to Screen Space
    fn sample_to_screen(&self, n: usize) -> usize {
        (n as f32 / self.scale).floor() as usize
    }

    /// Screen Space to Sample Space
    fn screen_to_sample(&self, n: usize) -> usize {
        self.screen_to_sample_scale(n) + (self.offset * self.scale.floor() as usize)
    }

    /// Get the range of samples contained by a single pixel (this is always >=1 and usually >1)
    fn screen_to_sample_range(&self, n: usize, clamp: usize) -> Range<usize> {
        self.screen_to_sample(n)..self.screen_to_sample(n+1).clamp(0, clamp)
    }

    // Screen Space to FFT Space (scale only, do not apply offset)
    fn screen_to_fft_scale(&self, n: usize) -> usize {
        (n as f32 * self.scale / self.samples_per_fft as f32).floor() as usize
    }

    /// Screen Space to FFT Space
    fn screen_to_fft(&self, n: usize) -> usize {
        self.screen_to_fft_scale(n) + (self.offset as f32 * self.scale / self.samples_per_fft as f32).floor() as usize
    }

    /// Get the range of fft data contained by a single pixel (this is always >=1 and usually >1)
    /// note that unlike samples, many pixels can point to the same fft data
    fn screen_to_fft_range(&self, n: usize, clamp: usize) -> Range<usize> {
        self.screen_to_fft(n)..self.screen_to_fft(n+1).clamp(1, clamp)
    }

    /// Translate screen coordinates to vector position
    fn screen_to_image_idx(&self, width: usize, x: usize, y: usize) -> usize {
        ((y * width) + x) as usize
    }

    /// Translate polar coordinates to vector position for IQ diagram
    fn polar_to_iq_idx(&self, magnitude: f32, phase: f32) -> usize {
        let x = ((1.0 + (phase.cos() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        let y = ((1.0 - (phase.sin() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        //debug!("{} {} {} {}", phase.cos(), 1.0 + phase.cos(), (1.0 + phase.cos()) * magnitude, (1.0 + phase.cos()) * magnitude * self.samples_per_fft as f32);
        (y.clamp(0, self.samples_per_fft * 2 - 1) * self.samples_per_fft * 2) + x.clamp(0, self.samples_per_fft * 2 - 1)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Get the current screen real estate that we have to work with
        let width = ui.available_size().x.floor() as usize;
        let height = self.height;

        // Show the timeline controls
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.live, "Live")
                .on_hover_text("If checked, the timeline will auto-scroll to keep up with live data.");

            let sampleoffset = self.screen_to_sample_scale(self.offset + width/2);
            let prevscale = self.scale;

            ui.add(DragValue::new(&mut self.scale)
                .range(1.0f32..=44100.0f32)
                .prefix("Scale: ")
            ).on_hover_text("Scales the timeline view to N samples per 1 pixel.");

            if !self.live && prevscale != self.scale {
                self.offset = self.sample_to_screen(sampleoffset) - width/2;
            }
        });

        // I am assuming that egui will scale this properly but it may need to be revisited after
        // experimentation. Look into ui.pixels_per_point() if necessary.

        // The amplitude image is drawn horizontally.
        // The most recent sample is on the right.
        // Zero is in the center. Lines drawn at +-128
        let mut amplitude_image = std::vec::from_elem(
            Color32::from_gray(0),
            width * height
        );

        // The waterfall image is drawn horizontally (yes unusual but bear with me)
        // The most recent sample is on the right.
        // The fundamental is at the top.
        let mut waterfall_image = std::vec::from_elem(
            Color32::from_gray(0),
            width * self.samples_per_fft
        );

        // Temporary IQ image... curious what we get but this will have to be a separate control
        // and somehow linked into a selection range.
        let mut iq_image = std::vec::from_elem(
            Color32::from_gray(0),
            self.samples_per_fft * self.samples_per_fft * 4
        );

        // Acquire read lock on samples
        let samples = self.samples.read();

        // Acquire read lock on fft
        let fft = self.fft.read();

        // If live, move with the live data
        if self.live {
            let samplen_scaled = self.sample_to_screen(samples.len());
            self.offset = if samplen_scaled > width {
                samplen_scaled - width
            } else {
                0
            }
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

            // Now build the FFT image
            // There are always more samples than FFT data so if we get this far we are good
            let fft_range = self.screen_to_fft_range(i, fft.len());
            if fft_range.is_empty() {
                // TODO: if there is more fft data and the scale is too fine, just include the
                // next datapoint so we smear the FFT display over multiple pixels
                continue;
            }

            let fft_bucket = &fft[fft_range];

            for j in 0..self.samples_per_fft {
                // we have to shift by samples_per_fft/2 to put the fundamental in the center
                let fft_shift = (j + (self.samples_per_fft / 2)) % self.samples_per_fft;
                let (magnitude, _phase) = fft_bucket.into_iter().fold((0f32, 0f32), |acc, k| {
                    let polar = k[fft_shift].to_polar();
                    // We plot every magnitude and phase on the IQ diagram. This might hurt a bit
                    // but I really don't want to implement selection ranges this morning.
                    iq_image[self.polar_to_iq_idx(polar.0, polar.1)] = Color32::from_gray(255);
                    (acc.0.max(polar.0), acc.1.max(polar.1))
                });
                // let's ignore phase for now that ought to only matter at IQ time
                /*waterfall_image[self.screen_to_image_idx(width, i, j)] = 
                    Color32::from_rgb(
                        (phase.sin() * magnitude * 255f32).floor() as u8,
                        0,
                        (phase.cos() * magnitude * 255f32).floor() as u8,
                    );*/
                waterfall_image[self.screen_to_image_idx(width, i, j)] =
                    Color32::from_gray((magnitude * 255f32).floor() as u8);
            }
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

        drop(fft);
        drop(samples);

        let amplitude_texture = ui.ctx().load_texture(
            "samples",
            ColorImage::new([width as usize, height as usize], amplitude_image),
            TextureOptions::NEAREST,
        );

        let waterfall_texture = ui.ctx().load_texture(
            "waterfall",
            ColorImage::new([width as usize, self.samples_per_fft], waterfall_image),
            TextureOptions::NEAREST,
        );

        let iq_texture = ui.ctx().load_texture(
            "iq",
            ColorImage::new([self.samples_per_fft * 2, self.samples_per_fft * 2], iq_image),
            TextureOptions::NEAREST,
        );

        let mut drag_action = |delta: Vec2| {
            self.live = false;
            let val = delta.x;
            let mag = val.abs().floor() as usize;
            debug!("offset {} mag {}", self.offset, mag);
            if val < 0.0 {
                self.offset += mag;
            } else if mag > self.offset {
                self.offset = 0;
            } else {
                self.offset -= mag;
            }
            debug!("new offset {}", self.offset);
        };

        // Show the timeline
        let samples_size = amplitude_texture.size_vec2();
        let samples_sized_texture = SizedTexture::new(&amplitude_texture, samples_size);
        let samples_image_widget = Image::new(samples_sized_texture)
            .sense(Sense::click_and_drag());
        let samples_response = ui.add(samples_image_widget);
        if samples_response.clicked() {
            debug!("Samples Clicked!!");
        }
        if samples_response.dragged() {
            drag_action(samples_response.drag_delta());
        }

        // Show the waterfall
        let waterfall_size = waterfall_texture.size_vec2();
        let waterfall_sized_texture = SizedTexture::new(&waterfall_texture, waterfall_size);
        let waterfall_image_widget = Image::new(waterfall_sized_texture)
            .sense(Sense::click_and_drag());
        let waterfall_response = ui.add(waterfall_image_widget);
        if waterfall_response.is_pointer_button_down_on() {
            drag_action(waterfall_response.drag_delta());
        }

        // Show the IQ diagram
        let iq_size = iq_texture.size_vec2();
        let iq_sized_texture = SizedTexture::new(&iq_texture, iq_size);
        let iq_image_widget = Image::new(iq_sized_texture);
        let iq_response = ui.add(iq_image_widget);
    }
}