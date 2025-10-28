use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use egui::{Ui, Window, scroll_area::ScrollBarVisibility};

use crate::{
    data::audio::{Clip, ClipId},
    gui::timeline::Timeline,
};

pub struct ClipExplorer {
    pub open: bool,
    title: String,
    timeline: Timeline,
}

impl ClipExplorer {
    pub fn new(clip: Clip) -> Self {
        let title = clip.read().id().to_string();
        let timeline = Timeline::new(clip);
        Self {
            title,
            timeline,
            open: true,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx();

        // TODO:
        // Analysis - show window
        // OpenClip - hold the transient data for GUI ie texture cache
        // Split Timeline into Samples, Waterfall; tie together with Scroll
        //  (I think)
        Window::new(&self.title)
            .constrain_to(ui.clip_rect())
            .scroll(true)
            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
            .open(&mut self.open)
            .show(ctx, |ui| {
                self.timeline.update_and_show(ui);
            });
    }
}

#[derive(Default)]
pub struct OpenClips(BTreeMap<ClipId, ClipExplorer>);

impl OpenClips {
    pub fn show_editor_windows(&mut self, ui: &mut egui::Ui) {
        for clipeditor in self.0.values_mut() {
            clipeditor.show(ui);
        }
    }

    pub fn show_clip_list(&mut self, ui: &mut egui::Ui) {
        let mut first = true;
        for (clip_id, clipeditor) in self.0.iter_mut() {
            if !first {
                ui.separator();
            }
            first = false;
            if ui.button(clip_id.to_string()).clicked() {
                clipeditor.open = true;
            }
        }
    }
}

impl Deref for OpenClips {
    type Target = BTreeMap<ClipId, ClipExplorer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OpenClips {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
