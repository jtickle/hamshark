use cpal::{available_hosts, traits::DeviceTrait};
use egui::{ComboBox, Id, Modal, Ui};
use crate::data::audioinput::{
    AudioInputDeviceBuilder,
};
use log::info;
use crate::gui::View;

impl View for AudioInputDeviceBuilder {
    fn show(&mut self, ui: &mut Ui, on_save: impl FnOnce(), on_cancel: impl FnOnce()) {
        Modal::new(Id::new("Audio Input Chooser"))
            .show(ui.ctx(), |ui| {

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

                // Select Audio Device
                if let Some(selected_device) = self.device.clone() {
                    let mut selected_device_name = selected_device.name().expect("There was a selected_device, it ought to be named");
                    ComboBox::new("audioinput_device", "Device")
                        .selected_text(selected_device_name.clone())
                        .show_ui(ui, |ui| {
                            for device in self.input_devices() {
                                let device_name = device.name().expect("There was a device, it ought to be named");
                                ui.selectable_value(&mut selected_device_name, device_name.clone(), device_name);
                            }
                        });
                    if selected_device_name != self.device.as_ref().expect("There should still be a device").name().expect("There was a device, it should have a name") {
                        for device in self.input_devices() {
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

                if ui.button("Save").clicked() {
                    on_save();
                }
                if ui.button("Cancel").clicked() {
                    on_cancel();
                }
            });
    }
}