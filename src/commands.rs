/// Commands supported by Omapdf.
pub const COMMANDS: &[&str] = &[
    "add_annot_to_highlight",
    "add_annot_to_selected_highlight",
    "add_bookmark",
    "add_highlight",
    "add_highlight_with_current_type",
    "clear_status_string",
    "close",
    "close_overview",
    "close_tab",
    "close_visual_mark",
    "close_window",
    "collapse_toc",
    "command",
    "context_menu",
    "copy",
    "copy_image",
    "copy_import_report",
    "copy_screenshot_to_clipboard",
    "cycle_layout",
    "delete_bookmark",
    "delete_highlight",
    "delete_portal",
    "edit_selected_highlight",
    "embed_annotations",
    "enter_visual_mark_mode",
    "execute_command",
    "expand_toc",
    "export",
    "export_image",
    "filter_settings",
    "fit_to_page",
    "fit_to_page_height",
    "fit_to_page_width",
    "fit_to_page_width_smart",
    "goto",
    "goto_begining",
    "goto_beginning",
    "goto_bookmark",
    "goto_bookmark_g",
    "goto_bottom_of_page",
    "goto_end",
    "goto_highlight",
    "goto_highlight_g",
    "goto_left",
    "goto_mark",
    "goto_next_highlight",
    "goto_next_highlight_of_type",
    "goto_overview",
    "goto_page_with_page_number",
    "goto_portal",
    "goto_prev_highlight",
    "goto_prev_highlight_of_type",
    "goto_right",
    "goto_selected_text",
    "goto_toc",
    "goto_top_of_page",
    "help",
    "import",
    "import_annotations",
    "import_report",
    "import_sqlite",
    "keys",
    "keys_user",
    "layout_horizontal",
    "layout_menu",
    "layout_single",
    "layout_spread",
    "layout_vertical",
    "move_down",
    "move_left",
    "move_right",
    "move_up",
    "move_visual_mark_down",
    "move_visual_mark_next",
    "move_visual_mark_prev",
    "move_visual_mark_up",
    "next_chapter",
    "next_item",
    "next_match",
    "next_page",
    "next_state",
    "next_tab",
    "open",
    "open_document",
    "open_path",
    "open_prev_doc",
    "overview",
    "overview_to_portal",
    "portal",
    "portal_to_definition",
    "portal_to_overview",
    "prefs",
    "prefs_user",
    "prev_chapter",
    "prev_state",
    "previous_item",
    "previous_match",
    "previous_page",
    "previous_tab",
    "q",
    "quit",
    "recent",
    "regex_search",
    "reload",
    "reload_config",
    "reset_preferences",
    "retry_imports",
    "screen_down",
    "screen_up",
    "search",
    "select_rect",
    "set_mark",
    "set_preference",
    "set_select_highlight_type",
    "set_status_string",
    "settings",
    "toggle_dark_mode",
    "toggle_fullscreen",
    "toggle_highlight",
    "toggle_smooth_scroll_mode",
    "toggle_statusbar",
    "toggle_visual_scroll",
    "toggle_window_configuration",
    "visual_mark_under_cursor",
    "zoom_in",
    "zoom_in_cursor",
    "zoom_out",
    "zoom_out_cursor",
];

