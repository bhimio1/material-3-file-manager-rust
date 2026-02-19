import sys

with open("src/ui_components/file_list.rs", "r") as f:
    lines = f.readlines()

new_lines = []
in_filter_block = False
filter_block_replaced = False
struct_field_added = False
new_init_added = False
import_added = False

for line in lines:
    # Add imports
    if not import_added and "use gpui::*;" in line:
        new_lines.append(line)
        new_lines.append("use std::collections::HashSet;\n")
        new_lines.append("use std::path::PathBuf;\n")
        import_added = True
        continue

    # Add struct field
    if not struct_field_added and "collapsed_categories: std::collections::HashSet<String>," in line:
        new_lines.append(line)
        new_lines.append("    pending_thumbnails: HashSet<PathBuf>,\n")
        struct_field_added = True
        continue

    # Initialize in new
    if not new_init_added and "collapsed_categories: std::collections::HashSet::new()," in line:
        new_lines.append(line)
        new_lines.append("            pending_thumbnails: HashSet::new(),\n")
        new_init_added = True
        continue

    # Replace filtering logic
    if "let matcher = SkimMatcherV2::default();" in line:
        in_filter_block = True
        continue

    if in_filter_block:
        if ".collect()" in line and "};" in line: # End of filter block (heuristic)
            # Actually, the block ends with "};" indented?
            pass

        if "let filtered_items: Vec<_> = if let Some(global_results)" in line:
             pass

        # We need to find the end of the block.
        # The block starts at  and ends at  closing
        # Let's just consume until we see ?
        if "let item_count = " in line:
            in_filter_block = False
            new_lines.append("        let filtered_items = ws.filtered_items.clone();\n")
            new_lines.append("\n")
            new_lines.append(line) # let item_count = ...
        continue

    # Capture entity handle before list_id definition
    if "let list_id = ElementId::Name" in line:
        new_lines.append("        let file_list_entity = cx.entity().clone();\n")
        new_lines.append(line)
        continue

    # Modify thumbnail logic inside uniform_list
    if "let thumbnail_path = if is_image {" in line:
        new_lines.append(line)
        continue

    # We want to replace the  block
    if "if is_image && thumbnail_path.is_none() {" in line:
        # We will skip the lines until the closing brace of this block
        # But wait, logic replacement is complex with line iteration.
        # I'll output my new logic and skip the old lines.
        new_lines.append(line) # Keep the if condition? No, I want to change logic.
        # Actually, let's just replace the whole inner part.

        # New Logic:
        # Check if already pending using file_list_entity.read(cx)
        # But wait, uniform_list closure has  as ?
        # Actually  signature:
        #  is .

        # Logic:
        # if is_image && thumbnail_path.is_none() {
        #    let is_pending = file_list_entity.read(cx).pending_thumbnails.contains(&item_path);
        #    if !is_pending {
        #        let path_for_task = item_path.clone();
        #        let entity_clone = file_list_entity.clone();
        #
        #        // Mark as pending immediately?
        #        // We can't mutate Sync in read. We need update.
        #        // But update inside render/layout might be tricky.
        #        // file_list_entity.update(cx, |this, _| this.pending_thumbnails.insert(path_for_task.clone()));
        #
        #        // Spawn
        #        cx.spawn(|cx| async move { ... })
        #    }
        # }

        # I'll just skip lines for now and insert my own block.
        continue

    # Skip the body of the old thumbnail block
    if "cx.background_executor().spawn(async move {" in line:
        continue
    if "crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task);" in line:
        continue
    if "}).detach();" in line:
        continue

    # Check for closing brace of the if block
    # It was  followed by ?
    if "let icon_name = if is_dir {" in line:
        # Insert new logic before this line
        new_lines.append("""                                            if is_image && thumbnail_path.is_none() {
                                                let is_pending = file_list_entity.read(cx).pending_thumbnails.contains(&item_path);
                                                if !is_pending {
                                                    let path_for_task = item_path.clone();
                                                    let entity_for_task = file_list_entity.clone();

                                                    // Mark as pending
                                                    file_list_entity.update(cx, |this, _cx| {
                                                        this.pending_thumbnails.insert(path_for_task.clone());
                                                    });

                                                    cx.spawn(move |mut cx| async move {
                                                        crate::assets::thumbnail_worker::ThumbnailWorker::generate_thumbnail(path_for_task.clone());
                                                        let _ = entity_for_task.update(&mut cx, |this, cx| {
                                                            this.pending_thumbnails.remove(&path_for_task);
                                                            cx.notify();
                                                        });
                                                    }).detach();
                                                }
                                            }
""")
        new_lines.append(line)
        continue

    # Special handling for closing braces of skipped block if I missed one?
    # The old block was:
    # if ... {
    #   let ...
    #   cx...spawn...
    # }
    # I skipped the content but need to skip the closing brace .
    # The line  usually follows immediately.
    # So if I see  I assume I'm done skipping.

    # Wait, the  for  is on a separate line usually.
    # My simple parser might fail if formatting is different.

    new_lines.append(line)

with open("src/ui_components/file_list.rs", "w") as f:
    f.writelines(new_lines)
