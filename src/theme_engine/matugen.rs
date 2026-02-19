use std::fs;
use std::path::PathBuf;

pub fn generate_template() -> anyhow::Result<PathBuf> {
    let template_content = r#"{
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

    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
    let app_config_dir = config_dir.join("material_3_file_manager");

    if !app_config_dir.exists() {
        fs::create_dir_all(&app_config_dir)?;
    }

    let template_path = app_config_dir.join("matugen_template.json");
    fs::write(&template_path, template_content)?;

    Ok(template_path)
}