/// Human-readable command help.
pub fn description(command: &str) -> &'static str {
    match command {
        "add_annot_to_highlight" => "Add or edit a note on the highlight containing the selection.",
        "add_annot_to_selected_highlight" => "Add or edit a note on the selected highlight.",
        "add_bookmark" => {
            "Add a bookmark to the current location at the document with the entered text."
        }
        "add_highlight" => {
            "Add the selected text as a highlight with the type of following symbol."
        }
        "add_highlight_with_current_type" => {
            "Add a highlight with the type specified using set_select_highlight_type."
        }
        "clear_status_string" => "Clear the message in Omapdf's statusbar.",
        "close" => "Dismiss the current dialog, menu or preview.",
        "close_overview" => "Close the overview window.",
        "close_tab" => "Close the active document tab.",
        "close_visual_mark" => "Exit the visual mark (ruler) mode.",
        "close_window" => "Close the current window.",
        "collapse_toc" => "Collapse every branch in the table of contents.",
        "command" => "Open a list of all Omapdf commands.",
        "context_menu" => "Open actions for the selected text or current page.",
        "copy" => "Copy the selected text into clipboard.",
        "copy_image" => "Copy the selected document area as a PNG image, or drag an area.",
        "copy_import_report" => "Copy the latest library import/export report to the clipboard.",
        "copy_screenshot_to_clipboard" => {
            "Copy a visible document region as an image, without reader controls."
        }
        "cycle_layout" => "Cycle through single page, spread and continuous page layouts.",
        "delete_bookmark" => "Delete the closest bookmark to the current location.",
        "delete_highlight" => "Delete the last highlight clicked on.",
        "delete_portal" => "Delete the closest portal to the current location.",
        "edit_selected_highlight" => "Edit the comment attached to the selected highlight.",
        "embed_annotations" => "Save a new PDF containing library highlights and bookmarks.",
        "enter_visual_mark_mode" => "Place a visual mark at the center of screen.",
        "execute_command" => "Search and run a reader command.",
        "expand_toc" => "Expand every branch in the table of contents.",
        "export" => "Export a lossless reader backup with legacy-compatible annotation data.",
        "export_image" => "Save the selected document area to a new PNG file, or drag an area.",
        "filter_settings" => "Search the supported reader preferences.",
        "fit_to_page" => "Fit the whole page inside the viewport.",
        "fit_to_page_height" => "Fit the current page to window width/height.",
        "fit_to_page_width" => "Fit the current page to window width/height.",
        "fit_to_page_width_smart" => "Fit the current page to the available width.",
        "goto" => "Jump to a one-based page number.",
        "goto_begining" => {
            "Go to the beginning of the document. If prefixed with a number, it will go to that page number."
        }
        "goto_beginning" => "Jump to the first page, or the numbered page prefix.",
        "goto_bookmark" => "Open bookmark list of current document.",
        "goto_bookmark_g" => "Open bookmark list of all documents.",
        "goto_bottom_of_page" => "Go to the left/right/top/bottom of the current page.",
        "goto_end" => {
            "Go to the end of the document. If prefixed with a number, it will go to that page number."
        }
        "goto_highlight" => "Open highlight list of current document.",
        "goto_highlight_g" => "Open highlight list of all documents.",
        "goto_left" => "Go to the left/right/top/bottom of the current page.",
        "goto_mark" => "Go to the mark previously set using set_mark.",
        "goto_next_highlight" => "Go to the next/previous highlight in the current document.",
        "goto_next_highlight_of_type" => {
            "Go to the next/previous highlight in the current document with following symbol's type."
        }
        "goto_overview" => {
            "Go to / create a portal to the location displayed in the overview window."
        }
        "goto_page_with_page_number" => "Go to the page number corresponding to entered text.",
        "goto_portal" => {
            "Go to the destination of the closest portal source to the current document location."
        }
        "goto_prev_highlight" => "Go to the next/previous highlight in the current document.",
        "goto_prev_highlight_of_type" => {
            "Go to the next/previous highlight in the current document with following symbol's type."
        }
        "goto_right" => "Go to the left/right/top/bottom of the current page.",
        "goto_selected_text" => "Jump to the location of current selected text.",
        "goto_toc" => "Open the table of contents.",
        "goto_top_of_page" => "Go to the left/right/top/bottom of the current page.",
        "help" => "Show keyboard shortcuts and reading commands.",
        "import" => "Merge a reader JSON backup or legacy JSON/SQLite library.",
        "import_annotations" => {
            "Import PDF highlights and text notes into the library without changing the PDF."
        }
        "import_report" => {
            "Show the latest library import/export report and retained-data warnings."
        }
        "import_sqlite" => "Migrate a combined or split legacy database read-only.",
        "keys" => "Open the keys.config / prefs.config file.",
        "keys_user" => "Open the keys_user.config / prefs_user.config file.",
        "layout_horizontal" => "Read a continuous horizontal strip of pages.",
        "layout_menu" => "Choose a page layout.",
        "layout_single" => "Display one page at a time.",
        "layout_spread" => "Display consecutive pages in pairs.",
        "layout_vertical" => "Read a continuous vertical strip of pages.",
        "move_down" => "Move the document.",
        "move_left" => "Move the document.",
        "move_right" => "Move the document.",
        "move_up" => "Move the document.",
        "move_visual_mark_down" => {
            "Move the visual mark up and down which highlights the previous/next line."
        }
        "move_visual_mark_next" => "Move the reading ruler to the next text line.",
        "move_visual_mark_prev" => "Move the reading ruler to the previous text line.",
        "move_visual_mark_up" => {
            "Move the visual mark up and down which highlights the previous/next line."
        }
        "next_chapter" => "Go to the next/previous chapter in the current document.",
        "next_item" => "Jump to the next document search result.",
        "next_match" => "Jump to the next document search result.",
        "next_page" => "Navigate to the next/previous page.",
        "next_state" => "Go forward/backward in history.",
        "next_tab" => "Switch to the next document tab.",
        "open" => "Open a PDF or EPUB path in another tab or window.",
        "open_document" => "Open the file browser to open a document.",
        "open_path" => "Enter the path of a document to open.",
        "open_prev_doc" => "Choose a recently opened document.",
        "overview" => "Preview a page without leaving the current location.",
        "overview_to_portal" => {
            "Open the overview window to the closest portal to current document location."
        }
        "portal" => {
            "Set the current location as the portal source. If a portal source is already set, then use the current location as the portal destination and create the portal."
        }
        "portal_to_definition" => "Create a portal by choosing its destination.",
        "portal_to_overview" => {
            "Go to / create a portal to the location displayed in the overview window."
        }
        "prefs" => "Open the keys.config / prefs.config file.",
        "prefs_user" => "Open the keys_user.config / prefs_user.config file.",
        "prev_chapter" => "Go to the next/previous chapter in the current document.",
        "prev_state" => "Go forward/backward in history.",
        "previous_item" => "Jump to the previous document search result.",
        "previous_match" => "Jump to the previous document search result.",
        "previous_page" => "Navigate to the next/previous page.",
        "previous_tab" => "Switch to the previous document tab.",
        "q" => "Quit Omapdf (closes all windows).",
        "quit" => "Quit Omapdf (closes all windows).",
        "recent" => "Choose a recently opened document.",
        "regex_search" => "Search for a regular expression in the document.",
        "reload" => "Reload the current document.",
        "reload_config" => "Reload preferences and keybindings from configuration files.",
        "reset_preferences" => "Reset supported preferences while preserving unrelated settings.",
        "retry_imports" => {
            "Resolve retained import records after missing or moved documents become available."
        }
        "screen_down" => "Move the screen up/down.",
        "screen_up" => "Move the screen up/down.",
        "search" => "Search for a text in document.",
        "select_rect" => "Drag a rectangle across visible document pages; Escape cancels.",
        "set_mark" => "Mark the location with the symbol entered following the set_mark command.",
        "set_preference" => "Set and save a supported reader preference.",
        "set_select_highlight_type" => "Set the highlight type used in select highlight mode.",
        "set_status_string" => "Set a message to be displayed in Omapdf's statusbar.",
        "settings" => "Edit reader preferences.",
        "toggle_dark_mode" => "Switch between light/dark colorschemes.",
        "toggle_fullscreen" => "Toggle fullscreen mode.",
        "toggle_highlight" => "Show or hide saved highlights in the reader.",
        "toggle_smooth_scroll_mode" => {
            "Toggle smooth scroll mode. In this mode, scrolling is done smoothly."
        }
        "toggle_statusbar" => "Toggle the statusbar at the bottom of the screen.",
        "toggle_visual_scroll" => {
            "Toggle visual scroll mode. In visual scroll mode, mouse wheel moves the ruler (the visual mark) instead of scrolling the document."
        }
        "toggle_window_configuration" => {
            "Toggle between one window/two window configuration settings."
        }
        "visual_mark_under_cursor" => "Place a visual mark (ruler) under the mouse cursor.",
        "zoom_in" => "Change the zoom level.",
        "zoom_in_cursor" => "Zoom in/out on the mouse cursor.",
        "zoom_out" => "Change the zoom level.",
        "zoom_out_cursor" => "Zoom in/out on the mouse cursor.",
        _ => "",
    }
}
