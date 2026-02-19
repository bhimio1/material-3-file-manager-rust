#![allow(dead_code)]
use super::config::ThemeConfig;
use gpui::Rgba;

#[derive(Clone)]
pub struct M3Palette {
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub primary_container: Rgba,
    pub on_primary_container: Rgba,
    pub secondary: Rgba,
    pub on_secondary: Rgba,
    pub secondary_container: Rgba,
    pub on_secondary_container: Rgba,
    pub tertiary: Rgba,
    pub on_tertiary: Rgba,
    pub tertiary_container: Rgba,
    pub on_tertiary_container: Rgba,
    pub error: Rgba,
    pub on_error: Rgba,
    pub error_container: Rgba,
    pub on_error_container: Rgba,
    pub background: Rgba,
    pub on_background: Rgba,
    pub surface: Rgba,
    pub on_surface: Rgba,
    pub surface_variant: Rgba,
    pub on_surface_variant: Rgba,
    pub outline: Rgba,
    pub outline_variant: Rgba,
    pub shadow: Rgba,
    pub scrim: Rgba,
    pub inverse_surface: Rgba,
    pub inverse_on_surface: Rgba,
    pub inverse_primary: Rgba,
    pub surface_container_lowest: Rgba,
    pub surface_container_low: Rgba,
    pub surface_container: Rgba,
    pub surface_container_high: Rgba,
    pub surface_container_highest: Rgba,
}

impl M3Palette {
    pub fn from_hex(_hex_code: u32) -> Self {
        let c = |r: u32, g: u32, b: u32| {
            let hex = (r << 24) | (g << 16) | (b << 8) | 0xFF;
            gpui::rgba(hex)
        };

        Self {
            primary: c(0x42, 0x85, 0xF4),
            on_primary: c(255, 255, 255),
            primary_container: c(0xD2, 0xE3, 0xFC),
            on_primary_container: c(0x00, 0x1D, 0x3D),
            secondary: c(0x5F, 0x63, 0x68),
            on_secondary: c(255, 255, 255),
            secondary_container: c(0xDA, 0xDC, 0xE0),
            on_secondary_container: c(0x1F, 0x1F, 0x1F),
            tertiary: c(0xDB, 0x44, 0x37),
            on_tertiary: c(255, 255, 255),
            tertiary_container: c(0xFA, 0xD2, 0xCF),
            on_tertiary_container: c(0x3C, 0x0D, 0x09),
            error: c(0xB0, 0x00, 0x20),
            on_error: c(255, 255, 255),
            error_container: c(0xF9, 0xD6, 0xD6),
            on_error_container: c(0x37, 0x00, 0x0B),
            background: c(0xFF, 0xFF, 0xFF),
            on_background: c(0x1F, 0x1F, 0x1F),
            surface: c(0xFF, 0xFF, 0xFF),
            on_surface: c(0x1F, 0x1F, 0x1F),
            surface_variant: c(0xF1, 0xF3, 0xF4),
            on_surface_variant: c(0x5F, 0x63, 0x68),
            surface_container_lowest: c(0xFF, 0xFF, 0xFF),
            surface_container_low: c(0xF7, 0xF9, 0xFA),
            surface_container: c(0xF0, 0xF2, 0xF4),
            surface_container_high: c(0xE8, 0xEA, 0xED),
            surface_container_highest: c(0xE3, 0xE5, 0xE8),
            outline: c(0xBD, 0xC1, 0xC6),
            outline_variant: c(0xC0, 0xC4, 0xC8), // Fixed placeholder
            shadow: c(0, 0, 0),
            scrim: c(0, 0, 0),
            inverse_surface: c(0x30, 0x31, 0x34),
            inverse_on_surface: c(0xFF, 0xFF, 0xFF),
            inverse_primary: c(0x8A, 0xB4, 0xF8),
        }
    }
}

impl From<ThemeConfig> for M3Palette {
    fn from(config: ThemeConfig) -> Self {
        let default = Self::from_hex(0x4285F4); // Fallback default
        let p = |opt: Option<String>, fallback: Rgba| -> Rgba {
            opt.as_deref().and_then(hex_to_rgba).unwrap_or(fallback)
        };

        Self {
            primary: p(config.primary, default.primary),
            on_primary: p(config.on_primary, default.on_primary),
            primary_container: p(config.primary_container, default.primary_container),
            on_primary_container: p(config.on_primary_container, default.on_primary_container),
            secondary: p(config.secondary, default.secondary),
            on_secondary: p(config.on_secondary, default.on_secondary),
            secondary_container: p(config.secondary_container, default.secondary_container),
            on_secondary_container: p(
                config.on_secondary_container,
                default.on_secondary_container,
            ),
            tertiary: p(config.tertiary, default.tertiary),
            on_tertiary: p(config.on_tertiary, default.on_tertiary),
            tertiary_container: p(config.tertiary_container, default.tertiary_container),
            on_tertiary_container: p(config.on_tertiary_container, default.on_tertiary_container),
            error: p(config.error, default.error),
            on_error: p(config.on_error, default.on_error),
            error_container: p(config.error_container, default.error_container),
            on_error_container: p(config.on_error_container, default.on_error_container),
            background: p(config.background, default.background),
            on_background: p(config.on_background, default.on_background),
            surface: p(config.surface, default.surface),
            on_surface: p(config.on_surface, default.on_surface),
            surface_variant: p(config.surface_variant, default.surface_variant),
            on_surface_variant: p(config.on_surface_variant, default.on_surface_variant),
            outline: p(config.outline, default.outline),
            outline_variant: p(config.outline_variant, default.outline_variant),
            shadow: p(config.shadow, default.shadow),
            scrim: p(config.scrim, default.scrim),
            inverse_surface: p(config.inverse_surface, default.inverse_surface),
            inverse_on_surface: p(config.inverse_on_surface, default.inverse_on_surface),
            inverse_primary: p(config.inverse_primary, default.inverse_primary),
            surface_container_lowest: p(
                config.surface_container_lowest,
                default.surface_container_lowest,
            ),
            surface_container_low: p(config.surface_container_low, default.surface_container_low),
            surface_container: p(config.surface_container, default.surface_container),
            surface_container_high: p(
                config.surface_container_high,
                default.surface_container_high,
            ),
            surface_container_highest: p(
                config.surface_container_highest,
                default.surface_container_highest,
            ),
        }
    }
}

pub fn hex_to_rgba(hex: &str) -> Option<Rgba> {
    let hex = hex.trim_start_matches('#');
    let val = u32::from_str_radix(hex, 16).ok()?;

    match hex.len() {
        6 => {
            // RRGGBB -> RRGGBBFF
            Some(gpui::rgba((val << 8) | 0xFF))
        }
        8 => {
            // RRGGBBAA -> RRGGBBAA
            Some(gpui::rgba(val))
        }
        _ => None,
    }
}

// Helper to convert material_colors [u8; 4] (ARGB) to GPUI Rgba
fn to_rgba(argb: [u8; 4]) -> Rgba {
    let a = argb[0] as u32;
    let r = argb[1] as u32;
    let g = argb[2] as u32;
    let b = argb[3] as u32;

    let hex = (r << 24) | (g << 16) | (b << 8) | a;
    gpui::rgba(hex)
}
