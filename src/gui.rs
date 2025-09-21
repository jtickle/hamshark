mod audioinput;
mod amplitudes;

use chrono::Utc;
use eframe::egui::{
        CentralPanel, Context
    };
use log::{debug, trace};
use crate::{data::audioinput::AudioInputDeviceBuilder, gui::amplitudes::Amplitudes, session::Session};
use crate::config::{Configuration, Settings};

use open;

const GPLV3: &str = "https://www.gnu.org/licenses/gpl-3.0.en.html";
const REPO: &str = "https://git.serenity.jefftickle.com/jwt/hamshark";

pub struct HamSharkGui {
    session: Session,
    config: Configuration,
    settings: Settings,

    audio_input_selecting: Option<AudioInputDeviceBuilder>,

    amplitudes: Option<Amplitudes>,
}

impl HamSharkGui {
    pub fn new(session: Session, config: Configuration, settings: Settings) -> Self {
        Self {
            session,
            config,
            settings,
            audio_input_selecting: None,
            amplitudes: None,
        }
    }
}

pub trait View {
    fn show(&mut self, ui: &mut egui::Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce());
}

impl eframe::App for HamSharkGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let begin = Utc::now();
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let path = self.session.path.to_str();
                ui.label(format!("Live Session: {}", path.unwrap_or("OS STR DECODE ERROR")));
                if let Some(p) = path {
                    if ui.button("Browse").clicked() {
                        open::that(p).expect(format!("Could not open {}", p).as_str());
                    }
                }
                ui.separator();
                if ui.button("GPLv3").clicked() {
                    open::that(GPLV3).expect(format!("Could not open browser to GPLv3 at {} ... fortunately this software is open source, so you can fix that bug!", GPLV3).as_str());
                }
                ui.separator();
                if ui.button("Source").clicked() {
                    open::that(REPO).expect(format!("Could not open browser to code repository at {} ... fortunately this software is open source, so you ca nfix that bug!", REPO).as_str());
                }
            })
        });
        CentralPanel::default().show(ctx, |ui| {
            log::trace!("Updating GUI, dt is {}", ctx.input(|i| i.stable_dt));

            if ui.button("Configure Live Audio Input").clicked() {
                self.audio_input_selecting = match self.session.configuration() {
                    Some(config) => Some(config.into()),
                    None => Some(AudioInputDeviceBuilder::default()),
                };
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
                        let audiodevice = data.build().expect("should have built an audio device");
                        self.session.configure(audiodevice).expect("should have configured an input device");
                    } else if !should_cancel {
                        self.audio_input_selecting = Option::Some(data);
                    }
                },
                None => (),
            }

            match self.session.is_started() {
                true => {
                    if ui.button("Stop").clicked() {
                        self.session.stop().unwrap();
                    }

                },
                false => {
                    if ui.button("Start").clicked() {
                        self.session.start().unwrap();
                        self.amplitudes = Some(Amplitudes::new(
                            self.session.amplitudes(),
                            self.session.configuration().unwrap().config.sample_rate
                        ));
                    }
                }
            }

            if let Some(amplitudes) = &mut self.amplitudes {
                amplitudes.show(ui);
            }


            
            // Amplitude display
            /* 
            //ScrollArea::horizontal().show(ui, |ui| {
                debug!("Available size: {:?}", ui.available_size());
                let amplitude_scale = 255u8;
                let raw_amplitudes_arc = self.session.amplitudes();
                let raw_amplitudes = raw_amplitudes_arc.read();
                let amplitude_range = raw_amplitudes.len();
                let mut amplitude_image = std::vec::from_elem(Color32::from_gray(0), amplitude_scale as usize * amplitude_range);
                let f32scale: f32 = amplitude_scale.into();
                for i in 0..amplitude_range {
                    let display = (raw_amplitudes[i] * f32scale) as usize;
                    amplitude_image[(display * amplitude_range) + i] = Color32::from_gray(255);
                }

                let amplitude_texture = ctx.load_texture(
                    "amplitudes",
                    ColorImage::new([amplitude_range, amplitude_scale.into()], amplitude_image),
                    TextureOptions::NEAREST,
                );

                let amplitude_size = amplitude_texture.size_vec2();
                let amplitude_sized_texture = egui::load::SizedTexture::new(&amplitude_texture, amplitude_size);

                ui.add(Image::new(amplitude_sized_texture));
            //});*/
        });

        debug!("Frame drawn in {}", Utc::now() - begin);

        // Request a repaint to keep the UI updated
        //ctx.request_repaint();
    }
}
