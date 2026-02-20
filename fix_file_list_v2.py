import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

# Remove broken block
content = re.sub(r"if is_image && thumbnail_path\.is_none\(\) \{\s+let path_for_task = item_path\.clone\(\);\s+\}", "", content)

# Replace `cx.spawn(move |mut cx| async move {`
content = re.sub(r"cx\.spawn\(move \|mut cx\| async move \{",
                 r"let mut async_cx = cx.to_async(); cx.spawn(move |_| async move {", content)

# Replace `cx.spawn(move |cx| async move {` (if I removed mut)
content = re.sub(r"cx\.spawn\(move \|cx\| async move \{",
                 r"let mut async_cx = cx.to_async(); cx.spawn(move |_| async move {", content)

# Replace `entity_for_task.update(cx,`
content = re.sub(r"entity_for_task\.update\(cx,",
                 r"entity_for_task.update(&mut async_cx,", content)

# Replace `entity_for_task.update(&mut cx,`
content = re.sub(r"entity_for_task\.update\(&mut cx,",
                 r"entity_for_task.update(&mut async_cx,", content)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
