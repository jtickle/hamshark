use crate::config::{Configuration, Settings};
use crate::data::audioinput::AudioInputDeviceBuilder;
use crate::gui::HamSharkGui;
use crate::session::Session;
use log::debug;

mod config;
mod data;
mod gui;
mod pipeline;
mod session;
mod tools;

fn main() -> eframe::Result<()> {
    env_logger::init();
    let native_options = eframe::NativeOptions::default();

    // TODO: show the user an error message instead of unwrapping these
    let config = Configuration::from_env().unwrap();
    debug!("{:?}", config);
    let settings = Settings::from_file(config.settings_file_path.as_path()).unwrap();
    debug!("{:?}", settings);
    let mut session = Session::from_settings(&settings).expect("Able to create session");
    session
        .configure(AudioInputDeviceBuilder::default().build().unwrap())
        .unwrap();

    eframe::run_native(
        "Hamshark",
        native_options,
        Box::new(|_cc| Ok(Box::new(HamSharkGui::new(session, config, settings)))),
    )
}
