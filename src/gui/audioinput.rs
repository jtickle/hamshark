use crate::data::audioinput::AudioInputDeviceBuilder;
use crate::gui::View;
use cpal::{SampleFormat, SupportedStreamConfigRange, available_hosts, traits::DeviceTrait};
use egui::{ComboBox, Id, Modal, Ui};

impl View for AudioInputDeviceBuilder {
    fn show(&mut self, ui: &mut Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce()) {
        Modal::new(Id::new("Audio Input Chooser")).show(ui.ctx(), |ui| {
            // Title
            ui.heading("Configure Audio Source");

            // Select Audio Host
            let mut selected_host_id = self.host_id.clone();
            ComboBox::new("audioinput_host", "Host")
                .selected_text(selected_host_id.name())
                .show_ui(ui, |ui| {
                    for host_id in available_hosts() {
                        ui.selectable_value(&mut selected_host_id, host_id, host_id.name());
                    }
                });
            if selected_host_id != self.host_id {
                self.host_id = selected_host_id;
            }

            // If there is no selected device, then select the default one for the host.
            if self.device.is_none() {
                self.device = self.get_default_device();
            }
            let device = self.device.as_ref().unwrap().clone();
            let current_selected_device_name = device
                .name()
                .expect("a device existed and should have a name");
            let mut selected_device_name = current_selected_device_name.clone();

            // Show Audio Device Selector
            ComboBox::new("audioinput_device", "Device")
                .selected_text(&selected_device_name)
                .show_ui(ui, |ui| {
                    for device in self.input_devices() {
                        let device_name = device
                            .name()
                            .expect("There was a device, it ought to be named");
                        ui.selectable_value(
                            &mut selected_device_name,
                            device_name.clone(),
                            device_name,
                        );
                    }
                });
            if selected_device_name != current_selected_device_name {
                self.device = None;
                for device in self.input_devices() {
                    if device.name().expect("Device should have a name") == selected_device_name {
                        self.device = Some(device);
                        self.config = self.get_default_config();
                        break;
                    }
                }
                if self.device.is_none() {
                    panic!("Unable to select a device by name");
                }
            }

            // If there is no selected config, then select the default one for the device.
            if self.config.is_none() {
                self.config = self.get_default_config();
            }
            let config = self.config.as_ref().unwrap().clone();

            let supported_configs_range: Vec<SupportedStreamConfigRange> = device
                .supported_input_configs()
                .expect("Selected device has no supported input configs")
                .filter(|config| config.sample_format() == SampleFormat::F32)
                .collect();
            let selected_config_name = format!(
                "Channels: {}, Rate: {}",
                config.channels, config.sample_rate.0
            );
            let mut selected_config = config.clone();

            // Show input configuration selector
            ComboBox::new("audioinput_config", "Configuration")
                .selected_text(selected_config_name)
                .show_ui(ui, |ui| {
                    for config in supported_configs_range {
                        let config_display = format!(
                            "Channels: {}, Rate: {}",
                            config.channels(),
                            config.max_sample_rate().0
                        );
                        ui.selectable_value(
                            &mut selected_config,
                            config.with_max_sample_rate().config(),
                            config_display,
                        );
                    }
                });
            if selected_config != config {
                self.config = Some(selected_config);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("Save").clicked() {
                    on_save();
                }
                if ui.button("Cancel").clicked() {
                    on_cancel();
                }
            })
        });
    }
}
