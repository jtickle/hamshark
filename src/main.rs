use hamshark::HamShark;
use crate::gui::HamSharkGui;
use crate::config::{Configuration, Settings};
use log::{debug};

mod gui;
mod config;
pub mod data;
fn main() -> eframe::Result<()>{
    env_logger::init();
    let native_options = eframe::NativeOptions::default();

    // TODO: show the user an error message instead of unwrapping these
    let config = Configuration::from_env().unwrap();
    debug!("{:?}", config);
    let settings = Settings::from_file(config.settings_file_path.as_path()).unwrap();
    debug!("{:?}", settings);

    let hs = HamShark::new();

    eframe::run_native(
        "Hamshark",
        native_options,
        Box::new(|_cc| Ok(Box::new(HamSharkGui::new(hs, config, settings)))))
}

