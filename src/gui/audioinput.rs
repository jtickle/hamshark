use egui::{ComboBox, Id, Modal, Ui};
use hamshark::data::audioinput::{
    AudioInputBuilder,
    AudioInputBuilderSource,
};
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
                        let rendered_path = file.to_string_lossy().to_string();
                        ui.horizontal(|ui| {
                            ui.monospace(rendered_path);
                            if ui.button("Browse").clicked() {
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

                    },
                }
                ui.label(format!("{}", self.source));
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