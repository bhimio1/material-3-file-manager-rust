import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

# Revert background_executor to spawn
content = re.sub(r"cx\.background_executor\(\)\.spawn\(async move \{",
                 r"cx.spawn(move |_| async move {", content)

# Change entity.update to async_cx.update_entity
# Pattern: `entity_for_task.update(&mut async_cx,`
# To: `async_cx.update_entity(&entity_for_task,`
content = re.sub(r"entity_for_task\.update\(&mut async_cx,",
                 r"async_cx.update_entity(&entity_for_task,", content)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
