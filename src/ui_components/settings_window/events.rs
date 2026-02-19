use gpui::*;

#[derive(Clone, Debug)]
pub enum SettingsEvent {
    Close,
    ConfigChanged, // Emitted when settings are changed
    ShowToast(String),
}

impl EventEmitter<SettingsEvent> for super::SettingsWindow {}
