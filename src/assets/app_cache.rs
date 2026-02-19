use gpui::{AppContext, AsyncApp, Context, Entity};

use crate::fs_ops::applications::{scan_applications, AppEntry};

pub struct AppCache {
    pub apps: Vec<AppEntry>,
    pub is_loaded: bool,
}

impl AppCache {
    pub fn new<T: AppContext>(cx: &mut T) -> Entity<Self> {
        cx.new(|cx| {
            let mut this = Self {
                apps: Vec::new(),
                is_loaded: false,
            };
            this.load(cx);
            this
        })
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let cx = cx.to_async();
        cx.spawn(move |cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let apps = cx
                    .background_executor()
                    .spawn(async move { scan_applications() })
                    .await;

                this.update(&mut cx, |this, cx| {
                    this.apps = apps;
                    this.is_loaded = true;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}
