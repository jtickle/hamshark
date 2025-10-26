pub mod audioinput;
pub mod timeline;
pub mod audio;

use chrono::Utc;
use eframe::egui::{
        CentralPanel, Context
    };
use egui::Button;
use log::info;
use crate::{data::audioinput::AudioInputDeviceBuilder, session::Session};
use crate::config::{Configuration, Settings};

use open;

const GPLV3: &str = "https://www.gnu.org/licenses/gpl-3.0.en.html";
const REPO: &str = "https://git.serenity.jefftickle.com/jwt/hamshark";

pub struct HamSharkGui {
    session: Session,
    config: Configuration,
    settings: Settings,

    audio_input_selecting: Option<AudioInputDeviceBuilder>,
}

impl HamSharkGui {
    pub fn new(session: Session, config: Configuration, settings: Settings) -> Self {
        Self {
            session,
            config,
            settings,
            audio_input_selecting: None,
        }
    }
}

pub trait View {
    fn show(&mut self, ui: &mut egui::Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce());
}

impl eframe::App for HamSharkGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {

        let begin = Utc::now();

        // Top Menu Bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Configure Audio").clicked() {
                        self.audio_input_selecting = match self.session.configuration() {
                            Some(config) => Some(config.into()),
                            None => Some(AudioInputDeviceBuilder::default()),
                        };
                    }
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                })
            });
        });

        // Tool Bar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            let button = Button::new("➕");
            let enabled = ! self.session.is_recording();
            if ui.add_enabled(enabled, button).clicked() {
                self.session.record_new_clip().unwrap();
            }
        });

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
                    open::that(GPLV3).expect(format!("Could not open browser to GPLv3 at {} ... fortunately this is Free software, so you can fix that bug!", GPLV3).as_str());
                }
                ui.separator();
                if ui.button("Source").clicked() {
                    open::that(REPO).expect(format!("Could not open browser to code repository at {} ... fortunately this is Free software, so you can fix that bug!", REPO).as_str());
                }
            })
        });

        // Session Overview
        egui::SidePanel::left("clips").show(ctx, |ui| {
            self.session.clips.show_clip_list(ui);
        });

        // Main content panel
        CentralPanel::default().show(ctx, |ui| {
            log::trace!("Updating GUI, dt is {}", ctx.input(|i| i.stable_dt));

            // Show all of the open clip viewers
            self.session.clips.show_editor_windows(ui);

            // Show audio configuration if open
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
        });

        //debug!("Frame drawn in {}", Utc::now() - begin);

        // Request repaint if we're "running"
        if self.session.is_recording() {
            ctx.request_repaint();
        }
    }
}
