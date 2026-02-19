import re

with open("src/ui_components/file_list.rs", "r") as f:
    content = f.read()

search = r"let mut async_cx = cx.to_async\(\); cx.spawn\(move \|_\| async move \{"
replace = r"let async_cx = cx.to_async(); cx.spawn(move |_| async move { let mut async_cx = async_cx.clone();"

content = re.sub(search, replace, content)

with open("src/ui_components/file_list.rs", "w") as f:
    f.write(content)
