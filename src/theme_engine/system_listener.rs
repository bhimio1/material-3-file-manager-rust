use gpui::*;

pub fn spawn(cx: &mut impl AppContext) {
    cx.background_spawn(async move {
        // Stub
    })
    .detach();
}
