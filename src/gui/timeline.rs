use std::ops::Range;
use cpal::SampleRate;
use egui::{load::SizedTexture, Color32, ColorImage, DragValue, Image, PointerButton, Pos2, Response, Sense, TextureOptions, Vec2};
use log::debug;
use rustfft::num_complex::Complex;
use crate::{data::audio::Clip, session::Frequencies};

#[derive(PartialEq)]
enum DragState {
    DownButNotDragging(Pos2),
    Dragging,
    NotDragging,
}

pub struct Timeline {
    /// The desired screen height of the timeline control
    height: usize,
    /// The desired horizontal scale (samples:pixel, so a scale of 5 means 5:1)
    scale: f32,
    /// The desired vertical scale
    vscale: f32,
    /// How many samples per FFT
    samples_per_fft: usize,
    /// The clip we're browsing
    clip: Clip,
    /// The "start" offset in screen space
    offset: usize,
    /// The sample rate
    sample_rate: SampleRate,
    /// Keep up with live
    live: bool,
    /// Selection Markers
    marker_begin: Option<usize>,
    marker_end: Option<usize>,
    /// Make drag operations more precise
    drag_state: DragState,
}

impl Timeline {
    pub fn new(clip: Clip) -> Self {
        let sample_rate = clip.read().sample_rate;

        Self {
            clip,
            offset: 0,
            samples_per_fft: 128,
            height: 256,
            scale: 1024.0,
            vscale: 1.0,
            sample_rate,
            live: true,
            marker_begin: None,
            marker_end: None,
            drag_state: DragState::NotDragging,
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
        self.screen_to_fft_scale(n) + self.screen_to_fft_scale(self.offset)
    }

    /// Get the range of fft data contained by a single pixel (this is always >=1 and usually >1)
    /// note that unlike samples, many pixels can point to the same fft data
    fn screen_to_fft_range(&self, n: usize, clamp: usize) -> Range<usize> {
        self.screen_to_fft(n)..self.screen_to_fft(n+1).clamp(1, clamp)
    }

    /// Translate screen coordinates to vector position
    fn screen_to_image_idx(&self, width: usize, x: usize, y: usize) -> usize {
        ((y.clamp(0, self.height - 1) * width) + x) as usize
    }

    /// Translate polar coordinates to vector position for IQ diagram
    fn polar_to_iq_idx(&self, magnitude: f32, phase: f32) -> usize {
        let x = ((1.0 + (phase.cos() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        let y = ((1.0 - (phase.sin() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        //debug!("{} {} {} {}", phase.cos(), 1.0 + phase.cos(), (1.0 + phase.cos()) * magnitude, (1.0 + phase.cos()) * magnitude * self.samples_per_fft as f32);
        (y.clamp(0, self.samples_per_fft * 2 - 1) * self.samples_per_fft * 2) + x.clamp(0, self.samples_per_fft * 2 - 1)
    }

    /// Translate a sample to a screen coordinate
    fn sample_to_y_coordinate(&self, sample: f32) -> usize {
        let halfheight = self.height as f32 / 2f32;
        (self.vscale * sample * halfheight + halfheight) as usize
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

            ui.add(DragValue::new(&mut self.vscale)
                .range(1.0f32..=200.0f32)
                .prefix("VScale: ")
            ).on_hover_text("Scales the timeline amplitude");

            if !self.live && prevscale != self.scale {
                let halfwidth = width / 2;
                let uncentered_offset = self.sample_to_screen(sampleoffset);
                self.offset = if halfwidth > uncentered_offset {
                    0
                } else {
                    uncentered_offset - halfwidth
                }
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
        let samples = &self.clip.read().samples;

        // Acquire read lock on fft
        //let fft_derp: Frequencies = Default::default(); //self.fft.read();
        //let fft = fft_derp.read();

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

            // If the range only contains one sample, just draw one sample. This means scaling factor is 1.
            if sample_range.len() == 1 {
                let y = self.sample_to_y_coordinate(samples[sample_range.min().unwrap()]);
                let color = if y == 0 || y > height - 1 {
                    Color32::from_rgb(255, 0, 0)
                } else {
                    Color32::from_rgb(127, 127, 255)
                };
                amplitude_image[self.screen_to_image_idx(width, i, y)] = color;
                continue;
            }

            // Otherwise we draw a range
            let bucket = &samples[sample_range];

            // Take the maximum and minimum values over the samples in this bucket
            let (f32max, f32min) = bucket.iter().fold((f32::MIN, f32::MAX),
                |acc, x| (acc.0.max(*x), acc.1.min(*x))
            );

            let displaymax = self.sample_to_y_coordinate(f32max);
            let displaymin = self.sample_to_y_coordinate(f32min);

            for y in displaymin..displaymax {
                let color = if y == 0 || y > height - 1 {
                    Color32::from_rgb(255, 0, 0)
                } else {
                    Color32::from_rgb(127, 127, 255)
                };
                amplitude_image[self.screen_to_image_idx(width, i, y)] = color
            }

            // Now build the FFT image
            // There are always more samples than FFT data so if we get this far we are good
            /*let fft_range = self.screen_to_fft_range(i + self.sample_to_screen(self.offset % self.samples_per_fft), fft.len());
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
                    let iq_idx = self.polar_to_iq_idx(polar.0, polar.1);
                    let prev_color = iq_image[iq_idx];
                    iq_image[iq_idx] = Color32::from_gray(prev_color.r().max(255 - (self.sample_to_screen(samples.len()).min(width) - i).clamp(0, 255) as u8));
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
            }*/
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

        //drop(fft);

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

        let mut pan_action = |delta: Vec2| {
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
            .sense(Sense::click_and_drag() | Sense::hover());
        let samples_response = ui.add(samples_image_widget);

        // In egui, the "drag" deltas start reporting after the mouse has moved, and so if you click
        // precisely where you mean to begin the drag, it will not begin where you expected.
        // Submitting a patch to egui is probably the better solution here...
        if samples_response.is_pointer_button_down_on() {
            if self.drag_state == DragState::NotDragging
            && let Some(pos) = ui.input(|input| input.pointer.interact_pos()) {
                self.drag_state = DragState::DownButNotDragging(pos);
            } 
        } else {
            self.drag_state = DragState::NotDragging;
        }

        let mut get_delta = |response: &Response| -> Vec2 {
            match self.drag_state {
                DragState::DownButNotDragging(pos) => {
                    if let Some(cur) = ui.input(|input| input.pointer.interact_pos()) {
                        self.drag_state = DragState::Dragging;
                        cur - pos
                    } else {
                        panic!("In dragging state but no current mouse position")
                    }
                },
                DragState::Dragging => response.drag_delta(),
                DragState::NotDragging => panic!("Should not be able to get to this state"),
            }
        };

        // Only one of these can apply
        if samples_response.dragged_by(PointerButton::Primary) {
            if self.marker_begin.is_none() && let DragState::DownButNotDragging(begin) = self.drag_state {
                self.marker_begin = Some(self.screen_to_sample(begin.x as usize));
                println!("Start set to {:?}", self.marker_begin);
            }
        }
        else if samples_response.dragged_by(PointerButton::Secondary) {
            pan_action(get_delta(&samples_response));
        }

        // This can always apply
        if samples_response.hovered() {
            self.scale *= ui.input(|input| input.zoom_delta());
        }

        // Show the waterfall
        /*let waterfall_size = waterfall_texture.size_vec2();
        let waterfall_sized_texture = SizedTexture::new(&waterfall_texture, waterfall_size);
        let waterfall_image_widget = Image::new(waterfall_sized_texture)
            .sense(Sense::click_and_drag());
        let waterfall_response = ui.add(waterfall_image_widget);
        if waterfall_response.is_pointer_button_down_on() {
            drag_action(waterfall_response.drag_delta());
        }*/

        // Show the IQ diagram
        let iq_size = iq_texture.size_vec2();
        let iq_sized_texture = SizedTexture::new(&iq_texture, iq_size);
        let iq_image_widget = Image::new(iq_sized_texture);
        let iq_response = ui.add(iq_image_widget);
    }
}