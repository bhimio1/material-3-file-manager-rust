//! Interop with [matugen](https://github.com/InioX/matugen): the template
//! that renders the `theme.json` material-3-ui reads, and the config
//! snippet that installs it.

use std::path::PathBuf;

pub const TEMPLATE: &str = r#"{
    "primary": "{{colors.primary.default.hex}}",
    "on_primary": "{{colors.on_primary.default.hex}}",
    "primary_container": "{{colors.primary_container.default.hex}}",
    "on_primary_container": "{{colors.on_primary_container.default.hex}}",
    "secondary": "{{colors.secondary.default.hex}}",
    "on_secondary": "{{colors.on_secondary.default.hex}}",
    "secondary_container": "{{colors.secondary_container.default.hex}}",
    "on_secondary_container": "{{colors.on_secondary_container.default.hex}}",
    "tertiary": "{{colors.tertiary.default.hex}}",
    "on_tertiary": "{{colors.on_tertiary.default.hex}}",
    "tertiary_container": "{{colors.tertiary_container.default.hex}}",
    "on_tertiary_container": "{{colors.on_tertiary_container.default.hex}}",
    "error": "{{colors.error.default.hex}}",
    "on_error": "{{colors.on_error.default.hex}}",
    "error_container": "{{colors.error_container.default.hex}}",
    "on_error_container": "{{colors.on_error_container.default.hex}}",
    "background": "{{colors.background.default.hex}}",
    "on_background": "{{colors.on_background.default.hex}}",
    "surface": "{{colors.surface.default.hex}}",
    "on_surface": "{{colors.on_surface.default.hex}}",
    "surface_variant": "{{colors.surface_variant.default.hex}}",
    "on_surface_variant": "{{colors.on_surface_variant.default.hex}}",
    "outline": "{{colors.outline.default.hex}}",
    "outline_variant": "{{colors.outline_variant.default.hex}}",
    "shadow": "{{colors.shadow.default.hex}}",
    "scrim": "{{colors.scrim.default.hex}}",
    "inverse_surface": "{{colors.inverse_surface.default.hex}}",
    "inverse_on_surface": "{{colors.inverse_on_surface.default.hex}}",
    "inverse_primary": "{{colors.inverse_primary.default.hex}}",
    "surface_container_lowest": "{{colors.surface_container_lowest.default.hex}}",
    "surface_container_low": "{{colors.surface_container_low.default.hex}}",
    "surface_container": "{{colors.surface_container.default.hex}}",
    "surface_container_high": "{{colors.surface_container_high.default.hex}}",
    "surface_container_highest": "{{colors.surface_container_highest.default.hex}}"
}"#;

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("material_3_file_manager"))
}

/// The file matugen writes; material-3-ui reads it (after end4-pC's colors)
/// and follows it live.
pub fn theme_json_path() -> Option<PathBuf> {
    material_3_ui::theme::file_manager_theme_path()
}

pub fn template_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("matugen_template.json"))
}

/// Snippet for `~/.config/matugen/config.toml`.
pub fn config_snippet() -> String {
    let template = template_path().unwrap_or_default();
    let output = theme_json_path().unwrap_or_default();
    format!(
        "[templates.m3fm]\ninput_path = \"{}\"\noutput_path = \"{}\"\n",
        template.display(),
        output.display()
    )
}

pub fn write_template() -> anyhow::Result<PathBuf> {
    let path = template_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, TEMPLATE)?;
    Ok(path)
}
