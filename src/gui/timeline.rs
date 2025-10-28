use crate::{
    data::audio::{Clip, Selection},
    session::Frequencies,
};
use egui::{
    Color32, ColorImage, DragValue, Image, PointerButton, Pos2, Rect, Response, Sense,
    TextureOptions, load::SizedTexture,
};
use mint::Vector2;
use rustfft::num_complex::Complex;
use std::ops::Range;

#[derive(Default, PartialEq)]
enum DragState {
    DownButNotDragging(Vector2<usize>),
    Dragging,
    #[default]
    NotDragging,
}

pub trait Scaler {
    fn screen_space(&self) -> Vector2<usize>;
    fn data_space(&self) -> Vector2<usize>;
    fn scale(&self) -> Vector2<f32>;
    fn offset(&self) -> usize;

    fn width(&self) -> usize {
        self.screen_space().x
    }

    fn height(&self) -> usize {
        self.screen_space().y
    }

    fn screen_to_data_x_without_offset(&self, x: isize) -> isize {
        (x as f32 * self.scale().x).floor() as isize
    }

    fn screen_space_offset_x(&self) -> isize {
        self.screen_to_data_x_without_offset(self.offset() as isize)
    }

    fn screen_x_coordinate_to_data_range(&self, x: usize) -> Range<usize> {
        let x = x as isize;
        (self.screen_to_data_x(x) as usize)
            ..(self
                .screen_to_data_x(x + 1)
                .clamp(0, self.data_space().x as isize) as usize)
    }

    fn data_x_range_to_screen_x_range(&self, range: &Range<usize>) -> Range<usize> {
        (self
            .data_to_screen_x(range.start as isize)
            .clamp(0, self.width() as isize) as usize)
            ..(self
                .data_to_screen_x(range.end as isize)
                .clamp(0, self.width() as isize) as usize)
    }

    fn screen_to_data_x(&self, x: isize) -> isize {
        self.screen_to_data_x_without_offset(x) + self.offset() as isize
    }

    fn screen_to_data_y(&self, y: isize) -> isize {
        (y as f32 * self.scale().y).floor() as isize
    }

    fn screen_to_data(&self, coordinate: &Vector2<usize>) -> Vector2<usize> {
        Vector2 {
            x: self.screen_to_data_x(coordinate.x as isize) as usize,
            y: self.screen_to_data_y(coordinate.y as isize) as usize,
        }
    }

    fn data_to_screen_x_without_offset(&self, x: isize) -> isize {
        (x as f32 / self.scale().x).floor() as isize
    }

    fn data_to_screen_x(&self, x: isize) -> isize {
        self.data_to_screen_x_without_offset(x - self.offset() as isize)
    }

    fn data_to_screen_y(&self, y: isize) -> isize {
        (y as f32 / self.scale().y).floor() as isize
    }

    fn data_to_screen(&self, coordinate: &Vector2<usize>) -> Vector2<usize> {
        Vector2 {
            x: self.data_to_screen_x(coordinate.x as isize) as usize,
            y: self.data_to_screen_y(coordinate.y as isize) as usize,
        }
    }

    fn input_pos(&self, bounds: &Rect, pos: Option<Pos2>) -> Option<Vector2<usize>> {
        let mut bounds = bounds.clone();
        bounds.max.x -= 1.0f32;
        bounds.max.y -= 1.0f32;
        pos.map(|pos| {
            if bounds.contains(pos) {
                Some(Vector2 {
                    x: (pos.x - bounds.min.x).floor() as usize,
                    y: (pos.y - bounds.min.y).floor() as usize,
                })
            } else {
                None
            }
        })
        .flatten()
    }

    /// Translate screen coordinates to vector position
    fn screen_to_image_idx(&self, x: usize, y: usize) -> usize {
        let screen = self.screen_space();
        ((y.clamp(0, screen.y - 1) * screen.x) + x.clamp(0, screen.x - 1)) as usize
    }
}

