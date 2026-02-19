import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

# 1. Remove the "broken" block (duplicate dead code)
# It looks like:
#                                             if is_image && thumbnail_path.is_none() {
#                                                 let path_for_task = item_path.clone();
#                                             }
# Regex to match this specific structure with any indentation
broken_pattern = r"\s+if is_image && thumbnail_path\.is_none\(\) \{\s+let path_for_task = item_path\.clone\(\);\s+\}"
content = re.sub(broken_pattern, "", content)

# 2. Fix the "AsyncFnOnce" error by capturing async context
# We look for the block containing `cx.spawn(move |mut cx|` inside the `if !is_pending` block
# and rewrite it.

# New inner logic
new_inner = """
                                                    let path_for_task = item_path.clone();
                                                    let entity_for_task = file_list_entity.clone();
                                                    let mut async_cx = cx.to_async();

                                                    // Mark as pending
                                                    file_list_entity.update(cx, |this, _cx| {
                                                        this.pending_thumbnails.insert(path_for_task.clone());
                                                    });

                                                    cx.spawn(move |_| async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                        let _ = entity_for_task.update(&mut async_cx, |this, cx| {
                                                            this.pending_thumbnails.remove(&path_for_task);
                                                            cx.notify();
                                                        });
                                                    }).detach();"""

# Old inner logic start (to identify where to replace)
old_start = r"let path_for_task = item_path.clone();\s+let entity_for_task = file_list_entity.clone();\s+// Mark as pending"

# We can't easily match the whole block with regex safely.
# But we can replace the lines from `let path_for_task` down to `}).detach();`?
# The structure is:
# if !is_pending {
#    <REPLACE THIS>
# }

# Let's find the `if !is_pending {` and replace everything until the matching `}`.
# But matching brace is hard.
# However, the code inside is fairly unique.
# I'll replace the text chunk.

old_chunk = """                                                    let path_for_task = item_path.clone();
                                                    let entity_for_task = file_list_entity.clone();

                                                    // Mark as pending
                                                    file_list_entity.update(cx, |this, _cx| {
                                                        this.pending_thumbnails.insert(path_for_task.clone());
                                                    });

                                                    cx.spawn(move |mut cx| async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                        let _ = entity_for_task.update(cx, |this, cx| {
                                                            this.pending_thumbnails.remove(&path_for_task);
                                                            cx.notify();
                                                        });
                                                    }).detach();"""

# Try replacing `mut cx` with `cx` just in case I ran that sed command?
old_chunk_variant = old_chunk.replace("move |mut cx|", "move |cx|")

content = content.replace(old_chunk, new_inner)
content = content.replace(old_chunk_variant, new_inner)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
