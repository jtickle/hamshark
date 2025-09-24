mod audioinput;
mod timeline;

use chrono::Utc;
use eframe::egui::{
        CentralPanel, Context
    };
use log::{debug};
use crate::{data::audioinput::AudioInputDeviceBuilder, gui::timeline::Timeline, session::Session};
use crate::config::{Configuration, Settings};

use open;

const GPLV3: &str = "https://www.gnu.org/licenses/gpl-3.0.en.html";
const REPO: &str = "https://git.serenity.jefftickle.com/jwt/hamshark";

pub struct HamSharkGui {
    session: Session,
    config: Configuration,
    settings: Settings,

    audio_input_selecting: Option<AudioInputDeviceBuilder>,

    amplitudes: Option<Timeline>,
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

        // Add some status to the bottom of the window
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

        // Main content panel
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
                        self.amplitudes = Some(Timeline::new(
                            self.session.samples(),
                            self.session.fft(),
                            self.session.configuration().unwrap().config.sample_rate
                        ));
                    }
                }
            }

            if let Some(amplitudes) = &mut self.amplitudes {
                amplitudes.show(ui);
            }
        });

        //debug!("Frame drawn in {}", Utc::now() - begin);

        // Request repaint if we're "running"
        if self.session.is_started() {
            ctx.request_repaint();
        }
    }
}
