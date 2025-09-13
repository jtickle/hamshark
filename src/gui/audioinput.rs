use cpal::{traits::DeviceTrait, Device};
use egui::{ComboBox, Id, Modal, Ui};
use hamshark::data::audioinput::{
    AudioInputBuilder,
    AudioInputBuilderSource,
};
use log::info;
use crate::gui::View;

impl View for AudioInputBuilder {
    fn show(&mut self, ui: &mut Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce()) {
        Modal::new(Id::new("Audio Input Chooser"))
            .show(ui.ctx(), |ui| {

                // Title
                ui.heading("Configure Audio Source");

                // Choose live or file input
                ComboBox::new("audioinput_source", "Source")
                    .selected_text(self.source.to_string())
                    .show_ui(ui, |ui| {
                        vec![
                            AudioInputBuilderSource::FromDevice,
                            AudioInputBuilderSource::FromFile,
                        ].into_iter().map(|v| {
                            let text = v.to_string();
                            ui.selectable_value(&mut self.source, v, text);
                        }).for_each(drop);
                    });

                match self.source {

                    // Show controls for file-based input
                    AudioInputBuilderSource::FromFile => {

                        let file = self.file.clone().unwrap_or_default();
                        let s = file.to_string_lossy().to_string();
                        let rendered_path = if s.is_empty() {
                            "Select an Audio File".to_string()
                        } else {
                            s
                        };

                        ui.horizontal(|ui| {
                            let w_path = ui.monospace(rendered_path);
                            let w_button = ui.button("Browse");
                            if w_path.clicked() || w_button.clicked() {
                                let mut fd = rfd::FileDialog::new();
                                if let Some(path) = &self.file {
                                    if let Some(dir) = path.parent() {
                                        fd = fd.set_directory(dir);
                                    }
                                };

                                if let Some(file) = fd.pick_file() {
                                    self.file = Some(file);
                                }
                            }
                        });
                    },

                    // Show controls for live input
                    AudioInputBuilderSource::FromDevice => {

                        // Select Audio Host
                        let mut selected_host_id = self.host.clone();
                        ComboBox::new("audioinput_host", "Host")
                            .selected_text(selected_host_id.name())
                            .show_ui(ui, |ui| {
                                for host_id in self.hosts.clone() {
                                    ui.selectable_value(&mut selected_host_id, host_id, host_id.name());
                                }
                            });
                        if selected_host_id != self.host {
                            self.host = selected_host_id;
                        }

                        // Select Audio Device
                        if let Some(selected_device) = self.device.clone() {
                            let mut selected_device_name = selected_device.name().expect("There was a selected_device, it ought to be named");
                            ComboBox::new("audioinput_device", "Device")
                                .selected_text(selected_device_name.clone())
                                .show_ui(ui, |ui| {
                                    for device in self.devices.clone() {
                                        let device_name = device.name().expect("There was a device, it ought to be named");
                                        ui.selectable_value(&mut selected_device_name, device_name.clone(), device_name);
                                    }
                                });
                            if selected_device_name != self.device.as_ref().expect("There should still be a device").name().expect("There was a device, it should have a name") {
                                for device in self.devices.clone() {
                                    if device.name().expect("Device should have a name") == selected_device_name {
                                        self.device = Some(device);
                                        break;
                                    }
                                }
                                if self.device.is_none() {
                                    panic!("Unable to select a device by name");
                                }
                            }

                            let supported_configs_range = selected_device.supported_input_configs().expect("No supported input configs found for selected device");
                            for c in supported_configs_range {
                                info!("Supported Input Config: {} {:?} {} {:?} {:?}", c.sample_format(), c.buffer_size(), c.channels(), c.min_sample_rate(), c.max_sample_rate())

                            };
                        }
                    },
                }

                if ui.button("Save").clicked() {
                    on_save();
                }
                if ui.button("Cancel").clicked() {
                    on_cancel();
                }
                    /*ComboBox::new("host", "Audio Host")
                        .show_index(
                            ui,
                            input_device.host.id().name(),

                        )

                    });*/
            });
    }
}