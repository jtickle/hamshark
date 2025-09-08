mod audioinput;

use eframe::{
    egui::{
        containers::Frame, emath, epaint, lerp, pos2, remap, vec2, hex_color, CentralPanel, Color32, Context, Pos2, Rect
    }, epaint::PathStroke
};
use hamshark::HamShark;
use hamshark::data::audioinput::AudioInputBuilder;

use crate::config::{Configuration, Settings};

pub struct HamSharkGui {
    colors: bool,
    hamshark: HamShark,

    config: Configuration,
    settings: Settings,

    audio_input_selecting: Option<AudioInputBuilder>,
    audio_input: AudioInputBuilder,
}

impl HamSharkGui {
    pub fn new(hs: HamShark, config: Configuration, settings: Settings) -> Self {
        Self {
            colors: true,
            hamshark: hs,
            config: config,
            settings: settings,
            audio_input_selecting: None,
            audio_input: AudioInputBuilder::default(),
        }
    }
}

pub trait View {
    fn show(&mut self, ui: &mut egui::Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce());
}

impl eframe::App for HamSharkGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            let color = if ui.visuals().dark_mode {
                Color32::from_additive_luminance(196)
            } else {
                Color32::from_black_alpha(240)
            };

            ui.checkbox(&mut self.colors, "Colored")
                .on_hover_text("Splash some color in 'er");

            if ui.button("Configure Live Audio Input").clicked() {
                self.audio_input_selecting = Option::Some(self.audio_input.clone());
            }
            match self.audio_input_selecting.take() {
                Some(mut data) => {
                    let mut should_save = false;
                    let mut should_cancel = false;
                    data.show(ui, || {
                        should_save = true;
                    }, || {
                        should_cancel = true;
                    });
                    if should_save {
                        self.hamshark.update_audio_input(data.build().unwrap());
                    } else if !should_cancel {
                        self.audio_input_selecting = Option::Some(data);
                    }
                },
                None => (),
            }

            match self.hamshark.is_started() {
                true => {
                    if ui.button("Stop").clicked() {
                        self.hamshark.stop().unwrap();
                    }

                },
                false => {
                    if ui.button("Start").clicked() {
                        self.hamshark.start();
                    }
                }
            }

            /*let hosts = cpal::available_hosts();
            let selected_host = hosts[0].name().to_owned().as_mut_str();
            egui::ComboBox::from_label("Host")
                .show_ui(ui, |ui| {
                    cpal::available_hosts().into_iter().map(|host| {
                        ui.selectable_value(selected_host, host.name(), host.name());
                    })
                });*/

            Frame::canvas(ui.style()).show(ui, |ui| {
                ui.ctx().request_repaint();
                let time = ui.input(|i| i.time);

                let desired_size = ui.available_width() * vec2(1.0, 0.35);
                let (_id, rect) = ui.allocate_space(desired_size);

                let to_screen =
                    emath::RectTransform::from_to(Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0), rect);

                let mut shapes = vec![];

                for &mode in &[1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0] {
                    let mode = mode as f64;
                    let n = 120;
                    let speed = 1.5;

                    let points: Vec<Pos2> = (0..=n)
                        .map(|i| {
                            let t = i as f64 / (n as f64);
                            let amp = (time * speed * mode).sin() / mode;
                            let y = amp * (t * std::f64::consts::TAU / 2.0 * mode).sin();
                            to_screen * pos2(t as f32, y as f32)
                        })
                        .collect();

                    let thickness = 10.0 / mode as f32;
                    shapes.push(epaint::Shape::line(
                        points,
                        if self.colors {
                            PathStroke::new_uv(thickness, move |rect, p| {
                                let t = remap(p.x, rect.x_range(), -1.0..=1.0).abs();
                                let center_color = hex_color!("#5BCEFA");
                                let outer_color = hex_color!("#F5A9B8");

                                Color32::from_rgb(
                                    lerp(center_color.r() as f32..=outer_color.r() as f32, t) as u8,
                                    lerp(center_color.g() as f32..=outer_color.g() as f32, t) as u8,
                                    lerp(center_color.b() as f32..=outer_color.b() as f32, t) as u8,
                                )
                            })
                        } else {
                            PathStroke::new(thickness, color)
                        },
                    ));
                }

                ui.painter().extend(shapes);
            })
        });

        // Request a repaint to keep the UI updated
        //ctx.request_repaint();
    }
}
