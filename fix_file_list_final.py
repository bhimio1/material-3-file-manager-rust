import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

replacement = """cx.background_executor().spawn(async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                    }).detach();"""

content = re.sub(r"let async_cx = cx\.to_async\(\); cx\.spawn\(move \|_\| async move \{.*?\}\)\.detach\(\);", replacement, content, flags=re.DOTALL)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
