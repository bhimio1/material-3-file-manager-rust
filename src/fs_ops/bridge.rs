use crate::fs_ops::portal::start_portal_service;
use gpui::{App, AsyncApp};

pub struct Bridge;

impl Bridge {
    // This runs on the Main Thread initially
    pub fn init(cx: &mut App) {
        // Spawn the Portal Service in a background async task
        // let mut async_cx = cx.to_async(); // Unused
        cx.spawn(move |_cx: &mut AsyncApp| async move {
            println!("Starting XDG Portal Service...");
            if let Err(e) = start_portal_service().await {
                eprintln!("Failed to start Portal Service: {}", e);
            }
        })
        .detach();
    }
}