pub struct Timeline {
    /// The allocated screen height of the timeline control
    height: usize,
    /// The allocated screen width of the timeline control
    width: usize,
    /// The last recorded length of the sample data
    sample_len: usize,
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
    /// Keep up with live
    live: bool,
    /// Selection Markers
    selection: Option<Selection>,
    /// Make drag operations more precise
    drag_state: DragState,
    /// Cursor Position in screen space
    cursor_pos: Option<Vector2<usize>>,
}

impl Timeline {
    pub fn new(clip: Clip) -> Self {
        Self {
            clip,
            offset: 0,
            samples_per_fft: 128,
            height: 256,
            width: 1,
            sample_len: 0,
            scale: 1024.0,
            vscale: 1.0,
            live: true,
            selection: None,
            drag_state: DragState::NotDragging,
            cursor_pos: None,
        }
    }

    /// Translate polar coordinates to vector position for IQ diagram
    fn polar_to_iq_idx(&self, magnitude: f32, phase: f32) -> usize {
        let x = ((1.0 + (phase.cos() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        let y = ((1.0 - (phase.sin() * magnitude)) * self.samples_per_fft as f32).floor() as usize;
        (y.clamp(0, self.samples_per_fft * 2 - 1) * self.samples_per_fft * 2)
            + x.clamp(0, self.samples_per_fft * 2 - 1)
    }

    /// Translate a sample to a screen coordinate
    fn sample_to_y_coordinate(&self, sample: f32) -> usize {
        let halfheight = self.height as f32 / 2f32;
        (self.vscale * sample * halfheight + halfheight) as usize
    }

    fn pointer_pos_from_response(&self, response: &Response) -> Option<Vector2<usize>> {
        self.input_pos(&response.rect, response.interact_pointer_pos())
    }

    fn correct_drag_delta(&mut self, response: &Response) -> Vector2<isize> {
        match self.drag_state {
            DragState::DownButNotDragging(pos) => {
                if let Some(cur) = self.pointer_pos_from_response(&response) {
                    self.drag_state = DragState::Dragging;
                    Vector2 {
                        x: cur.x as isize - pos.x as isize,
                        y: cur.y as isize - pos.y as isize,
                    }
                } else {
                    panic!("In dragging state but no current mouse position")
                }
            }
            DragState::Dragging => {
                let delta = response.drag_delta();
                Vector2 {
                    x: delta.x.floor() as isize,
                    y: delta.y.floor() as isize,
                }
            }
            DragState::NotDragging => panic!("Should not be able to get to this state"),
        }
    }

    fn pan_action(&mut self, delta: Vector2<isize>) {
        self.live = false;
        let newoffset = self.offset as isize - self.screen_to_data_x_without_offset(delta.x);
        self.offset = newoffset.clamp(0, isize::MAX) as usize;
    }

    fn update_and_show_sample_explorer(&mut self, ui: &mut egui::Ui) {
        // The amplitude image is drawn horizontally.
        // The most recent sample is on the right.
        // Zero is in the center. Lines drawn at +-128
        let mut samples_image =
            std::vec::from_elem(Color32::from_gray(0), self.width * self.height);

        // Draw selection area by highlighting background
        if let Some(Selection { range }) = &self.selection {
            for x in self.data_x_range_to_screen_x_range(range) {
                for y in 0..self.height() {
                    let idx = self.screen_to_image_idx(x, y);
                    samples_image[idx] = Color32::from_rgb(0, 0, 128);
                }
            }
        }

        // Acquire read lock on samples
        let read_lock = self.clip.read();
        let samples = &read_lock.samples;

        // Update for any changes in the sample data
        self.sample_len = samples.len();

        // If live, move with the live data
        if self.live {
            let data_vis_width = self.screen_to_data_x_without_offset(self.width as isize);
            let newoffset = self.sample_len as isize - data_vis_width;
            self.offset = if newoffset < 0 { 0 } else { newoffset as usize }
        }

        // Draw the sample amplitudes by looping over the width of the timeline view
        // Each pixel may represent one or more samples, we will deal with that inside the loo
        for i in 0..(self.width as usize) {
            // Skip drawing anything if there are no samples yet
            if samples.len() == 0 {
                break;
            }

            // Derive the sample range for the current screen X coordinate
            let sample_range = self.screen_x_coordinate_to_data_range(i);

            // sample_range will be empty if the beginning is beyond the length of the data, in which case we're done
            if sample_range.is_empty() {
                break;
            }

            // If the range only contains one sample, just draw one sample. This means scaling factor is 1.
            if sample_range.len() == 1 {
                let y = self.sample_to_y_coordinate(samples[sample_range.min().unwrap()]);
                let color = if y == 0 || y > self.height - 1 {
                    Color32::from_rgb(255, 0, 0)
                } else {
                    Color32::from_rgb(127, 127, 255)
                };
                samples_image[self.screen_to_image_idx(i, y)] = color;
            }
            // Otherwise we summarize a range of values within one pixel by their max and min
            else {
                let bucket = &samples[sample_range];

                // Take the maximum and minimum values over the samples in this bucket
                let (f32max, f32min) = bucket.iter().fold((f32::MIN, f32::MAX), |acc, x| {
                    (acc.0.max(*x), acc.1.min(*x))
                });

                let displaymax = self.sample_to_y_coordinate(f32max);
                let displaymin = self.sample_to_y_coordinate(f32min);

                for y in displaymin..displaymax {
                    let color = if y == 0 || y > self.height - 1 {
                        Color32::from_rgb(255, 0, 0)
                    } else {
                        Color32::from_rgb(127, 127, 255)
                    };
                    samples_image[self.screen_to_image_idx(i, y)] = color
                }
            }
        }

        drop(read_lock);

        // Overlay a vertical line representing the current cursor position if the mouse is hovering
        if let Some(pos) = self.cursor_pos {
            for i in 0..(self.height as usize) {
                let idx = self.screen_to_image_idx(pos.x, i);
                samples_image[idx] = Color32::from_rgb(255, 0, 0);
            }
        }

        // Create TextureHandle from Pixel Data
        let samples_texture = ui.ctx().load_texture(
            "samples",
            ColorImage::new([self.width, self.height], samples_image),
            TextureOptions::NEAREST,
        );

        // Show the timeline
        let samples_size = samples_texture.size_vec2();
        let samples_sized_texture = SizedTexture::new(&samples_texture, samples_size);
        let samples_image_widget =
            Image::new(samples_sized_texture).sense(Sense::click_and_drag() | Sense::hover());
        let samples_response = ui.add(samples_image_widget);

        // Handle mouse interaction with timeline

        // In egui, the "drag" deltas start reporting after the mouse has moved, and so if you click
        // precisely where you mean to begin the drag, it will not begin where you expected.
        // Submitting a patch to egui is probably better than this mess...
        if samples_response.is_pointer_button_down_on() {
            if self.drag_state == DragState::NotDragging
                && let Some(pos) = self.pointer_pos_from_response(&samples_response)
            {
                self.drag_state = DragState::DownButNotDragging(pos);
            }
        } else {
            self.drag_state = DragState::NotDragging;
        }
        if samples_response.dragged_by(PointerButton::Primary) {
            if let Some(cur) = self.pointer_pos_from_response(&samples_response) {
                let current = self.screen_to_data_x(cur.x as isize);
                if let DragState::DownButNotDragging(begin) = self.drag_state {
                    self.selection = Some(Selection::new(
                        self.screen_to_data_x(begin.x as isize) as usize,
                        current as usize,
                    ));
                } else if let Some(selection) = &mut self.selection {
                    selection.update_bounds(current as usize);
                }
            }
        } else if samples_response.dragged_by(PointerButton::Secondary) {
            let delta = self.correct_drag_delta(&samples_response);
            self.pan_action(delta);
        }
        if samples_response.hovered() {
            self.cursor_pos = self.input_pos(&samples_response.rect, samples_response.hover_pos());
            if let Some(pos) = self.cursor_pos {
                let newscale = self.scale * ui.input(|input| input.zoom_delta());
                self.update_scale(newscale, pos.x);
            }
            //self.scale *= ui.input(|input| input.zoom_delta());
        } else {
            self.cursor_pos = None;
        }
    }

    /// Updates the scale and offset, centered at screen_pos
    /// If we're "live", then only update the scale. The "live" mechanism will take care of the offset.
    pub fn update_scale(&mut self, scale: f32, screen_pos: usize) {
        // If the scale doesn't change, don't change anything
        let scale = scale.clamp(1.0f32, f32::MAX);
        if self.scale == scale {
            return;
        }
        let adj_offset = self.screen_to_data_x(screen_pos as isize);
        self.scale = scale;
        if !self.live {
            let new_offset = adj_offset - self.screen_to_data_x_without_offset(screen_pos as isize);
            self.offset = if new_offset < 0 {
                0
            } else {
                new_offset as usize
            };
        }
    }

    pub fn update_and_show(&mut self, ui: &mut egui::Ui) {
        // Get the current screen real estate that we have to work with
        self.width = ui.available_size().x.floor() as usize;

        // Show the timeline controls
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.live, "Live").on_hover_text(
                "If checked, the timeline will auto-scroll to keep up with live data.",
            );

            let mut newscale = self.scale;

            ui.add(
                DragValue::new(&mut newscale)
                    .range(1.0f32..=44100.0f32)
                    .prefix("Scale: "),
            )
            .on_hover_text("Scales the timeline view to N samples per 1 pixel.");

            ui.add(
                DragValue::new(&mut self.vscale)
                    .range(1.0f32..=200.0f32)
                    .prefix("VScale: "),
            )
            .on_hover_text("Scales the timeline amplitude");

            ui.label(format!("O: {}", self.offset));
            if let Some(pos) = self.cursor_pos {
                let range = self.screen_x_coordinate_to_data_range(pos.x);
                let text = if range.len() > 1 {
                    format!("P: {}-{}", range.start, range.end)
                } else {
                    format!("P: {}", range.start)
                };
                ui.label(text);
            }

            // If zooming using the widget, keep it centered
            let halfwidth = self.width / 2;
            self.update_scale(newscale, halfwidth);
        });

        // I am assuming that egui will scale this properly but it may need to be revisited after
        // experimentation. Look into ui.pixels_per_point() if necessary.

        // This is the sample amplitude display
        self.update_and_show_sample_explorer(ui);

        // The waterfall image is drawn horizontally (yes unusual but bear with me)
        // The most recent sample is on the right.
        // The fundamental is at the top.
        /*let mut waterfall_image =
            std::vec::from_elem(Color32::from_gray(0), self.width * self.samples_per_fft);

        // Temporary IQ image... curious what we get but this will have to be a separate control
        // and somehow linked into a selection range.
        let mut iq_image = std::vec::from_elem(
            Color32::from_gray(0),
            self.samples_per_fft * self.samples_per_fft * 4,
        );

        // Acquire read lock on fft
        //let fft_derp: Frequencies = Default::default(); //self.fft.read();
        //let fft = fft_derp.read();

        // Loop over the width of the timeline control
        // The relative positions within the sample vector can be derived from those indexes
        for i in 0..(width as usize) {
        if samples.len() == 0 {
            break;
        }

        // Now build the FFT image
        // There are always more samples than FFT data so if we get this far we are good
        let fft_range = self.screen_to_fft_range(i + self.sample_to_screen(self.offset % self.samples_per_fft), fft.len());
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
        }
        //}

        //drop(fft);

        let waterfall_texture = ui.ctx().load_texture(
            "waterfall",
            ColorImage::new([self.width as usize, self.samples_per_fft], waterfall_image),
            TextureOptions::NEAREST,
        );

        let iq_texture = ui.ctx().load_texture(
            "iq",
            ColorImage::new(
                [self.samples_per_fft * 2, self.samples_per_fft * 2],
                iq_image,
            ),
            TextureOptions::NEAREST,
        );

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
        let iq_response = ui.add(iq_image_widget);*/
    }
}

impl Scaler for Timeline {
    fn screen_space(&self) -> Vector2<usize> {
        Vector2 {
            x: self.width,
            y: self.height,
        }
    }

    fn data_space(&self) -> Vector2<usize> {
        Vector2 {
            x: self.sample_len,
            y: self.height,
        }
    }

    fn scale(&self) -> Vector2<f32> {
        Vector2 {
            x: self.scale,
            y: self.vscale,
        }
    }

    fn offset(&self) -> usize {
        self.offset
    }
}
