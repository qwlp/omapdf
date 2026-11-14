# Omapdf roadmap

These TODOs track reader features and behavioral gaps. They are not claims about
features already available. See [README](../README.md) for the current feature set
and [commands](COMMANDS.md) for supported actions.

## Reading and navigation

- [ ] Add clockwise/counterclockwise page rotation, with consistent layout, overlays, clipboard geometry and saved state.
- [ ] Add content-aware fit, margin cropping, width ratios and smart column navigation. The current smart-width alias performs ordinary width fitting.
- [ ] Expand reference indexing and definition-jump heuristics. The definition-portal alias currently uses manual destination selection.
- [ ] Support search matches across page boundaries and additional search scopes.
- [ ] Autoscroll text selections beyond visible page boundaries.
- [ ] Detect reading-ruler lines on image-only pages and add further ruler interaction modes.
- [ ] Add keyboard selection, keyboard smart jumps and keyboard overview navigation.
- [ ] Add preview history/navigation, overview-under-pointer actions and portal editing/history.
- [ ] Expand external-document and embedded-document link handling, selected-URL opening and external search.
- [ ] Add presentation/fast-reading modes, drag mode, horizontal-scroll locking and additional window controls.

## Annotations and documents

- [ ] Add drawing and rich free-text annotation editing.
- [ ] Import additional embedded PDF annotation types. Current interchange supports highlights, underline, strikeout and text/free-text notes.
- [ ] Add general PDF modification, printing and advanced metadata tools.
- [ ] Remap stored locations after PDF page insertion or removal.
- [ ] Render migrated rich bookmark drawing/font geometry; today it is archived and shown as a plain note.
- [ ] Add full reader-frame screenshots and scratchpad integration.
- [ ] Expand supported document formats and fixed-layout EPUB fidelity.
- [ ] Explore interactive EPUB, audio and touch/mobile reading.

## Configuration and interface

- [ ] Expand preference coverage, modal keymaps and macro arguments.
- [ ] Complete menu, helper-window positioning and helper interaction behavior.
- [ ] Add further window commands, preview commands and configuration actions.
- [ ] Improve prompt mouse cursor placement, shaped text selection and IME coverage.
- [ ] Improve accessibility and validate more compositors, GPUs and platforms beyond Linux Wayland.
- [ ] Refine tile scheduling and memory use for very large rasters.

SyncTeX and extension APIs are outside the current project scope.

## Additional command entry points

These command names still need their own dispatcher entry points. Some underlying
features (such as password prompts and opening windows) already exist through
other UI or command paths.

- [ ] `copy_window_size_config`
- [ ] `edit_portal`
- [ ] `enter_password`
- [ ] `external_search`
- [ ] `fit_to_page_height_smart`
- [ ] `fit_to_page_width_ratio`
- [ ] `focus_text`
- [ ] `goto_definition`
- [ ] `goto_left_smart`
- [ ] `goto_right_smart`
- [ ] `goto_window`
- [ ] `keyboard_overview`
- [ ] `keyboard_select`
- [ ] `keyboard_smart_jump`
- [ ] `keys_user_all`
- [ ] `new_window`
- [ ] `next_preview`
- [ ] `open_document_embedded`
- [ ] `open_document_embedded_from_current_path`
- [ ] `open_last_document`
- [ ] `open_link`
- [ ] `open_selected_url`
- [ ] `overview_definition`
- [ ] `overview_next_item`
- [ ] `overview_prev_item`
- [ ] `overview_under_cursor`
- [ ] `prefs_user_all`
- [ ] `previous_preview`
- [ ] `rotate_clockwise`
- [ ] `rotate_counterclockwise`
- [ ] `smart_jump_under_cursor`
- [ ] `toggle_custom_color`
- [ ] `toggle_fastread`
- [ ] `toggle_horizontal_scroll_lock`
- [ ] `toggle_mouse_drag_mode`
- [ ] `toggle_one_window`
- [ ] `toggle_presentation_mode`
- [ ] `toggle_scrollbar`
- [ ] `toggle_select_highlight`
- [ ] `toggle_titlebar`
