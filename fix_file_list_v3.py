import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

# Switch to background_executor
content = re.sub(r"cx\.spawn\(move \|_\| async move \{",
                 r"cx.background_executor().spawn(async move {", content)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
