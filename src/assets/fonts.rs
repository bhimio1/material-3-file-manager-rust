use gpui::*;

pub fn load_fonts(cx: &mut App) {
    let _font_paths = vec![
        "fonts/Inter-Regular.ttf", // We renamed .woff2 to .ttf in the curl command or should use what we downloaded?
                                   // The command was: curl -L -o src/assets/fonts/Inter-Regular.ttf ...
                                   // So it is saved as .ttf even if source was .woff2.
    ];

    let mut fonts = Vec::new();
    // We will embed the bytes directly.
    // Note: GPUI text system usually takes a collection of font data.

    // Actually, let's just embed one main font "Inter"
    // and map it to the "Inter" family name.

    let inter_bytes = include_bytes!("fonts/Inter-Regular.ttf");
    fonts.push(SharedString::from("Inter"));

    cx.text_system()
        .add_fonts(vec![inter_bytes.as_ref().into()])
        .unwrap();
}
