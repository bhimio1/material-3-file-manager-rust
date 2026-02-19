with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

parts = content.split("let async_cx = cx.to_async();")

if len(parts) > 1:
    new_content = parts[0]
    for part in parts[1:]:
        # Find the end of the spawn block
        end_idx = part.find("}).detach();")
        if end_idx != -1:
            replacement = """cx.background_executor().spawn(async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                    }).detach();"""
            new_content += replacement + part[end_idx + len("}).detach();"):]
        else:
            new_content += "let async_cx = cx.to_async();" + part
else:
    new_content = content

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(new_content)
