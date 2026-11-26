use gpui::{prelude::*, *};
use omapdf::{
    appearance::Appearance,
    context_menu::{Action as MenuAction, Item as MenuItem, MenuState},
    document_theme::DocumentTheme,
    layout::{LayoutMode, PageLayout},
    raster::Rasterizer,
    scroll::ScrollMotion,
    settings::{COLORS, Kind, SETTINGS},
    text::TextBuffer,
    transition::Transition,
};
use omapdf::{
    config::{Binding, Config},
    document::Document,
    state::{Bookmark, Highlight, History, Library, Location, Portal},
};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy, PartialEq)]
enum Prompt {
    Command,
    Search,
    Page,
    Open,
    Bookmark,
    SetMark,
    JumpMark,
    Highlight,
    Filter,
    Preference,
    EmbedAnnotations,
    HighlightNote,
    ImportLibrary,
    ExportLibrary,
    Password,
    ExportImage,
}
#[derive(Clone, Copy, PartialEq)]
enum Panel {
    Recent,
    Bookmarks,
    Highlights,
    Search,
    Toc,
    Help,
    Commands,
    Settings,
    ImportReport,
}

struct PageJump {
    image: Arc<RenderImage>,
    bounds: Bounds<Pixels>,
    fade: Transition,
    created: std::time::Instant,
}

#[derive(Clone, Default)]
enum RegionAction {
    #[default]
    Select,
    Copy,
    Export(PathBuf),
}

struct UnlockRequest {
    path: PathBuf,
    page: Option<usize>,
    restore: Option<Location>,
    record: bool,
}

fn menu_keystroke(
    original: &Keystroke,
    popup: bool,
    prompt: Option<Prompt>,
    panel: Option<Panel>,
) -> Keystroke {
    let mut key = original.clone();
    if original.modifiers.alt || original.modifiers.platform {
        return key;
    }
    let list = original.modifiers.control
        && matches!(
            prompt,
            Some(Prompt::Command | Prompt::Filter | Prompt::Search)
        );
    let outline = list && prompt == Some(Prompt::Filter) && panel == Some(Panel::Toc);
    key.key = match original.key.as_str() {
        "j" if popup || list => "down",
        "k" if popup || list => "up",
        "h" if outline && !popup => "left",
        "l" if outline && !popup => "right",
        _ => return key,
    }
    .into();
    key.modifiers.control = false;
    key.key_char = None;
    key
}

pub struct Reader {
    focus: FocusHandle,
    window: AnyWindowHandle,
    config: Config,
    appearance: Appearance,
    prepared: Option<PreparedOpen>,
    editing_setting: Option<String>,
    editing_highlight: Option<(String, Highlight)>,
    settings_message: String,
    import_issues: Vec<String>,
    pending_file_prompt: bool,
    pending_unlock: Option<UnlockRequest>,
    cancelled_unlock: Option<(PathBuf, Option<omapdf::reload::Revision>)>,
    sequence: Vec<String>,
    startup_commands: Vec<String>,
    restore_target: Option<Location>,
    suppress_open_history: bool,
    pub opens: Option<omapdf::ipc::SignalSender<omapdf::ipc::Request>>,
    loading: bool,
    loading_path: Option<PathBuf>,
    fit_height: bool,
    dark: bool,
    library: Library,
    library_baseline: Library,
    library_path: PathBuf,
    document: Option<Arc<Document>>,
    page: usize,
    zoom: f32,
    fit: bool,
    image: Option<Arc<RenderImage>>,
    rasters_dirty: bool,
    rasterizer: Arc<Rasterizer>,
    motion: ScrollMotion,
    animation_time: Option<std::time::Instant>,
    menu_motions: [ScrollMotion; 5],
    menu_offsets: [Option<f32>; 5],
    menu_time: Option<std::time::Instant>,
    settings_scroll: ScrollHandle,
    completion_scroll: UniformListScrollHandle,
    completion_seed: Option<String>,
    completion_index: usize,
    completion_selected: bool,
    command_history: Vec<String>,
    status_since: Option<std::time::Instant>,
    toast_transition: Transition,
    animation_frame_pending: bool,
    ui_animating: bool,
    panel_transition: Transition,
    collapsed_headings: BTreeSet<usize>,
    panel_scroll: UniformListScrollHandle,
    menu_transition: Transition,
    layout_transition: Transition,
    preview_transition: Transition,
    presented_panel: Option<Panel>,
    presented_menu: Option<MenuState>,
    presented_preview: Option<(usize, Arc<RenderImage>)>,
    menu_opacity: f32,
    raster_images: BTreeMap<usize, (u32, Arc<RenderImage>)>,
    raster_tasks: BTreeMap<usize, (u32, Task<()>)>,
    text_tasks: BTreeMap<usize, Task<()>>,
    index_task: Option<Task<()>>,
    index_cancel: Arc<std::sync::atomic::AtomicBool>,
    search_jump_pending: bool,
    geometry: Option<PageLayout>,
    layout_cache: Option<(String, LayoutMode, usize, u32, u32, PageLayout)>,
    last_window_title: String,
    saved_viewport_hint: Option<(u32, u32)>,
    layout_menu: bool,
    layout_index: usize,
    tracking_scroll: bool,
    scrollbar_grab: Option<f32>,
    scroll_save_pending: bool,
    visual_mark: Option<omapdf::visual::Mark>,
    visual_scroll: bool,
    visual_task: Option<Task<()>>,
    visual_generation: u64,
    visual_cancel: Arc<std::sync::atomic::AtomicBool>,
    visual_pending: i64,
    selection_page: Option<usize>,
    selection_end_page: Option<usize>,
    region_mode: bool,
    region_drag: Option<([f32; 2], [f32; 2])>,
    region_selection: Option<(String, omapdf::region::Region)>,
    region_action: RegionAction,
    context_menu: Option<MenuState>,
    context_scroll: ScrollHandle,
    page_bounds: BTreeMap<usize, Bounds<Pixels>>,
    generation: u64,
    open_generation: u64,
    status: String,
    prompt: Option<Prompt>,
    input: TextBuffer,
    panel: Option<Panel>,
    panel_index: usize,
    panel_filter: String,
    panel_global: bool,
    query: String,
    regex_search: bool,
    search_case: bool,
    search_words: bool,
    search_busy: bool,
    annotation_busy: usize,
    search_committed: bool,
    search_error: Option<String>,
    search_generation: u64,
    search_task: Option<Task<()>>,
    search_scroll: UniformListScrollHandle,
    highlight_type: char,
    show_highlights: bool,
    show_statusbar: bool,
    matches: Vec<(usize, usize, usize)>,
    match_index: usize,
    selection: Option<(usize, usize)>,
    selecting: bool,
    count: String,
    display_width: f32,
    pending_offset: Option<[f32; 2]>,
    zoom_anchor: Option<omapdf::zoom::Anchor>,
    zoom_transition: Option<Transition>,
    page_jump: Option<PageJump>,
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
    canvas_inset: [f32; 2],
    history: History,
    portal_source: Option<Location>,
    preview_target: Option<Location>,
    preview_generation: u64,
    helper: Option<WindowHandle<crate::helper::Helper>>,
    preview: Option<(usize, Arc<RenderImage>)>,
    scroll: ScrollHandle,
}

pub struct Startup {
    pub library: Library,
    pub library_path: PathBuf,
    pub config: Config,
    pub rasterizer: Arc<Rasterizer>,
    pub file: Option<PathBuf>,
    pub page: Option<usize>,
    pub commands: Vec<String>,
    pub requests: omapdf::ipc::SignalReceiver<omapdf::ipc::Request>,
    pub prepared: Option<PreparedOpen>,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ViewportHint {
    width: f32,
    height: f32,
    scale: f32,
}

fn viewport_hint_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("omapdf/viewport-v1.json"))
}

fn load_viewport_hint() -> ViewportHint {
    viewport_hint_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<ViewportHint>(&bytes).ok())
        .filter(|hint| {
            hint.width.is_finite()
                && hint.height.is_finite()
                && hint.scale.is_finite()
                && hint.width >= 300.
                && hint.height >= 200.
                && (0.5..=4.0).contains(&hint.scale)
        })
        .unwrap_or(ViewportHint {
            width: 1200.,
            height: 900.,
            scale: 1.0,
        })
}

fn save_viewport_hint(hint: ViewportHint) {
    let Some(path) = viewport_hint_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(&hint) {
        let _ = std::fs::write(path, bytes);
    }
}

fn estimate_initial_raster_width(
    doc: &Document,
    page_index: usize,
    config: &Config,
    saved_zoom: f32,
    saved_fit: bool,
) -> u32 {
    let hint = load_viewport_hint();
    let (tab_w, tab_h) = if config.horizontal_tabs {
        (0., crate::workspace::TAB_BAR_HEIGHT)
    } else {
        (crate::workspace::TAB_SIDEBAR_WIDTH, 0.)
    };
    let viewport_width = (hint.width - tab_w).max(150.);
    let viewport_height = (hint.height - tab_h - 36.).max(100.);
    let padding = config.page_padding;
    let available = (viewport_width - padding * 2.).max(150.);
    let mode = config.layout;
    let columns = if mode == LayoutMode::Spread { 2. } else { 1. };
    let gap = 16.;
    let page_available = ((available - (columns - 1.) * gap) / columns).max(100.);
    let fit = if !config.restore_position {
        true
    } else {
        saved_fit || saved_zoom <= 0.
    };
    let zoom = if !config.restore_position || saved_zoom <= 0. {
        1.
    } else {
        saved_zoom.clamp(0.2, 5.)
    };
    let target_width = if mode == LayoutMode::Horizontal && fit {
        doc.pages
            .get(if mode.continuous() { 0 } else { page_index })
            .map(|page| {
                ((viewport_height - padding * 2.) * page.width / page.height)
                    .max(100.)
                    .min(page_available)
            })
            .unwrap_or(page_available)
    } else if fit {
        page_available.min(config.max_page_width / columns)
    } else {
        800. * zoom
    };
    let page_width = if mode == LayoutMode::Horizontal {
        let first = &doc.pages[0];
        let page = &doc.pages[page_index];
        let strip_height = target_width * first.height / first.width;
        strip_height * page.width / page.height
    } else {
        target_width
    };
    (page_width * hint.scale).round().clamp(64., 8192.) as u32
}

/// Start opening and pre-rasterizing the initial document while GPUI initializes its GPU.
pub struct PreparedOpen {
    path: PathBuf,
    worker: std::thread::JoinHandle<anyhow::Result<Arc<Document>>>,
}

impl PreparedOpen {
    pub fn new(
        path: PathBuf,
        page: Option<usize>,
        config: Config,
        library_path: PathBuf,
        rasterizer: Arc<Rasterizer>,
    ) -> Self {
        let source = path.clone();
        Self {
            path,
            worker: std::thread::spawn(move || {
                let doc = Arc::new(Document::open_preview(&source, config.epub)?);
                let library = Library::load(&library_path).unwrap_or_default();
                let saved = library.documents.get(&doc.id);
                let target_page = page
                    .unwrap_or(if config.restore_position {
                        saved.map_or(0, |state| state.page)
                    } else {
                        0
                    })
                    .min(doc.pages.len() - 1);
                let width = estimate_initial_raster_width(
                    &doc,
                    target_page,
                    &config,
                    saved.map_or(1., |state| state.zoom),
                    saved.is_none_or(|state| state.fit),
                );
                if config.document_theme {
                    let appearance = Appearance::load();
                    rasterizer.set_palette(Some(DocumentTheme::new(
                        appearance.background,
                        appearance.foreground,
                        appearance.accent,
                    )));
                }
                let _ = rasterizer.render(doc.clone(), target_page, width, config.dark);
                Ok(doc)
            }),
        }
    }
}

static LIBRARY_IO_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

impl Reader {
    pub fn new(startup: Startup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Startup {
            library,
            library_path,
            config,
            rasterizer,
            file,
            page,
            commands: startup_commands,
            requests,
            prepared,
        } = startup;
        let focus = cx.focus_handle();
        focus.focus(window);
        let config_highlights = config.highlights;
        let config_statusbar = config.statusbar;
        let appearance = Appearance::load();
        rasterizer.set_palette(config.document_theme.then_some(DocumentTheme::new(
            appearance.background,
            appearance.foreground,
            appearance.accent,
        )));
        rasterizer.set_contrast(config.dark_contrast);
        rasterizer.set_quality(config.render_scale);
        rasterizer.set_sharpness(config.render_sharpness);
        let mut motion = ScrollMotion::default();
        motion.set_response(config.scroll_response);
        let mut reader = Self {
            focus,
            window: window.window_handle(),
            dark: config.dark,
            appearance,
            prepared,
            editing_setting: None,
            editing_highlight: None,
            settings_message: String::new(),
            import_issues: Vec::new(),
            pending_file_prompt: false,
            pending_unlock: None,
            cancelled_unlock: None,
            config,
            sequence: vec![],
            startup_commands,
            restore_target: None,
            suppress_open_history: false,
            opens: None,
            loading: false,
            loading_path: None,
            fit_height: false,
            library_baseline: library.clone(),
            library,
            library_path,
            document: None,
            page: 0,
            zoom: 1.,
            fit: true,
            image: None,
            rasters_dirty: true,
            rasterizer,
            motion,
            animation_time: None,
            menu_motions: std::array::from_fn(|_| ScrollMotion::default()),
            menu_offsets: [None; 5],
            menu_time: None,
            settings_scroll: ScrollHandle::new(),
            completion_scroll: UniformListScrollHandle::new(),
            completion_seed: None,
            completion_index: 0,
            completion_selected: false,
            command_history: Vec::new(),
            status_since: None,
            toast_transition: Transition::default(),
            animation_frame_pending: false,
            ui_animating: false,
            panel_transition: Transition::default(),
            collapsed_headings: BTreeSet::new(),
            panel_scroll: UniformListScrollHandle::new(),
            menu_transition: Transition::default(),
            layout_transition: Transition::default(),
            preview_transition: Transition::default(),
            presented_panel: None,
            presented_menu: None,
            presented_preview: None,
            menu_opacity: 0.,
            raster_images: BTreeMap::new(),
            raster_tasks: BTreeMap::new(),
            text_tasks: BTreeMap::new(),
            index_task: None,
            index_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            search_jump_pending: false,
            geometry: None,
            layout_cache: None,
            last_window_title: String::new(),
            saved_viewport_hint: None,
            layout_menu: false,
            layout_index: 0,
            tracking_scroll: false,
            scrollbar_grab: None,
            scroll_save_pending: false,
            visual_mark: None,
            visual_scroll: false,
            visual_task: None,
            visual_generation: 0,
            visual_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            visual_pending: 0,
            selection_page: None,
            selection_end_page: None,
            region_mode: false,
            region_drag: None,
            region_selection: None,
            region_action: RegionAction::Select,
            context_menu: None,
            context_scroll: ScrollHandle::new(),
            page_bounds: BTreeMap::new(),
            generation: 0,
            open_generation: 0,
            status: "Open a PDF or EPUB with Ctrl+O".into(),
            prompt: None,
            input: TextBuffer::default(),
            panel: None,
            panel_index: 0,
            panel_filter: String::new(),
            panel_global: false,
            query: String::new(),
            regex_search: false,
            search_case: false,
            search_words: false,
            search_busy: false,
            annotation_busy: 0,
            search_committed: false,
            search_error: None,
            search_generation: 0,
            search_task: None,
            search_scroll: UniformListScrollHandle::new(),
            highlight_type: 'a',
            show_highlights: config_highlights,
            show_statusbar: config_statusbar,
            matches: vec![],
            match_index: 0,
            selection: None,
            selecting: false,
            count: String::new(),
            display_width: 1.,
            pending_offset: None,
            zoom_anchor: None,
            zoom_transition: None,
            page_jump: None,
            viewport_origin: [0., 36.],
            viewport_size: [0., 0.],
            canvas_inset: [0., 0.],
            history: History::default(),
            portal_source: None,
            preview_target: None,
            preview_generation: 0,
            helper: None,
            preview: None,
            scroll: ScrollHandle::new(),
        };
        if let Some(path) = file {
            reader.open(path, page, cx);
        }
        cx.on_release(|this, cx| {
            this.raster_tasks.clear();
            if let Some(helper) = this.helper.take() {
                let _ = helper.update(cx, |_, window, _| window.remove_window());
            }
        })
        .detach();
        let ipc_window = window.window_handle();
        cx.spawn(async move |this, cx| {
            while let Some(request) = requests.recv_async().await {
                let mut batch = vec![request];
                batch.extend(requests.try_iter());
                if ipc_window
                    .update(cx, |_, window, cx| {
                        this.update(cx, |this, cx| {
                            for request in batch {
                                if let Some(path) = request.file {
                                    this.open(path, request.page, cx);
                                } else if let Some(page) = request.page {
                                    this.goto(page, true, cx);
                                }
                                this.startup_commands.extend(request.commands);
                            }
                            this.run_startup_commands(window, cx);
                        })
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let executor = cx.background_executor().clone();
        let reader_window = window.window_handle();
        let initial_config_stamp = reader.config.directory.as_deref().map(Config::stamp);
        cx.spawn(async move |this, cx| {
            let mut tick = 0u32;
            let mut config_stamp = initial_config_stamp;
            let mut pending_config: Option<Config> = None;
            let mut config_error: Option<String> = None;
            let mut document_watch: Option<(
                (String, omapdf::reload::Revision),
                omapdf::reload::Watch,
            )> = None;
            let mut last_reload_attempt: Option<std::time::Instant> = None;
            loop {
                executor.timer(std::time::Duration::from_millis(100)).await;
                let Ok(Ok(())) = reader_window.update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        if this
                            .status_since
                            .is_some_and(|since| since.elapsed().as_secs_f32() >= 3.)
                        {
                            this.status_since = None;
                            cx.notify();
                        }
                        if this.scroll_save_pending
                            && this.motion.target.is_none()
                            && this.zoom_transition.is_none()
                        {
                            this.scroll_save_pending = false;
                            this.persist_async(cx);
                        }
                        // Dispatch independently of paint: hidden Wayland windows may not
                        // receive frames, but settings and IPC commands must still execute.
                        this.run_startup_commands(window, cx);
                        if this.search_busy {
                            cx.notify();
                        }
                    })
                }) else {
                    break;
                };
                tick = tick.wrapping_add(1);
                if !tick.is_multiple_of(5) {
                    continue;
                }
                let Ok((appearance, directory)) = this.update(cx, |this, _| {
                    (this.appearance.clone(), this.config.directory.clone())
                }) else {
                    break;
                };
                let previous_stamp = config_stamp;
                let (appearance, next_stamp, shared_preferences) = executor
                    .spawn(async move {
                        let next_stamp = directory.as_deref().map(Config::stamp);
                        let preferences = if next_stamp != previous_stamp {
                            directory
                                .map(|directory| Config::load(&directory))
                                .transpose()
                        } else {
                            Ok(None)
                        };
                        (appearance.refresh(false), next_stamp, preferences)
                    })
                    .await;
                let (shared_preferences, new_error) = match shared_preferences {
                    Ok(Some(shared)) if pending_config.as_ref() == Some(&shared) => {
                        config_stamp = next_stamp;
                        config_error = None;
                        (Some(shared), None)
                    }
                    Ok(Some(shared)) => {
                        pending_config = Some(shared);
                        config_error = None;
                        (None, None)
                    }
                    Ok(None) => {
                        if pending_config.is_some() {
                            config_stamp = next_stamp;
                        }
                        (pending_config.take(), None)
                    }
                    Err(error) => {
                        pending_config = None;
                        let message = format!("Live configuration reload: {error:#}");
                        let changed = config_error.as_ref() != Some(&message);
                        config_error = Some(message.clone());
                        (None, changed.then_some(message))
                    }
                };
                let _ = this.update(cx, |this, cx| {
                    if let Some(error) = new_error {
                        this.set_status(error);
                        cx.notify();
                    }
                    if this.appearance != appearance {
                        let palette_changed = this.appearance.background != appearance.background
                            || this.appearance.foreground != appearance.foreground
                            || this.appearance.accent != appearance.accent;
                        this.appearance = appearance;
                        if this.config.document_theme && palette_changed {
                            this.rasterizer.set_palette(Some(DocumentTheme::new(
                                this.appearance.background,
                                this.appearance.foreground,
                                this.appearance.accent,
                            )));
                            this.raster_tasks.clear();
                            this.rasters_dirty = true;
                            this.page_jump = None;
                        }
                        if let Some(helper) = this.helper {
                            let theme = this.appearance.clone();
                            let _ = helper.update(cx, |helper, _, cx| {
                                helper.appearance = theme;
                                cx.notify();
                            });
                        }
                        cx.notify();
                    }
                    if let Some(shared) = shared_preferences
                        && this.config != shared
                    {
                        let old = this.config.clone();
                        let (dark, highlights, statusbar) =
                            (this.dark, this.show_highlights, this.show_statusbar);
                        this.config = shared;
                        this.sequence.clear();
                        this.count.clear();
                        this.apply_loaded_config(cx);
                        if old.dark == this.config.dark {
                            this.dark = dark;
                        }
                        if old.highlights == this.config.highlights {
                            this.show_highlights = highlights;
                        }
                        if old.statusbar == this.config.statusbar {
                            this.show_statusbar = statusbar;
                        }
                    }
                });
                let Ok(snapshot) = this.update(cx, |this, _| {
                    if this.loading || this.pending_unlock.is_some() || this.region_mode {
                        None
                    } else {
                        this.document.clone()
                    }
                }) else {
                    break;
                };
                let Some(doc) = snapshot else {
                    continue;
                };
                let path = doc.path.clone();
                let changed = executor
                    .spawn(async move { omapdf::reload::Revision::read(&path).ok() })
                    .await;
                if document_watch
                    .as_ref()
                    .is_none_or(|((id, revision), _)| id != &doc.id || *revision != doc.revision)
                {
                    document_watch = Some((
                        (doc.id.clone(), doc.revision),
                        omapdf::reload::Watch::new(doc.revision),
                    ));
                    last_reload_attempt = None;
                }
                if document_watch.as_mut().unwrap().1.observe(changed)
                    && last_reload_attempt.is_none_or(|last| last.elapsed().as_secs_f32() >= 2.)
                {
                    last_reload_attempt = Some(std::time::Instant::now());
                    let _ =
                        this.update(cx, |this, cx| {
                            if !this.loading
                                && !this.cancelled_unlock.as_ref().is_some_and(
                                    |(path, revision)| *path == doc.path && *revision == changed,
                                )
                                && this
                                    .document
                                    .as_ref()
                                    .is_some_and(|current| Arc::ptr_eq(current, &doc))
                            {
                                this.suppress_open_history = true;
                                this.restore_target = this.location();
                                this.open(doc.path.clone(), Some(this.page), cx);
                            }
                        });
                }
            }
        })
        .detach();
        cx.defer_in(window, |this, window, cx| {
            this.run_startup_commands(window, cx)
        });
        reader
    }

    fn run_startup_commands(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut ran = false;
        while !self.loading
            && self.pending_unlock.is_none()
            && !self.region_mode
            && self.prompt != Some(Prompt::Open)
            && !self.search_busy
            && self.annotation_busy == 0
            && !self.startup_commands.is_empty()
        {
            let command = self.startup_commands.remove(0);
            self.execute(&command, window, cx);
            ran = true;
        }
        if ran && self.startup_commands.is_empty() && self.library != self.library_baseline {
            self.persist_async(cx);
        }
    }

    fn persist(&mut self) -> bool {
        self.library.normalize_ids();
        let offset = self.local_offset();
        if let Some(doc) = &self.document {
            let state = self.library.documents.entry(doc.id.clone()).or_default();
            state.page = self.page;
            state.zoom = self.zoom;
            state.fit = self.fit;
            state.offset = offset;
        }
        let _io = LIBRARY_IO_LOCK.lock();
        if let Ok(latest) = Library::load(&self.library_path) {
            self.library =
                omapdf::state::merge_library(&self.library_baseline, &self.library, latest);
        }
        self.library.prune_deleted();
        match self.library.save(&self.library_path) {
            Ok(()) => {
                self.library_baseline = self.library.clone();
                true
            }
            Err(error) => {
                self.set_status(format!("Cannot save library: {error:#}"));
                false
            }
        }
    }

    fn persist_async(&mut self, cx: &mut Context<Self>) {
        self.library.normalize_ids();
        let offset = self.local_offset();
        if let Some(doc) = &self.document {
            let state = self.library.documents.entry(doc.id.clone()).or_default();
            state.page = self.page;
            state.zoom = self.zoom;
            state.fit = self.fit;
            state.offset = offset;
        }
        if self.library == self.library_baseline && self.library_path.exists() {
            return;
        }
        let baseline = self.library_baseline.clone();
        let edited = self.library.clone();
        let path = self.library_path.clone();
        let task = cx.background_executor().spawn(async move {
            let _io = LIBRARY_IO_LOCK.lock();
            let mut merged = match Library::load(&path) {
                Ok(latest) => omapdf::state::merge_library(&baseline, &edited, latest),
                Err(_) => edited,
            };
            merged.prune_deleted();
            merged.save(&path)?;
            Ok::<Library, anyhow::Error>(merged)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(saved) => {
                    this.library_baseline = saved;
                }
                Err(error) => {
                    this.set_status(format!("Cannot save library: {error:#}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn location(&self) -> Option<Location> {
        self.document.as_ref().map(|doc| Location {
            document: doc.id.clone(),
            page: self.page,
            zoom: self.zoom,
            fit: self.fit,
            offset: self.local_offset(),
        })
    }

    fn open(&mut self, path: PathBuf, page: Option<usize>, cx: &mut Context<Self>) {
        self.open_password(path, page, None, cx);
    }

    fn open_password(
        &mut self,
        path: PathBuf,
        page: Option<usize>,
        password: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.pending_unlock.take().is_some() {
            self.startup_commands.clear();
        }
        self.cancelled_unlock = None;
        self.region_mode = false;
        self.region_drag = None;
        self.region_selection = None;
        if self.prompt == Some(Prompt::Password) {
            self.prompt = None;
            self.input = TextBuffer::default();
        }
        if self.document.is_some() || self.library != self.library_baseline {
            self.persist_async(cx);
        }
        self.close_visual_mark();
        self.close_preview();
        self.collapsed_headings.clear();
        self.page_jump = None;
        self.zoom_transition = None;
        self.search_generation += 1;
        self.search_task = None;
        self.search_busy = false;
        self.search_error = None;
        self.loading = true;
        self.loading_path = Some(path.clone());
        self.context_menu = None;
        self.zoom_anchor = None;
        self.page_bounds.clear();
        self.text_tasks.clear();
        self.index_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.index_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.index_task = None;
        self.motion.cancel();
        self.raster_tasks.clear();
        self.open_generation += 1;
        self.generation += 1;
        let generation = self.open_generation;
        let record = !std::mem::take(&mut self.suppress_open_history);
        let restore = self.restore_target.take();
        self.set_status(format!("Opening {}…", path.display()));
        let options = self.config.epub;
        let pending_path = path.clone();
        let prepared = self
            .prepared
            .take()
            .filter(|prepared| prepared.path == path && password.is_none());
        let previous = self.document.clone();
        // Fast path: if the initial document opened & pre-rasterized while GPUI
        // initialized Vulkan, install it immediately so Frame 1 paints the page
        // directly without an empty "Opening..." frame or async round-trip.
        if previous.is_none()
            && let Some(prepared) = prepared
        {
            if prepared.worker.is_finished() {
                let immediate = prepared
                    .worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("Initial document worker failed"))
                    .and_then(|res| res);
                match immediate {
                    Ok(doc)
                        if omapdf::reload::Revision::read(&path).ok() == Some(doc.revision)
                            && doc.epub_options == options =>
                    {
                        self.finish_open(Ok(doc), pending_path, page, restore, record, cx);
                        cx.notify();
                        return;
                    }
                    Err(error) => {
                        self.finish_open(Err(error), pending_path, page, restore, record, cx);
                        cx.notify();
                        return;
                    }
                    Ok(_) => {}
                }
            } else {
                let task = cx.background_executor().spawn(async move {
                    let result = prepared
                        .worker
                        .join()
                        .map_err(|_| anyhow::anyhow!("Initial document worker failed"))?;
                    match result {
                        Ok(doc)
                            if omapdf::reload::Revision::read(&path).ok() == Some(doc.revision)
                                && doc.epub_options == options =>
                        {
                            Ok(doc)
                        }
                        Ok(_) => Ok(Arc::new(Document::open_preview(&path, options)?)),
                        Err(error) => Err(error),
                    }
                });
                let reader_window = self.window;
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        if this.open_generation != generation {
                            return;
                        }
                        this.finish_open(result, pending_path, page, restore, record, cx);
                        cx.notify();
                    });
                    let _ = reader_window.update(cx, |_, window, cx| {
                        this.update(cx, |this, cx| {
                            if this.open_generation == generation {
                                this.run_startup_commands(window, cx);
                            }
                        })
                    });
                })
                .detach();
                cx.notify();
                return;
            }
        }
        let task = cx.background_executor().spawn(async move {
            let document = match password {
                Some(password) => Document::unlock_preview(&path, options, password),
                None => Document::open_preview(&path, options),
            }?;
            // Reflow migration needs text anchors from both versions. Ordinary
            // EPUB opens remain lazy; perform migration indexing off the UI thread.
            if let Some(previous) = previous
                && previous.path == document.path
                && previous.id != document.id
                && previous.reflowable
                && document.reflowable
            {
                previous.ensure_all_text()?;
                document.ensure_all_text()?;
            }
            Ok::<_, anyhow::Error>(Arc::new(document))
        });
        let reader_window = self.window;
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.open_generation != generation {
                    return;
                }
                this.finish_open(result, pending_path, page, restore, record, cx);
                cx.notify();
            });
            // Loading completion is an event: initial commands need not wait
            // for the periodic IPC/settings housekeeping timer.
            let _ = reader_window.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    if this.open_generation == generation {
                        this.run_startup_commands(window, cx);
                    }
                })
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_open(
        &mut self,
        result: anyhow::Result<Arc<Document>>,
        pending_path: PathBuf,
        page: Option<usize>,
        restore: Option<Location>,
        record: bool,
        cx: &mut Context<Self>,
    ) {
        self.loading = false;
        self.loading_path = None;
        match result {
            Ok(doc) => {
                if record && let Some(location) = self.location() {
                    self.history.push(location);
                }
                if let Some(previous) = &self.document
                    && previous.path == doc.path
                    && previous.id != doc.id
                {
                    if let Some(mut state) = self.library.documents.get(&previous.id).cloned() {
                        let reflow = previous.reflowable && doc.reflowable;
                        if reflow {
                            let at = Location {
                                document: previous.id.clone(),
                                page: state.page,
                                zoom: state.zoom,
                                fit: state.fit,
                                offset: state.offset,
                            };
                            let mapped = doc.remap_location(previous, &at);
                            state.page = mapped.page;
                            state.offset = mapped.offset;
                            state.highlights = state
                                .highlights
                                .iter()
                                .filter_map(|highlight| {
                                    let (page, start, end) = doc.find_text_near(
                                        &highlight.text,
                                        highlight.page as f32 / previous.pages.len() as f32,
                                    )?;
                                    let mut mapped = highlight.clone();
                                    mapped.page = page;
                                    mapped.rects = doc.pages[page].words[start..end]
                                        .iter()
                                        .map(|word| word.rect)
                                        .collect();
                                    Some(mapped)
                                })
                                .collect();
                        }
                        for mark in state.marks.values_mut() {
                            if mark.document == previous.id {
                                if reflow {
                                    *mark = doc.remap_location(previous, mark);
                                } else {
                                    mark.document = doc.id.clone();
                                }
                            }
                        }
                        for portal in &mut state.portals {
                            if reflow {
                                let source = doc.remap_location(
                                    previous,
                                    &Location {
                                        document: previous.id.clone(),
                                        page: portal.source_page,
                                        offset: portal.source_offset.unwrap_or_default(),
                                        ..Default::default()
                                    },
                                );
                                portal.source_page = source.page;
                                portal.source_offset = Some(source.offset);
                            }
                            if portal.destination.document == previous.id {
                                if reflow {
                                    portal.destination =
                                        doc.remap_location(previous, &portal.destination);
                                } else {
                                    portal.destination.document = doc.id.clone();
                                }
                            }
                        }
                        self.library.documents.insert(doc.id.clone(), state);
                    }
                    for mark in self.library.global_marks.values_mut() {
                        if mark.document == previous.id {
                            if previous.reflowable && doc.reflowable {
                                *mark = doc.remap_location(previous, mark);
                            } else {
                                mark.document = doc.id.clone();
                            }
                        }
                    }
                    for bookmark in &mut self.library.bookmarks {
                        if bookmark.location.document == previous.id {
                            if previous.reflowable && doc.reflowable {
                                bookmark.location =
                                    doc.remap_location(previous, &bookmark.location);
                            } else {
                                bookmark.location.document = doc.id.clone();
                            }
                        }
                    }
                    for state in self.library.documents.values_mut() {
                        for portal in &mut state.portals {
                            if portal.destination.document == previous.id {
                                if previous.reflowable && doc.reflowable {
                                    portal.destination =
                                        doc.remap_location(previous, &portal.destination);
                                } else {
                                    portal.destination.document = doc.id.clone();
                                    portal.destination.page =
                                        portal.destination.page.min(doc.pages.len() - 1);
                                }
                            }
                        }
                    }
                    self.library.recent.retain(|id| id != &previous.id);
                }
                self.library.touch(&doc.id, &doc.path);
                let saved = &self.library.documents[&doc.id];
                self.page = page
                    .unwrap_or(if self.config.restore_position {
                        saved.page
                    } else {
                        0
                    })
                    .min(doc.pages.len() - 1);
                self.zoom = if saved.zoom > 0. {
                    saved.zoom.clamp(0.2, 5.)
                } else {
                    1.
                };
                self.fit = saved.fit || saved.zoom <= 0.;
                if !self.config.restore_position && page.is_none() {
                    self.zoom = 1.;
                    self.fit = true;
                }
                self.pending_offset = if page.is_none() && self.config.restore_position {
                    Some(saved.offset)
                } else {
                    Some([0., 0.])
                };
                if let Some(location) = restore {
                    let location = self
                        .document
                        .as_ref()
                        .filter(|previous| previous.reflowable && doc.reflowable)
                        .map(|previous| doc.remap_location(previous, &location))
                        .unwrap_or(location);
                    self.page = location.page.min(doc.pages.len() - 1);
                    self.zoom = location.zoom.clamp(0.2, 5.);
                    self.fit = location.fit;
                    self.pending_offset = Some(location.offset);
                }
                self.collapsed_headings = omapdf::outline::collapsed_branches(&doc.outline);
                self.document = Some(doc);
                self.prepare_text_index(cx);
                self.selection = None;
                self.matches.clear();
                self.query.clear();
                self.preview = None;

                self.image = None;
                self.rasters_dirty = true;
                self.geometry = None;
                self.layout_cache = None;
                self.selection_page = None;
                self.selection_end_page = None;
                self.set_status("Ready".into());
                if self.startup_commands.is_empty() {
                    self.persist_async(cx);
                }
            }
            Err(error) => {
                if let Some(required) = error.downcast_ref::<omapdf::document::PasswordRequired>() {
                    self.pending_unlock = Some(UnlockRequest {
                        path: pending_path,
                        page,
                        restore,
                        record,
                    });
                    self.panel = None;
                    self.layout_menu = false;
                    self.prompt(Prompt::Password, cx);
                    self.set_status(required.to_string());
                } else {
                    self.set_status(format!("{error:#}"));
                }
                self.rasters_dirty = true;
                self.prepare_text_index(cx);
            }
        }
    }

    pub fn receive_request(&mut self, request: omapdf::ipc::Request, cx: &mut Context<Self>) {
        if let Some(path) = request.file {
            self.open(path, request.page, cx);
        } else if let Some(page) = request.page {
            self.goto(page, true, cx);
        }
        self.startup_commands.extend(request.commands);
    }

    pub fn horizontal_tabs(&self) -> bool {
        self.config.horizontal_tabs
    }

    fn tab_insets(&self) -> Size<Pixels> {
        if self.horizontal_tabs() {
            size(px(0.), px(crate::workspace::TAB_BAR_HEIGHT))
        } else {
            size(px(crate::workspace::TAB_SIDEBAR_WIDTH), px(0.))
        }
    }

    fn reader_viewport(&self, window: &Window) -> Size<Pixels> {
        let size = window.viewport_size();
        let insets = self.tab_insets();
        gpui::size(size.width - insets.width, size.height - insets.height)
    }

    pub fn ui_appearance(&self) -> (&Appearance, f32) {
        (&self.appearance, self.config.font_size)
    }

    pub fn is_empty(&self) -> bool {
        self.document.is_none() && !self.loading && self.pending_unlock.is_none()
    }

    pub fn tab_path(&self) -> Option<&PathBuf> {
        self.loading_path
            .as_ref()
            .or_else(|| self.pending_unlock.as_ref().map(|request| &request.path))
            .or_else(|| self.document.as_ref().map(|document| &document.path))
    }

    pub fn tab_title(&self) -> String {
        self.tab_path()
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| "New document".into())
    }

    pub fn focus_tab(&self, window: &mut Window) {
        self.focus.focus(window);
    }

    fn open_additional(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(opens) = &self.opens {
            let _ = opens.send(omapdf::ipc::Request {
                file: Some(path),
                ..Default::default()
            });
        } else {
            self.open(path, None, cx);
        }
    }

    /// Carry the continuation to the destination reader, including a new window.
    fn open_ordered(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.persist();
        if let Some(opens) = &self.opens {
            let request = omapdf::ipc::Request {
                file: Some(path),
                commands: std::mem::take(&mut self.startup_commands),
                ..Default::default()
            };
            if opens.send(request).is_err() {
                self.set_status("Cannot open the destination reader".into());
            }
        } else {
            self.open(path, None, cx);
        }
    }

    fn picker(&mut self, cx: &mut Context<Self>) {
        self.annotation_busy += 1;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open PDF or EPUB".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = paths.await;
            let _ = this.update(cx, |this, cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                match result {
                    Ok(Ok(Some(mut paths))) if !paths.is_empty() => {
                        let final_path = paths.pop().unwrap();
                        for path in paths {
                            this.open_additional(path, cx);
                        }
                        this.open_ordered(final_path, cx);
                    }
                    Ok(Err(error)) => {
                        this.prompt(Prompt::Open, cx);
                        this.set_status(format!("File chooser: {error}. Enter a document path"));
                    }
                    _ => {
                        this.startup_commands.clear();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prepare_text_index(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        if doc.indexed_pages() == doc.pages.len() {
            return;
        }
        let cancel = self.index_cancel.clone();
        let executor = cx.background_executor().clone();
        let id = doc.id.clone();
        self.index_task = Some(cx.spawn(async move |this, cx| {
            // Let the visible page render before walking the remaining document.
            executor.timer(std::time::Duration::from_millis(500)).await;
            let result = executor
                .spawn(async move { doc.ensure_all_text_until(&cancel) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.document.as_ref().is_none_or(|doc| doc.id != id) {
                    return;
                }
                this.index_task = None;
                if let Err(error) = result {
                    this.set_status(format!("Some text could not be indexed: {error}"));
                }
                cx.notify();
            });
        }));
    }

    fn request_page_text(&mut self, page: usize, cx: &mut Context<Self>) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        if doc.text_ready(page) || self.text_tasks.contains_key(&page) {
            return;
        }
        let document_id = doc.id.clone();
        let task = cx
            .background_executor()
            .spawn(async move { doc.ensure_page_text(page) });
        self.text_tasks.insert(
            page,
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if this
                        .document
                        .as_ref()
                        .is_none_or(|doc| doc.id != document_id)
                    {
                        return;
                    }
                    this.text_tasks.remove(&page);
                    if let Err(error) = result {
                        this.set_status(format!("Page text unavailable: {error}"));
                    }
                    cx.notify();
                });
            }),
        );
    }

    fn rasterize_page(&mut self, page: usize, width: u32, cx: &mut Context<Self>) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        if self
            .raster_images
            .get(&page)
            .is_some_and(|(w, _)| *w == width)
        {
            return;
        }
        if let Some(image) = self.rasterizer.cached(&doc, page, width, self.dark) {
            self.raster_tasks.remove(&page);
            self.raster_images.insert(page, (width, image));
            return;
        }
        let mut had_image = self.raster_images.contains_key(&page);
        let mut seeded_fallback = false;
        if !had_image
            && let Some((cached_width, image)) =
                self.rasterizer.cached_nearest(&doc, page, width, self.dark)
        {
            // Show pre-rasterized or previous-width page immediately on Frame 1
            // while upgrading to the exact device-pixel width without delay.
            self.raster_images.insert(page, (cached_width, image));
            had_image = true;
            seeded_fallback = true;
        }
        if self
            .raster_tasks
            .get(&page)
            .is_some_and(|(w, _)| *w == width)
        {
            return;
        }
        self.raster_tasks.remove(&page);
        let generation = self.generation;
        let rasterizer = self.rasterizer.clone();
        let dark = self.dark;
        let executor = cx.background_executor().clone();
        // Debounce only when resizing/zooming an already-visible page, never
        // for a newly scrolled-in page or an initial pre-rasterized upgrade.
        let debounce = had_image && !seeded_fallback;
        let task = cx.spawn(async move |this, cx| {
            // Keep the existing image during resize/zoom bursts; only the latest
            // settled size should enter the native renderer's work queue.
            if debounce {
                executor.timer(std::time::Duration::from_millis(24)).await;
            }
            let result = executor
                .spawn(async move { rasterizer.render(doc, page, width, dark) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation
                    || this
                        .raster_tasks
                        .get(&page)
                        .is_none_or(|(w, _)| *w != width)
                {
                    return;
                }
                this.raster_tasks.remove(&page);
                match result {
                    Ok(image) => {
                        this.raster_images.insert(page, (width, image));
                    }
                    Err(error) => this.set_status(format!("Rendering failed: {error:#}")),
                }
                cx.notify();
            });
        });
        self.raster_tasks.insert(page, (width, task));
    }

    fn local_offset(&self) -> [f32; 2] {
        if let Some(offset) = self.pending_offset {
            return offset;
        }
        let origin = self
            .geometry
            .as_ref()
            .map(|layout| layout.origin(self.page))
            .unwrap_or_default();
        [
            (f32::from(self.scroll.offset().x) + origin[0]) / self.display_width,
            (f32::from(self.scroll.offset().y) + origin[1]) / self.display_width,
        ]
    }

    fn begin_page_jump(&mut self) {
        if !self.config.animations {
            self.page_jump = None;
            return;
        }
        if let (Some((_, image)), Some(bounds)) = (
            self.raster_images.get(&self.page),
            self.page_bounds.get(&self.page),
        ) {
            self.page_jump = Some(PageJump {
                image: image.clone(),
                bounds: *bounds,
                fade: Transition::default(),
                created: std::time::Instant::now(),
            });
        }
    }

    fn begin_zoom(&mut self) {
        self.page_jump = None;
        if self.config.animations && self.display_width > 1. {
            self.zoom_transition
                .get_or_insert_with(|| Transition::settled(self.display_width));
        }
    }

    fn goto(&mut self, page: usize, record: bool, cx: &mut Context<Self>) {
        let Some(doc) = &self.document else {
            return;
        };
        let page = page.min(doc.pages.len() - 1);
        if page == self.page {
            return;
        }
        if record && let Some(location) = self.location() {
            self.history.push(location);
        }
        self.close_visual_mark();
        self.begin_page_jump();
        self.zoom_transition = None;
        self.motion.cancel();
        self.animation_time = None;
        self.page = page;
        self.zoom_anchor = None;
        self.context_menu = None;
        self.tracking_scroll = false;
        self.generation += 1;
        self.raster_tasks.clear();
        self.image = None;
        self.rasters_dirty = true;
        self.selection = None;
        self.selection_page = None;
        self.selection_end_page = None;
        self.pending_offset = Some([0., 0.]);
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.persist_async(cx);
        if self.helper.is_some()
            && let Some(location) = self.nearest_portal()
        {
            self.preview_location(location, cx);
        }
        cx.notify();
    }

    fn zoom_at_pointer(&mut self, factor: f32, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        self.begin_zoom();
        let local = [
            f32::from(pointer.x) - self.viewport_origin[0],
            f32::from(pointer.y) - self.viewport_origin[1],
        ];
        let local = if local[0] >= 0.
            && local[1] >= 0.
            && local[0] <= self.viewport_size[0]
            && local[1] <= self.viewport_size[1]
        {
            local
        } else {
            [self.viewport_size[0] / 2., self.viewport_size[1] / 2.]
        };
        let offset = self.scroll.offset();
        self.zoom_anchor = self.geometry.as_ref().and_then(|layout| {
            omapdf::zoom::Anchor::capture(
                layout,
                local,
                [f32::from(offset.x), f32::from(offset.y)],
                self.canvas_inset,
                self.page,
            )
        });
        // Start from the currently displayed fit width rather than a stale manual zoom value.
        self.zoom = (if self.fit {
            self.display_width / 800.
        } else {
            self.zoom
        } * factor)
            .clamp(0.2, 5.);
        self.fit = false;
        self.fit_height = false;
        self.motion.cancel();
        self.pending_offset = None;
        self.tracking_scroll = false;
        cx.notify();
    }

    fn set_status(&mut self, message: String) {
        self.status_since =
            (!matches!(message.as_str(), "Ready" | "")).then(std::time::Instant::now);
        self.status = message;
    }

    fn reset_completion(&mut self) {
        self.completion_seed = None;
        self.completion_index = 0;
        self.completion_selected = false;
        self.menu_motions[4].cancel();
        self.completion_scroll
            .0
            .borrow()
            .base_handle
            .set_offset(point(px(0.), px(0.)));
    }

    fn command_candidates(&self) -> Vec<omapdf::completion::Candidate> {
        omapdf::completion::candidates(
            self.completion_seed.as_deref().unwrap_or(&self.input.text),
            &self.command_history,
        )
    }

    fn accept_completion(&mut self, candidate: &omapdf::completion::Candidate) {
        let mut value = candidate.value.clone();
        let tail = omapdf::completion::active(&value);
        if (tail.split_once(' ').is_none() && omapdf::completion::usage(tail).is_some())
            || (tail.starts_with("set_preference ") && tail.split_whitespace().count() == 2)
        {
            value.push(' ');
        }
        self.input = TextBuffer::default();
        self.input.replace(None, &value, false);
    }

    fn command_key(&mut self, key: &Keystroke, cx: &mut Context<Self>) -> bool {
        if key.modifiers.control || key.modifiers.alt || key.modifiers.platform {
            return false;
        }
        let candidates = self.command_candidates();
        let length = candidates.len();
        if matches!(key.key.as_str(), "up" | "down" | "tab") {
            if length == 0 {
                return true;
            }
            let backwards = key.key == "up" || (key.key == "tab" && key.modifiers.shift);
            if (key.key != "tab" && self.completion_selected) || self.completion_seed.is_some() {
                self.completion_index = if backwards {
                    (self.completion_index + length - 1) % length
                } else {
                    (self.completion_index + 1) % length
                };
            } else if backwards {
                self.completion_index = length - 1;
            }
            self.completion_selected = true;
            self.completion_index = self.completion_index.min(length - 1);
            if key.key == "tab" {
                self.completion_seed
                    .get_or_insert_with(|| self.input.text.clone());
                self.accept_completion(&candidates[self.completion_index]);
            }
            self.menu_motions[4].cancel();
            self.completion_scroll
                .scroll_to_item(self.completion_index, ScrollStrategy::Top);
            return true;
        }
        if key.key == "right"
            && self.input.selection.is_empty()
            && self.input.selection.end == self.input.text.len()
            && let Some(candidate) = candidates.get(self.completion_index)
            && candidate.value.starts_with(&self.input.text)
            && candidate.value != self.input.text
        {
            self.accept_completion(candidate);
            self.reset_completion();
            return true;
        }
        if key.key != "enter" {
            return false;
        }
        if self.input.text.trim().is_empty() && !self.completion_selected {
            return true;
        }
        let tail = omapdf::completion::active(&self.input.text);
        let exact = tail.split_once(' ').is_some() || omapdf::commands::COMMANDS.contains(&tail);
        if (self.completion_selected || !exact)
            && let Some(candidate) = candidates.get(self.completion_index)
        {
            self.accept_completion(candidate);
        }
        let tail = omapdf::completion::active(&self.input.text);
        if tail.split_once(' ').is_none() && omapdf::completion::usage(tail).is_some() {
            let end = self.input.text.len();
            self.input.selection = end..end;
            self.input.replace(None, " ", false);
            self.reset_completion();
            return true;
        }
        if let Some((command, argument)) = tail.split_once(' ')
            && omapdf::completion::usage(command).is_some()
            && (argument.trim().is_empty()
                || (command == "set_preference" && argument.split_whitespace().count() < 2))
        {
            self.reset_completion();
            return true;
        }
        self.command_history
            .retain(|value| value != &self.input.text);
        self.command_history.push(self.input.text.clone());
        if self.command_history.len() > 50 {
            self.command_history.remove(0);
        }
        cx.notify();
        false
    }

    fn command_box(&self, viewport_width: f32, cx: &mut Context<Self>) -> AnyElement {
        let candidates = self.command_candidates();
        let count = candidates.len();
        let selected = self.completion_index.min(count.saturating_sub(1));
        let ghost =
            if self.input.selection.end == self.input.text.len() && self.input.marked.is_none() {
                candidates
                    .get(selected)
                    .and_then(|candidate| candidate.value.strip_prefix(&self.input.text))
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
        let tail = omapdf::completion::active(&self.input.text);
        let command = tail
            .split_once(' ')
            .map(|(command, _)| command)
            .unwrap_or(tail);
        let usage = omapdf::completion::usage(command).map(|args| format!("{command} {args}"));
        let entity = cx.entity();
        let focus = self.focus.clone();
        let theme = &self.appearance;
        div()
            .id("command-prompt")
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .absolute()
            .bottom(px(24.))
            .left(px((viewport_width - (viewport_width - 48.).min(600.)) / 2.))
            .w(px((viewport_width - 48.).min(600.)))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(self.corner_radius(12.))
            .shadow_lg()
            .border_1()
            .border_color(rgb(theme.border))
            .bg(rgb(theme.panel))
            .text_sm()
            .when(count > 0, |view| {
                view.child(
                    uniform_list(
                        "command-suggestions",
                        count,
                        cx.processor(move |this, range: Range<usize>, _, cx| {
                            range
                                .filter_map(|index| {
                                    let candidate = candidates.get(index)?.clone();
                                    let hover = this.appearance.surface;
                                    Some(
                                        div()
                                            .id(("command-candidate", index))
                                            .w_full()
                                            .h(px(32.))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .rounded(this.corner_radius(4.))
                                            .cursor_pointer()
                                            .when(index == selected, |view| {
                                                view.bg(rgb(this.appearance.selection))
                                            })
                                            .hover(move |style| style.bg(rgb(hover)))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_ellipsis()
                                                    .child(candidate.label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .max_w(px(260.))
                                                    .min_w_0()
                                                    .text_ellipsis()
                                                    .text_xs()
                                                    .text_color(rgb(this.appearance.muted))
                                                    .child(candidate.detail.clone()),
                                            )
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.accept_completion(&candidate);
                                                this.reset_completion();
                                                this.focus.focus(window);
                                                cx.notify();
                                            })),
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(self.completion_scroll.clone())
                    .h(px(count.min(6) as f32 * 32.))
                    .mx_2()
                    .mt_2(),
                )
            })
            .when(count == 0, |view| {
                view.child(
                    div()
                        .px_4()
                        .pt_3()
                        .text_xs()
                        .text_color(rgb(theme.muted))
                        .child(
                            usage
                                .clone()
                                .unwrap_or_else(|| "No matching commands".into()),
                        ),
                )
            })
            .child(
                div()
                    .id("command-input")
                    .relative()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(div().mr_2().text_color(rgb(theme.accent)).child(":"))
                    .child(div().text_color(rgb(theme.foreground)).child(format!(
                        "{}▏{}",
                        &self.input.text[..self.input.selection.end],
                        &self.input.text[self.input.selection.end..]
                    )))
                    .child(div().text_color(rgb(theme.muted)).child(ghost))
                    .child(
                        canvas(
                            |bounds, _, _| bounds,
                            move |_, bounds, window, cx| {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, entity.clone()),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .pb_2()
                    .flex()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(theme.muted))
                    .child("↑↓ select · Tab complete · Enter run")
                    .child(if count > 0 {
                        format!("{} / {}", selected + 1, count)
                    } else {
                        String::new()
                    }),
            )
            .into_any_element()
    }

    fn menu_handles(&self) -> [ScrollHandle; 5] {
        [
            self.settings_scroll.clone(),
            self.panel_scroll.0.borrow().base_handle.clone(),
            self.search_scroll.0.borrow().base_handle.clone(),
            self.context_scroll.clone(),
            self.completion_scroll.0.borrow().base_handle.clone(),
        ]
    }

    fn menu_wheel(&mut self, event: &ScrollWheelEvent, window: &Window, cx: &mut Context<Self>) {
        let index = if self.context_menu.is_some() {
            Some(3)
        } else if self.prompt == Some(Prompt::Command) {
            Some(4)
        } else {
            self.panel.map(|panel| match panel {
                Panel::Settings => 0,
                Panel::Search => 2,
                _ => 1,
            })
        };
        let Some(index) = index else {
            return;
        };
        let handle = self.menu_handles()[index].clone();
        // Consume modal wheel input even at its edges, so it cannot reach the document.
        cx.stop_propagation();
        if !handle.bounds().contains(&event.position) {
            return;
        }
        let delta = event.delta.pixel_delta(px(20.));
        let precise = event.delta.precise();
        let distance = f32::from(delta.y)
            * if precise {
                1.
            } else {
                self.config.wheel_multiplier
            };
        let offset = handle.offset();
        let current = f32::from(offset.y);
        let limit = f32::from(handle.max_offset().height);
        if precise || !self.config.smooth_scroll || !self.config.animations {
            self.menu_motions[index].cancel();
            handle.set_offset(point(offset.x, px((current + distance).clamp(-limit, 0.))));
        } else {
            if self.menu_offsets[index].is_some_and(|last| (last - current).abs() > 0.5) {
                self.menu_motions[index].cancel();
            }
            self.menu_motions[index].set_response(self.config.scroll_response);
            self.menu_motions[index].classic_wheel_glide = self.config.wheel_scroll_glide;
            self.menu_motions[index].queue_wheel(current, distance, limit);
            self.menu_offsets[index] = Some(current);
            self.menu_time.get_or_insert_with(std::time::Instant::now);
            self.request_scroll_frame(window, cx);
        }
        cx.notify();
    }

    fn animate_menus(&mut self, now: std::time::Instant, window: &Window, cx: &Context<Self>) {
        let elapsed = self
            .menu_time
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f32())
            .unwrap_or(1. / 60.);
        let active = if self.context_menu.is_some() {
            Some(3)
        } else if self.prompt == Some(Prompt::Command) {
            Some(4)
        } else {
            self.panel.map(|panel| match panel {
                Panel::Settings => 0,
                Panel::Search => 2,
                _ => 1,
            })
        };
        for (index, handle) in self.menu_handles().into_iter().enumerate() {
            let motion = &mut self.menu_motions[index];
            let offset = handle.offset();
            let current = f32::from(offset.y);
            if active != Some(index)
                || self.menu_offsets[index].is_some_and(|last| (last - current).abs() > 0.5)
            {
                motion.cancel();
            }
            if motion.target.is_some() {
                let next = if self.config.animations && self.config.smooth_scroll {
                    motion.step(current, elapsed, f32::from(handle.max_offset().height))
                } else {
                    let target = motion.target.unwrap();
                    motion.cancel();
                    target
                };
                handle.set_offset(point(offset.x, px(next)));
                self.menu_offsets[index] = Some(next);
            }
        }
        if self
            .menu_motions
            .iter()
            .any(|motion| motion.target.is_some())
        {
            self.request_scroll_frame(window, cx);
        } else {
            self.menu_time = None;
        }
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.context_menu = None;
        let delta = event.delta.pixel_delta(px(20.));
        if event.modifiers.control {
            if delta.y != px(0.) {
                let exponent = if event.delta.precise() {
                    (f32::from(delta.y) / 100.).clamp(-1., 1.)
                } else {
                    f32::from(delta.y).signum()
                };
                self.zoom_at_pointer(self.config.zoom_factor.powf(exponent), event.position, cx);
            }
            cx.stop_propagation();
            return;
        }
        let distance =
            if self.config.layout == LayoutMode::Horizontal && delta.x.abs() > delta.y.abs() {
                delta.x
            } else {
                delta.y
            };
        if distance != px(0.)
            && self.visual_scroll
            && (self.visual_mark.is_some() || self.visual_task.is_some())
        {
            self.move_visual(if distance < px(0.) { 1 } else { -1 }, None, cx);
            cx.stop_propagation();
            return;
        }
        if distance != px(0.) {
            let precise = event.delta.precise();
            self.scroll_input(
                f32::from(distance)
                    * if precise {
                        1.
                    } else {
                        self.config.wheel_multiplier
                    },
                self.config.smooth_scroll && !precise,
                !precise,
                window,
                cx,
            );
            cx.stop_propagation();
        }
    }

    fn scroll_by(&mut self, distance: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll_input(distance, self.config.smooth_scroll, false, window, cx);
    }
    fn scroll_input(
        &mut self,
        distance: f32,
        animate: bool,
        wheel: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.document.is_none() {
            return;
        }
        self.page_jump = None;
        if self.zoom_transition.take().is_some() {
            // A fresh scroll gesture interrupts scaling at the displayed size;
            // the old zoom anchor must not fight the new scroll offset.
            self.zoom = (self.display_width / 800.).clamp(0.2, 5.);
            self.fit = false;
            self.fit_height = false;
            self.zoom_anchor = None;
            self.pending_offset = None;
            self.scroll_save_pending = true;
        }
        let horizontal = self.config.layout == LayoutMode::Horizontal;
        let current = f32::from(if horizontal {
            self.scroll.offset().x
        } else {
            self.scroll.offset().y
        });
        let limit = f32::from(if horizontal {
            self.scroll.max_offset().width
        } else {
            self.scroll.max_offset().height
        })
        .max(0.);
        let at_edge = if distance < 0. {
            current <= -limit + 0.5
        } else {
            current >= -0.5
        };
        if !self.config.layout.continuous()
            && at_edge
            && self.motion.target.is_none()
            && self.image.is_some()
        {
            let last = self.document.as_ref().unwrap().pages.len() - 1;
            if distance < 0. && self.page < last {
                self.goto(
                    self.page.saturating_add(self.config.layout.step()),
                    false,
                    cx,
                );
                return;
            }
            if distance > 0. && self.page > 0 {
                self.goto(
                    self.page.saturating_sub(self.config.layout.step()),
                    false,
                    cx,
                );
                // Applied after the destination page has been laid out.
                self.pending_offset = Some([0., -10000.]);
                return;
            }
        }
        let idle = self.motion.target.is_none();
        if !animate {
            self.motion.cancel();
        }
        let queued = if wheel {
            self.motion.classic_wheel_glide = self.config.wheel_scroll_glide;
            self.motion.queue_wheel(current, distance, limit)
        } else {
            self.motion.queue(current, distance, limit)
        };
        if queued {
            self.tracking_scroll = true;
            if animate {
                if idle {
                    self.animation_time = Some(std::time::Instant::now());
                }
                self.request_scroll_frame(window, cx);
            } else {
                let target = self.motion.target.unwrap();
                self.motion.cancel();
                let offset = self.scroll.offset();
                self.scroll.set_offset(if horizontal {
                    point(px(target), offset.y)
                } else {
                    point(offset.x, px(target))
                });
            }
            cx.notify();
        }
    }

    fn scrollbar_metrics(&self) -> (bool, f32, f32, f32, f32) {
        let horizontal = self.config.layout == LayoutMode::Horizontal;
        let length = self.viewport_size[if horizontal { 0 } else { 1 }].max(1.);
        let limit = f32::from(if horizontal {
            self.scroll.max_offset().width
        } else {
            self.scroll.max_offset().height
        })
        .max(0.);
        let thumb = (length * length / (length + limit)).max(28.).min(length);
        let offset = self.scroll.offset();
        let current = -f32::from(if horizontal { offset.x } else { offset.y });
        let position = if limit > 0. {
            current.clamp(0., limit) / limit * (length - thumb)
        } else {
            0.
        };
        (horizontal, length, thumb, position, limit)
    }

    fn drag_scrollbar(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(grab) = self.scrollbar_grab else {
            return;
        };
        let (horizontal, length, thumb, _, limit) = self.scrollbar_metrics();
        let coordinate = if horizontal {
            f32::from(pointer.x)
        } else {
            f32::from(pointer.y) - self.viewport_origin[1]
        };
        let fraction = ((coordinate - grab) / (length - thumb).max(1.)).clamp(0., 1.);
        let offset = self.scroll.offset();
        self.motion.cancel();
        self.scroll.set_offset(if horizontal {
            point(px(-fraction * limit), offset.y)
        } else {
            point(offset.x, px(-fraction * limit))
        });
        self.tracking_scroll = true;
        self.scroll_save_pending = true;
        cx.notify();
    }

    fn page_edge(&mut self, horizontal: bool, end: bool) {
        let Some(doc) = &self.document else {
            return;
        };
        let layout = PageLayout::new(
            &doc.pages,
            self.config.layout,
            self.page,
            self.display_width,
            16.,
        );
        let Some(page) = layout.pages.iter().find(|page| page.page == self.page) else {
            return;
        };
        let mut offset = self.local_offset();
        let extent = if horizontal { page.width } else { page.height };
        let viewport = f32::from(if horizontal {
            self.scroll.bounds().size.width
        } else {
            self.scroll.bounds().size.height
        });
        offset[if horizontal { 0 } else { 1 }] = if end {
            -(extent - viewport).max(0.) / self.display_width
        } else {
            0.
        };
        self.pending_offset = Some(offset);
        self.tracking_scroll = false;
        self.motion.cancel();
    }

    fn request_scroll_frame(&mut self, window: &Window, cx: &Context<Self>) {
        if self.animation_frame_pending {
            return;
        }
        self.animation_frame_pending = true;
        let reader = cx.weak_entity();
        // GPUI's request_animation_frame() looks up the currently painted view.
        // Input dispatch and Render::render have no such view on its paint stack.
        // Schedule using this reader's explicit identity instead.
        window.on_next_frame(move |_, cx| {
            let _ = reader.update(cx, |reader, cx| {
                reader.animation_frame_pending = false;
                if reader.motion.target.is_some()
                    || reader.menu_motions.iter().any(|m| m.target.is_some())
                    || reader.ui_animating
                {
                    cx.notify();
                }
            });
        });
    }

    fn restore(&mut self, location: Location, cx: &mut Context<Self>) {
        self.begin_page_jump();
        self.zoom = location.zoom.clamp(0.2, 5.);
        self.fit = location.fit;
        if self
            .document
            .as_ref()
            .is_some_and(|doc| doc.id == location.document)
        {
            self.goto(location.page, false, cx);
            self.pending_offset = Some(location.offset);
        } else if let Some(state) = self.library.documents.get(&location.document) {
            let path = state.path.clone();
            self.suppress_open_history = true;
            self.restore_target = Some(location.clone());
            self.open(path, Some(location.page), cx);
        }
    }

    fn prompt(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        self.finish_file_prompt();
        if prompt == Prompt::Command {
            self.panel = None;
            self.context_menu = None;
            self.layout_menu = false;
        }
        self.prompt = Some(prompt);
        self.reset_completion();
        self.input = TextBuffer::default();
        cx.notify();
    }

    fn search_options(&self) -> omapdf::search::Options {
        omapdf::search::Options {
            case_sensitive: self.search_case,
            whole_word: self.search_words,
            regex: self.regex_search,
        }
    }
    fn begin_search(&mut self, cx: &mut Context<Self>) {
        self.panel = Some(Panel::Search);
        self.prompt = Some(Prompt::Search);
        self.input = TextBuffer::default();
        self.input.replace(None, &self.query, false);
        self.input.selection = 0..self.input.text.len();
        cx.notify();
    }
    fn apply_search(
        &mut self,
        result: anyhow::Result<Vec<omapdf::search::Hit>>,
        jump: bool,
        cx: &mut Context<Self>,
    ) {
        self.search_busy = false;
        let jump = jump || std::mem::take(&mut self.search_jump_pending);
        self.search_error = None;
        match result {
            Ok(matches) => {
                self.matches = matches;
                self.match_index = self
                    .matches
                    .iter()
                    .position(|(page, _, _)| *page >= self.page)
                    .unwrap_or(0);
                self.search_scroll
                    .scroll_to_item(self.match_index, ScrollStrategy::Center);
                if jump && !self.query.trim().is_empty() {
                    self.search_committed = true;
                    self.jump_match(cx);
                } else {
                    self.set_status(format!("{} matches", self.matches.len()));
                }
            }
            Err(error) => {
                self.matches.clear();
                self.search_error = Some(error.to_string());
                self.set_status("Invalid search pattern".into());
            }
        }
        cx.notify();
    }
    fn search(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_task = None;
        self.search_generation += 1;
        if self
            .document
            .as_ref()
            .is_some_and(|doc| doc.indexed_pages() < doc.pages.len())
        {
            self.panel = Some(Panel::Search);
            self.input = TextBuffer::default();
            self.input.replace(None, &query, false);
            self.live_search(cx);
            self.search_jump_pending = true;
            return;
        }
        self.query = query;
        if self.prompt == Some(Prompt::Search) {
            self.input = TextBuffer::default();
            self.input.replace(None, &self.query, false);
        }
        self.panel = Some(Panel::Search);
        self.panel_filter.clear();
        let result = self
            .document
            .as_ref()
            .map(|doc| omapdf::search::find(doc, &self.query, self.search_options()))
            .unwrap_or_else(|| Ok(vec![]));
        self.search_committed = true;
        self.apply_search(result, true, cx);
    }
    fn live_search(&mut self, cx: &mut Context<Self>) {
        let Some(document) = self.document.clone() else {
            return;
        };
        let query = self.input.text.clone();
        self.query = query.clone();
        self.region_selection = None;
        self.selection = None;
        self.search_committed = false;
        self.search_jump_pending = false;
        self.search_generation += 1;
        let generation = self.search_generation;
        self.matches.clear();
        self.search_error = None;
        self.search_busy = !query.trim().is_empty();
        self.search_task = None;
        if !self.search_busy {
            cx.notify();
            return;
        }
        let options = self.search_options();
        let executor = cx.background_executor().clone();
        self.search_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(std::time::Duration::from_millis(120)).await;
            let result = executor
                .spawn(async move { omapdf::search::find(&document, &query, options) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation == this.search_generation {
                    this.apply_search(result, false, cx);
                }
            });
        }));
        cx.notify();
    }
    fn jump_match(&mut self, cx: &mut Context<Self>) {
        self.begin_page_jump();
        self.search_scroll
            .scroll_to_item(self.match_index, ScrollStrategy::Center);
        if let Some(&(page, start, end)) = self.matches.get(self.match_index) {
            self.goto(page, true, cx);
            self.region_selection = None;
            self.selection_end_page = None;
            self.selection = Some((start, end.saturating_sub(1)));
            self.selection_page = Some(page);
            self.focus_word(start);
            self.set_status(format!(
                "Match {} / {}",
                self.match_index + 1,
                self.matches.len()
            ));
        } else {
            self.set_status("No matches".into());
        }
        cx.notify();
    }

    fn focus_word(&mut self, index: usize) {
        if let Some(page) = self
            .document
            .as_ref()
            .and_then(|doc| doc.pages.get(self.page))
            && let Some(word) = page.words.get(index)
        {
            let display = self
                .geometry
                .as_ref()
                .and_then(|layout| layout.pages.iter().find(|p| p.page == self.page))
                .map(|p| p.width)
                .unwrap_or(self.display_width);
            let factor = display / page.width / self.display_width;
            self.tracking_scroll = false;
            self.pending_offset = Some([
                if self.config.layout == LayoutMode::Horizontal {
                    -(word.rect.x * factor - 0.12).max(0.)
                } else {
                    0.
                },
                -(word.rect.y * factor - 0.08).max(0.),
            ]);
        }
    }

    fn begin_region(&mut self, action: RegionAction, cx: &mut Context<Self>) {
        if self.loading {
            self.set_status("Wait for the document to open".into());
            return;
        }
        if self.document.is_none() {
            self.set_status("Open a document first".into());
            return;
        }
        self.motion.cancel();
        self.tracking_scroll = false;
        self.zoom_transition = None;
        self.search_generation += 1;
        self.search_task = None;
        self.search_busy = false;
        self.search_jump_pending = false;
        self.close_visual_mark();
        self.close_preview();
        self.panel = None;
        self.context_menu = None;
        self.layout_menu = false;
        self.prompt = None;
        self.selection = None;
        self.selecting = false;
        self.region_selection = None;
        self.region_drag = None;
        self.region_action = action;
        self.region_mode = true;
        self.set_status("Drag an area of the visible pages · Escape cancels".into());
        cx.notify();
    }

    fn region_plan(
        &self,
        end: Point<Pixels>,
        density: f32,
    ) -> anyhow::Result<omapdf::region::Region> {
        use omapdf::{
            document::Rect,
            region::{Placement, Region},
        };
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No document"))?;
        let start = self
            .region_drag
            .ok_or_else(|| anyhow::anyhow!("Drag a document area"))?
            .0;
        let pages = self
            .page_bounds
            .iter()
            .filter_map(|(index, bounds)| {
                Some(Placement {
                    page: *index,
                    bounds: Rect {
                        x: f32::from(bounds.origin.x),
                        y: f32::from(bounds.origin.y),
                        width: f32::from(bounds.size.width),
                        height: f32::from(bounds.size.height),
                    },
                    size: [doc.pages.get(*index)?.width, doc.pages.get(*index)?.height],
                })
            })
            .collect::<Vec<_>>();
        Region::select(
            start,
            [f32::from(end.x), f32::from(end.y)],
            Rect {
                x: self.viewport_origin[0],
                y: self.viewport_origin[1],
                width: self.viewport_size[0],
                height: self.viewport_size[1],
            },
            density,
            self.appearance.background,
            &pages,
        )
    }

    fn finish_region(&mut self, end: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.region_drag.is_none() {
            return;
        }
        match self.region_plan(end, window.scale_factor()) {
            Ok(mut region) => {
                let doc = self.document.as_ref().unwrap();
                // Capture saved markup and search overlays, excluding the selection border.
                for slice in &mut region.slices {
                    let bounds = &self.page_bounds[&slice.page];
                    let scale = f32::from(bounds.size.width) / doc.pages[slice.page].width;
                    let mut paint = |mut rect: omapdf::document::Rect, color: u32| {
                        rect.x -= 1.5 / scale;
                        rect.y -= 0.75 / scale;
                        rect.width += 3. / scale;
                        rect.height += 1.5 / scale;
                        slice.overlays.push(omapdf::region::Overlay {
                            rect,
                            color: (color & 0xffffff00) | (color & 255).min(85),
                        });
                    };
                    if self.show_highlights {
                        for highlight in &self.library.documents[&doc.id].highlights {
                            if highlight.page == slice.page {
                                for rect in omapdf::highlight::line_boxes(&highlight.rects) {
                                    paint(
                                        omapdf::highlight::decoration(highlight.kind, rect),
                                        self.config.highlight_colors[(highlight
                                            .kind
                                            .to_ascii_lowercase()
                                            as usize)
                                            .saturating_sub('a' as usize)
                                            % 26],
                                    );
                                }
                            }
                        }
                    }
                    let words = &doc.pages[slice.page].words;
                    let matched = self
                        .matches
                        .iter()
                        .filter(|(page, _, _)| *page == slice.page)
                        .flat_map(|(_, a, b)| words.get(*a..*b).unwrap_or(&[]))
                        .filter(|word| {
                            !self.show_highlights
                                || !self.library.documents[&doc.id].highlights.iter().any(
                                    |highlight| {
                                        highlight.page == slice.page
                                            && highlight.rects.contains(&word.rect)
                                    },
                                )
                        })
                        .map(|word| word.rect)
                        .collect::<Vec<_>>();
                    for rect in omapdf::highlight::line_boxes(&matched) {
                        paint(rect, self.config.search_color);
                    }
                }
                self.region_selection = Some((doc.id.clone(), region));
                self.region_mode = false;
                self.region_drag = None;
                let action = std::mem::take(&mut self.region_action);
                if matches!(action, RegionAction::Select) {
                    self.set_status(
                        "Area selected · Ctrl+C copies an image · Escape clears".into(),
                    );
                } else {
                    self.capture_region(action, cx);
                }
            }
            Err(error) => {
                self.region_drag = None;
                self.region_selection = None;
                self.set_status(error.to_string());
            }
        }
        cx.notify();
    }

    fn capture_region(&mut self, action: RegionAction, cx: &mut Context<Self>) {
        let Some((id, region)) = self.region_selection.clone() else {
            self.begin_region(action, cx);
            return;
        };
        let Some(doc) = self.document.clone().filter(|doc| doc.id == id) else {
            self.region_selection = None;
            return;
        };
        let renderer = Rasterizer::new();
        renderer.set_palette(self.config.document_theme.then_some(DocumentTheme::new(
            self.appearance.background,
            self.appearance.foreground,
            self.appearance.accent,
        )));
        renderer.set_contrast(self.config.dark_contrast);
        renderer.set_quality(self.config.render_scale);
        renderer.set_sharpness(self.config.render_sharpness);
        let dark = self.dark;
        let generation = self.open_generation;
        self.annotation_busy += 1;
        self.set_status("Rendering selected image…".into());
        let destination = if let RegionAction::Export(path) = action {
            Some(path)
        } else {
            None
        };
        let task = cx.background_executor().spawn(async move {
            let bytes = region.render(doc, &renderer, dark)?;
            if let Some(path) = &destination {
                omapdf::region::write(path, &bytes)?;
            }
            Ok::<_, anyhow::Error>((bytes, destination))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                if this.open_generation != generation {
                    return;
                }
                match result {
                    Ok((bytes, Some(path))) => {
                        drop(bytes);
                        this.set_status(format!("Image saved to {}", path.display()));
                    }
                    Ok((bytes, None)) => {
                        cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
                            ImageFormat::Png,
                            bytes,
                        )));
                        this.set_status("Copied image".into());
                    }
                    Err(error) => this.set_status(format!("Image capture: {error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn text_span(&self) -> Option<omapdf::selection::Span> {
        let (a, b) = self.selection?;
        let page = self.selection_page.unwrap_or(self.page);
        Some(omapdf::selection::Span {
            anchor: omapdf::selection::Endpoint { page, word: a },
            focus: omapdf::selection::Endpoint {
                page: self.selection_end_page.unwrap_or(page),
                word: b,
            },
        })
    }

    fn selected(&self) -> Option<(String, Vec<omapdf::document::Rect>)> {
        let fragments = self.text_span()?.cached(self.document.as_ref()?)?;
        Some((
            omapdf::selection::text(&fragments),
            fragments
                .iter()
                .flat_map(|fragment| fragment.rects.iter().copied())
                .collect(),
        ))
    }

    fn apply_text_selection(&mut self, kind: Option<char>, cx: &mut Context<Self>) {
        let Some(span) = self.text_span() else {
            self.set_status("Select some text first".into());
            return;
        };
        let Some(doc) = self.document.clone() else {
            return;
        };
        let id = doc.id.clone();
        let generation = self.open_generation;
        let cancel = self.index_cancel.clone();
        self.annotation_busy += 1;
        let task = cx
            .background_executor()
            .spawn(async move { span.extract(&doc, &cancel) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                if this.open_generation != generation
                    || this.document.as_ref().is_none_or(|doc| doc.id != id)
                {
                    return;
                }
                match result {
                    Ok(fragments) => {
                        if let Some(kind) = kind {
                            this.highlight_type = kind;
                            this.library
                                .documents
                                .get_mut(&id)
                                .unwrap()
                                .highlights
                                .extend(omapdf::selection::highlights(fragments, kind));
                            if this.persist() {
                                this.set_status("Highlight saved".into());
                            }
                        } else {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                omapdf::selection::text(&fragments),
                            ));
                            this.set_status("Copied selection".into());
                        }
                    }
                    Err(error) => this.set_status(format!("Text selection: {error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn highlight(&mut self, kind: char, cx: &mut Context<Self>) {
        if !(kind.is_ascii_alphabetic() || kind == '_') {
            self.set_status("Highlight type must be a–z, A–Z or _".into());
            return;
        }
        self.apply_text_selection(Some(kind), cx);
    }

    fn preview(&mut self, page: usize, cx: &mut Context<Self>) {
        if let Some(mut location) = self.location() {
            location.page = page;
            self.preview_location(location, cx);
        }
    }

    fn preview_location(&mut self, location: Location, cx: &mut Context<Self>) {
        let Some(current) = self.document.clone() else {
            return;
        };
        let Some(state) = self.library.documents.get(&location.document) else {
            return;
        };
        let target_path = state.path.clone();
        let source_id = current.id.clone();
        let rasterizer = self.rasterizer.clone();
        let dark = self.dark;
        self.preview_generation += 1;
        let generation = self.preview_generation;
        let mut location = location;
        let task = cx.background_executor().spawn(async move {
            let target = if location.document == current.id {
                current
            } else {
                Arc::new(Document::open_with_options(
                    &target_path,
                    current.epub_options,
                )?)
            };
            location.page = location.page.min(target.pages.len() - 1);
            let image = rasterizer.render(target, location.page, 1200, dark)?;
            anyhow::Ok((location, image))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.preview_generation
                    || this.document.as_ref().is_none_or(|doc| doc.id != source_id)
                {
                    return;
                }
                match result {
                    Ok((location, image)) => {
                        this.preview = Some((location.page, image.clone()));
                        this.preview_target = Some(location.clone());
                        if let Some(handle) = this.helper {
                            let _ = handle.update(cx, |view, _, cx| {
                                view.image = Some(image);
                                view.title = format!("Portal · page {}", location.page + 1);
                                cx.notify();
                            });
                        }
                    }
                    Err(error) => this.set_status(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn close_preview(&mut self) {
        self.preview = None;
        self.preview_target = None;
        self.preview_generation += 1;
    }

    fn nearest_portal_index(&self) -> Option<usize> {
        let doc = self.document.as_ref()?;
        omapdf::state::nearest_portal_index(
            doc,
            &self.location()?,
            &self.library.documents.get(&doc.id)?.portals,
        )
    }
    fn nearest_portal(&self) -> Option<Location> {
        let index = self.nearest_portal_index()?;
        Some(
            self.library
                .documents
                .get(&self.document.as_ref()?.id)?
                .portals[index]
                .destination
                .clone(),
        )
    }

    fn close_visual_mark(&mut self) {
        self.visual_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.visual_cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.visual_generation += 1;
        self.visual_task = None;
        self.visual_pending = 0;
        self.visual_mark = None;
    }

    fn visual_seed(&self, position: Point<Pixels>) -> (usize, [f32; 2]) {
        if let Some((page, bounds)) = self
            .page_bounds
            .iter()
            .find(|(_, bounds)| bounds.contains(&position))
            && let Some(doc) = &self.document
        {
            let scale = doc.pages[*page].width / f32::from(bounds.size.width);
            return (
                *page,
                [
                    f32::from(position.x - bounds.origin.x) * scale,
                    f32::from(position.y - bounds.origin.y) * scale,
                ],
            );
        }
        (self.page, [0., 0.])
    }

    fn move_visual(&mut self, delta: i64, seed: Option<(usize, [f32; 2])>, cx: &mut Context<Self>) {
        if seed.is_some() {
            self.close_visual_mark();
        }
        self.visual_pending = (self.visual_pending + delta).clamp(-100_000, 100_000);
        if self.visual_task.is_some() {
            return;
        }
        let Some(doc) = self.document.clone() else {
            return;
        };
        let seed = seed.unwrap_or_else(|| {
            self.visual_seed(point(
                px(self.viewport_origin[0] + self.viewport_size[0] * 0.5),
                px(self.viewport_origin[1] + self.viewport_size[1] * 0.5),
            ))
        });
        let delta = std::mem::take(&mut self.visual_pending);
        let mark = self.visual_mark;
        let generation = self.visual_generation;
        let cancelled = self.visual_cancel.clone();
        let task = cx.background_executor().spawn(async move {
            omapdf::visual::navigate_until(&doc, mark, seed, delta, &cancelled)
        });
        self.visual_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.visual_generation != generation {
                    return;
                }
                this.visual_task = None;
                match result {
                    Ok(Some(mark)) => {
                        this.motion.cancel();
                        if mark.page != this.page {
                            let pending = this.visual_pending;
                            this.goto(mark.page, false, cx);
                            this.visual_pending = pending;
                        }
                        this.visual_mark = Some(mark);
                        // Keep the ruler visible without moving the viewport on every line.
                        let visible = this.page_bounds.get(&mark.page).is_some_and(|bounds| {
                            let scale = f32::from(bounds.size.width)
                                / this.document.as_ref().unwrap().pages[mark.page].width;
                            let top = f32::from(bounds.origin.y) + mark.rect.y * scale;
                            top >= this.viewport_origin[1] + 12.
                                && top + mark.rect.height * scale
                                    <= this.viewport_origin[1] + this.viewport_size[1] - 12.
                                && f32::from(bounds.origin.x) + mark.rect.x * scale
                                    >= this.viewport_origin[0]
                                && f32::from(bounds.origin.x)
                                    + (mark.rect.x + mark.rect.width) * scale
                                    <= this.viewport_origin[0] + this.viewport_size[0]
                        });
                        if !visible {
                            this.focus_word(mark.word);
                        }
                    }
                    Ok(None) => this.set_status("No text lines in this document".into()),
                    Err(error) => this.set_status(format!("Cannot move ruler: {error:#}")),
                }
                if this.visual_pending != 0 {
                    this.move_visual(0, None, cx);
                }
                cx.notify();
            });
        }));
    }

    fn selected_highlight(&self) -> Option<usize> {
        let doc = self.document.as_ref()?;
        let fragments = self.text_span()?.cached(doc)?;
        self.library
            .documents
            .get(&doc.id)?
            .highlights
            .iter()
            .rposition(|highlight| {
                fragments
                    .iter()
                    .filter(|fragment| fragment.page == highlight.page)
                    .any(|fragment| {
                        fragment.rects.iter().any(|word| {
                            highlight.rects.iter().any(|area| {
                                let (x, y) = (word.x + word.width / 2., word.y + word.height / 2.);
                                x >= area.x
                                    && x <= area.x + area.width
                                    && y >= area.y
                                    && y <= area.y + area.height
                            })
                        })
                    })
            })
    }

    fn begin_highlight_note(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(doc) = &self.document else {
            return;
        };
        let Some(highlight) = self
            .library
            .documents
            .get(&doc.id)
            .and_then(|state| state.highlights.get(index))
        else {
            return;
        };
        self.editing_highlight = Some((doc.id.clone(), highlight.clone()));
        let note = highlight.note.clone().unwrap_or_default();
        self.prompt(Prompt::HighlightNote, cx);
        self.input.replace(None, &note, false);
        self.input.selection = 0..self.input.text.len();
    }

    fn save_highlight_note(&mut self, note: &str, cx: &mut Context<Self>) {
        let target = self
            .editing_highlight
            .clone()
            .filter(|(id, _)| self.document.as_ref().is_some_and(|doc| doc.id == *id));
        if target
            .as_ref()
            .is_some_and(|(id, original)| self.library.set_highlight_note(id, &original.meta, note))
        {
            self.editing_highlight = None;
            self.persist();
            self.set_status("Highlight note saved".into());
        } else {
            self.prompt(Prompt::HighlightNote, cx);
            self.input.replace(None, note, false);
            self.set_status(
                "The highlight changed; copy your note before closing this prompt".into(),
            );
        }
        cx.notify();
    }

    fn finish_file_prompt(&mut self) {
        if self.pending_file_prompt {
            self.pending_file_prompt = false;
            self.annotation_busy = self.annotation_busy.saturating_sub(1);
        }
    }

    fn choose_library_file(&mut self, export: bool, cx: &mut Context<Self>) {
        self.annotation_busy += 1;
        if export {
            let directory = self
                .document
                .as_ref()
                .and_then(|doc| doc.path.parent())
                .unwrap_or(std::path::Path::new("."));
            let path = cx.prompt_for_new_path(directory, Some("omapdf-library.json"));
            cx.spawn(async move |this, cx| {
                let result = path.await;
                let _ = this.update(cx, |this, cx| {
                    this.annotation_busy = this.annotation_busy.saturating_sub(1);
                    match result {
                        Ok(Ok(Some(path))) => this.export_library(path, cx),
                        Ok(Err(error)) => {
                            this.prompt(Prompt::ExportLibrary, cx);
                            this.annotation_busy += 1;
                            this.pending_file_prompt = true;
                            this.set_status(format!("File chooser: {error}. Enter an export path"));
                            cx.notify();
                        }
                        _ => {}
                    }
                });
            })
            .detach();
        } else {
            let paths = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("Import library: JSON or SQLite database files".into()),
            });
            cx.spawn(async move |this, cx| {
                let result = paths.await;
                let _ = this.update(cx, |this, cx| {
                    this.annotation_busy = this.annotation_busy.saturating_sub(1);
                    match result {
                        Ok(Ok(Some(paths))) => this.import_library(paths, cx),
                        Ok(Err(error)) => {
                            this.prompt(Prompt::ImportLibrary, cx);
                            this.annotation_busy += 1;
                            this.pending_file_prompt = true;
                            this.set_status(format!("File chooser: {error}. Enter an import path"));
                            cx.notify();
                        }
                        _ => {}
                    }
                });
            })
            .detach();
        }
    }

    fn import_argument(&mut self, argument: &str, cx: &mut Context<Self>) {
        let result =
            if argument.trim_start().starts_with('[') && !std::path::Path::new(argument).exists() {
                serde_json::from_str::<Vec<PathBuf>>(argument)
            } else {
                Ok(vec![PathBuf::from(argument)])
            };
        match result {
            Ok(paths) => self.import_library(paths, cx),
            Err(error) => {
                self.set_status(format!("Import paths: {error}"));
                cx.notify();
            }
        }
    }

    fn import_library(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.read_library(Some(paths), cx);
    }

    fn read_library(&mut self, paths: Option<Vec<PathBuf>>, cx: &mut Context<Self>) {
        self.persist();
        self.annotation_busy += 1;
        self.set_status("Reading library import…".into());
        let known = self.library.clone();
        let options = self.config.epub;
        let task = cx.background_executor().spawn(async move {
            match paths {
                Some(paths) => omapdf::interchange::read_paths(&paths, &known, options),
                None => omapdf::interchange::retry(&known, options),
            }
        });
        cx.spawn(async move |this,cx| {
            let result=task.await;
            let _=this.update(cx,|this,cx| {
                this.annotation_busy=this.annotation_busy.saturating_sub(1);
                let result=result.and_then(|report| {
                    this.persist();
                    let backup=this.library.backup(&this.library_path)?;
                    let processed=report.library.documents.len();
                    let active=this.document.as_ref().and_then(|doc| report.library.documents.get(&doc.id).map(|state| Location {
                        document:doc.id.clone(),page:state.page,zoom:state.zoom,fit:state.fit,offset:state.offset,
                    }));
                    let counts=omapdf::interchange::merge(&mut this.library,report.library)?;
                    if let Some(at)=active {this.restore(at,cx);}
                    if let Some(doc)=&this.document {this.library.touch(&doc.id,&doc.path);}
                    let summary=format!("Merged {processed} documents: {} new bookmarks, {} highlights, {} portals, {} marks; {} updated, {} issues",
                        counts.bookmarks,counts.highlights,counts.portals,counts.marks,counts.updated,report.issues.len());
                    this.import_issues=vec![summary.clone(),format!("Previous library backup: {}",backup.display())];
                    this.import_issues.extend(report.issues);
                    if this.persist() {this.set_status(format!("{summary}. Use import_report for details"));}
                    Ok(())
                });
                if let Err(error)=result {this.import_issues=vec![format!("Library import: {error:#}")];this.set_status(this.import_issues[0].clone());}
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn export_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.persist();
        self.annotation_busy += 1;
        let library = self.library.clone();
        let options = self.config.epub;
        let active = self.library_path.clone();
        self.set_status("Exporting library…".into());
        let task = cx.background_executor().spawn(async move {
            omapdf::interchange::write(&path, &library, options, &active)
                .map(|issues| (path, issues))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                match result {
                    Ok((path, issues)) => {
                        let summary = format!(
                            "Library exported to {}; {} original-format issues",
                            path.display(),
                            issues.len()
                        );
                        this.import_issues = vec![summary.clone()];
                        this.import_issues.extend(issues);
                        this.set_status(summary);
                    }
                    Err(error) => {
                        this.import_issues = vec![format!("Library export: {error:#}")];
                        this.set_status(this.import_issues[0].clone());
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn embed_annotations(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        self.annotation_busy += 1;
        self.persist();
        let library = self.library.clone();
        let colors = self.config.highlight_colors;
        let task = cx.background_executor().spawn(async move {
            omapdf::annotations::embed(&doc, &library, &colors, &path).map(|counts| (path, counts))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                this.set_status(match result {
                    Ok((path, (highlights, bookmarks))) => format!(
                        "Exported {highlights} highlights and {bookmarks} bookmarks to {}",
                        path.display()
                    ),
                    Err(error) => format!("Annotation export: {error:#}"),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn import_annotations(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.document.clone() else {
            return;
        };
        self.annotation_busy += 1;
        let id = doc.id.clone();
        let colors = self.config.highlight_colors;
        let task = cx
            .background_executor()
            .spawn(async move { omapdf::annotations::read(&doc, &colors) });
        cx.spawn(async move |this,cx| {
            let result = task.await;
            let _ = this.update(cx, |this,cx| {
                this.annotation_busy = this.annotation_busy.saturating_sub(1);
                this.persist();
                let result = result.and_then(|imported| omapdf::annotations::merge(&mut this.library,&id,imported));
                let message = match result {
                    Ok((highlights,bookmarks,unsupported)) => {
                        this.persist();
                        format!("Imported {highlights} highlights and {bookmarks} notes; {unsupported} unsupported annotations retained in PDF")
                    }
                    Err(error) => format!("Annotation import: {error:#}"),
                };
                this.set_status(message);
                cx.notify();
            });
        }).detach();
    }

    fn execute(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (name, argument) = command
            .trim()
            .split_once(' ')
            .unwrap_or((command.trim(), ""));
        let count = self.count.parse::<usize>().unwrap_or(1).clamp(1, 100_000);
        self.count.clear();
        if command.contains(';') {
            let mut commands = command
                .split(';')
                .map(str::trim)
                .filter(|command| !command.is_empty());
            while let Some(command) = commands.next() {
                let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
                if matches!(name, "open" | "open_document") && !argument.is_empty() {
                    self.startup_commands
                        .splice(0..0, commands.map(str::to_owned));
                    self.execute(command, window, cx);
                    break;
                }
                self.execute(command, window, cx);
                if self.loading
                    || self.region_mode
                    || self.search_busy
                    || self.annotation_busy > 0
                    || self.prompt == Some(Prompt::Open)
                {
                    self.startup_commands
                        .splice(0..0, commands.map(str::to_owned));
                    break;
                }
            }
            return;
        }
        if matches!(
            name,
            "goto_top_of_page"
                | "goto_bottom_of_page"
                | "goto_left"
                | "goto_right"
                | "zoom_in"
                | "zoom_out"
                | "fit_to_page_width"
                | "fit_to_page"
                | "goto"
                | "goto_beginning"
                | "goto_end"
        ) {
            self.motion.cancel();
            self.animation_time = None;
            self.tracking_scroll = false;
        }
        match name {
            "next_tab" => window.dispatch_action(Box::new(crate::workspace::NextTab), cx),
            "previous_tab" => window.dispatch_action(Box::new(crate::workspace::PreviousTab), cx),
            "close_tab" => window.dispatch_action(Box::new(crate::workspace::CloseTab), cx),
            "select_rect" => self.begin_region(RegionAction::Select, cx),
            "copy_image" | "copy_screenshot_to_clipboard" => {
                self.capture_region(RegionAction::Copy, cx)
            }
            "export_image" if argument.is_empty() => {
                self.prompt(Prompt::ExportImage, cx);
                self.pending_file_prompt = true;
                self.annotation_busy += 1;
            }
            "export_image" => {
                self.capture_region(RegionAction::Export(PathBuf::from(argument)), cx)
            }
            "import" | "import_sqlite" if argument.is_empty() => {
                self.choose_library_file(false, cx)
            }
            "import" | "import_sqlite" => self.import_argument(argument, cx),
            "export" if argument.is_empty() => self.choose_library_file(true, cx),
            "export" => self.export_library(PathBuf::from(argument), cx),
            "retry_imports" => self.read_library(None, cx),
            "import_report" => self.show_panel(Panel::ImportReport, cx),
            "copy_import_report" => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.import_issues.join("\n")))
            }
            "edit_selected_highlight"
            | "add_annot_to_highlight"
            | "add_annot_to_selected_highlight" => {
                if let Some(index) = self.selected_highlight() {
                    self.begin_highlight_note(index, cx);
                    if !argument.is_empty() {
                        self.prompt = None;
                        self.save_highlight_note(argument, cx);
                    }
                } else {
                    self.set_status("Select text inside a highlight first".into());
                }
            }
            "embed_annotations" if argument.is_empty() => self.prompt(Prompt::EmbedAnnotations, cx),
            "embed_annotations" => self.embed_annotations(PathBuf::from(argument), cx),
            "import_annotations" => self.import_annotations(cx),
            "q" | "quit" => {
                self.raster_tasks.clear();
                self.persist();
                cx.quit();
            }
            "open" | "open_document" if !argument.is_empty() => {
                self.open_ordered(PathBuf::from(argument), cx)
            }
            "open" | "open_document" => self.picker(cx),
            "next_page" => self.goto(
                self.page.saturating_add(count * self.config.layout.step()),
                false,
                cx,
            ),
            "previous_page" => self.goto(
                self.page.saturating_sub(count * self.config.layout.step()),
                false,
                cx,
            ),
            "enter_visual_mark_mode" | "visual_mark_under_cursor" => {
                let position = if name == "visual_mark_under_cursor" {
                    window.mouse_position()
                } else {
                    point(
                        px(self.viewport_origin[0] + self.viewport_size[0] * 0.5),
                        px(self.viewport_origin[1] + self.viewport_size[1] * 0.5),
                    )
                };
                self.move_visual(0, Some(self.visual_seed(position)), cx);
            }
            "close_visual_mark" => self.close_visual_mark(),
            "toggle_visual_scroll" => {
                self.visual_scroll = !self.visual_scroll;
                if self.visual_scroll && self.visual_mark.is_none() {
                    self.move_visual(0, None, cx);
                }
                self.set_status(format!(
                    "Visual scroll {}",
                    if self.visual_scroll { "on" } else { "off" }
                ));
            }
            "move_visual_mark_down"
            | "move_visual_mark_up"
            | "move_visual_mark_next"
            | "move_visual_mark_prev" => {
                self.move_visual(
                    if name.ends_with("up") || name.ends_with("prev") {
                        -(count as i64)
                    } else {
                        count as i64
                    },
                    None,
                    cx,
                );
            }
            "move_down" | "move_up" if self.visual_mark.is_some() || self.visual_task.is_some() => {
                self.move_visual(
                    if name == "move_down" {
                        count as i64
                    } else {
                        -(count as i64)
                    },
                    None,
                    cx,
                );
            }
            "move_down" | "move_up" | "screen_down" | "screen_up" => {
                let distance = if name.starts_with("screen") {
                    f32::from(if self.config.layout == LayoutMode::Horizontal {
                        self.scroll.bounds().size.width
                    } else {
                        self.scroll.bounds().size.height
                    }) * self.config.screen_ratio
                } else {
                    self.config.move_amount
                } * count as f32;
                self.scroll_by(
                    if name.ends_with("down") {
                        -distance
                    } else {
                        distance
                    },
                    window,
                    cx,
                );
                return;
            }
            "move_left" | "move_right" => {
                if self.config.layout == LayoutMode::Horizontal {
                    self.scroll_by(
                        if name == "move_left" {
                            self.config.horizontal_move
                        } else {
                            -self.config.horizontal_move
                        },
                        window,
                        cx,
                    );
                    return;
                }
                let offset = self.scroll.offset();
                let limit = f32::from(self.scroll.max_offset().width);
                self.scroll.set_offset(point(
                    px((f32::from(offset.x)
                        + if name == "move_left" {
                            self.config.horizontal_move
                        } else {
                            -self.config.horizontal_move
                        })
                    .clamp(-limit, 0.)),
                    offset.y,
                ));
            }
            "goto_beginning" | "goto_begining" => self.goto(count.saturating_sub(1), true, cx),
            "goto_end" => self.goto(usize::MAX, true, cx),
            "goto_page_with_page_number" => self.prompt(Prompt::Page, cx),
            "goto" => match argument.parse::<usize>() {
                Ok(page) => self.goto(page.saturating_sub(1), true, cx),
                Err(_) => self.set_status("Usage: goto PAGE".into()),
            },
            "zoom_in" | "zoom_out" | "zoom_in_cursor" | "zoom_out_cursor" => {
                let factor = if name.starts_with("zoom_in") {
                    self.config.zoom_factor
                } else {
                    1. / self.config.zoom_factor
                };
                self.zoom_at_pointer(factor, window.mouse_position(), cx);
            }
            "fit_to_page_width" | "fit_to_page_width_smart" => {
                self.begin_zoom();
                self.zoom_anchor = None;
                self.fit = true;
                self.fit_height = false;
                self.zoom = 1.;
            }
            "fit_to_page" => {
                self.begin_zoom();
                self.zoom_anchor = None;
                self.fit = true;
                self.fit_height = true;
            }
            "fit_to_page_height" => {
                self.begin_zoom();
                self.fit = true;
                self.fit_height = true;
            }
            "goto_top_of_page" => {
                if self.config.layout.continuous() {
                    self.page_edge(false, false);
                } else {
                    self.scroll
                        .set_offset(point(self.scroll.offset().x, px(0.)));
                }
            }
            "goto_bottom_of_page" => {
                if self.config.layout.continuous() {
                    self.page_edge(false, true);
                } else {
                    self.scroll.scroll_to_bottom();
                }
            }
            "goto_left" => {
                if self.config.layout.continuous() {
                    self.page_edge(true, false);
                } else {
                    self.scroll
                        .set_offset(point(px(0.), self.scroll.offset().y));
                }
            }
            "goto_right" => {
                if self.config.layout.continuous() {
                    self.page_edge(true, true);
                } else {
                    self.scroll.set_offset(point(
                        -self.scroll.max_offset().width,
                        self.scroll.offset().y,
                    ));
                }
            }
            "next_chapter" | "prev_chapter" => {
                if let Some(doc) = &self.document {
                    let target = if name == "next_chapter" {
                        doc.outline
                            .iter()
                            .filter(|entry| entry.page > self.page)
                            .map(|e| e.page)
                            .min()
                    } else {
                        doc.outline
                            .iter()
                            .filter(|entry| entry.page < self.page)
                            .map(|e| e.page)
                            .max()
                    };
                    if let Some(page) = target {
                        self.goto(page, true, cx);
                    }
                }
            }
            "toggle_smooth_scroll_mode" => {
                self.config.smooth_scroll = !self.config.smooth_scroll;
                self.motion.cancel();
            }
            "toggle_dark_mode" => {
                if self.config.document_theme {
                    self.dark = false;
                    self.change_setting("document_theme", "0", cx);
                } else {
                    self.dark = !self.dark;
                    self.rasters_dirty = true;
                }
            }
            "goto_toc" => self.show_panel(Panel::Toc, cx),
            "collapse_toc" | "expand_toc" => {
                self.collapsed_headings = if name == "collapse_toc" {
                    self.document
                        .as_ref()
                        .map(|doc| omapdf::outline::collapsed_branches(&doc.outline))
                        .unwrap_or_default()
                } else {
                    BTreeSet::new()
                };
                self.panel_filter.clear();
                if self.prompt == Some(Prompt::Filter) {
                    self.input = TextBuffer::default();
                }
                self.panel_index = 0;
                self.panel_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(point(px(0.), px(0.)));
            }
            "open_path" => self.prompt(Prompt::Open, cx),
            "search" | "regex_search" => {
                if name == "regex_search" {
                    self.regex_search = true;
                } else if !argument.is_empty() {
                    self.regex_search = false;
                }
                if argument.is_empty() {
                    self.begin_search(cx);
                } else {
                    self.search(argument.into(), cx);
                }
            }
            "next_item" | "next_match" => {
                if !self.matches.is_empty() {
                    self.match_index = (self.match_index + count) % self.matches.len();
                    self.jump_match(cx);
                }
            }
            "previous_item" | "previous_match" => {
                if !self.matches.is_empty() {
                    self.match_index = (self.match_index + self.matches.len()
                        - count % self.matches.len())
                        % self.matches.len();
                    self.jump_match(cx);
                }
            }
            "command" => self.prompt(Prompt::Command, cx),
            "execute_command" => self.show_panel(Panel::Commands, cx),
            "open_prev_doc" | "recent" => self.show_panel(Panel::Recent, cx),
            "add_bookmark" if argument.is_empty() => {
                self.prompt(Prompt::Bookmark, cx);
                if let Some((text, _)) = self.selected() {
                    self.input.replace(None, &text, false);
                }
            }
            "add_bookmark" => {
                if let Some(location) = self.location() {
                    self.library.bookmarks.push(Bookmark {
                        meta: omapdf::state::AnnotationMeta::new(),
                        name: argument.into(),
                        location,
                    });
                    self.persist();
                }
            }
            "goto_bookmark" | "goto_bookmark_g" => {
                self.panel_global = name.ends_with("_g");
                self.show_panel(Panel::Bookmarks, cx);
            }
            "goto_highlight" | "goto_highlight_g" => {
                self.panel_global = name.ends_with("_g");
                self.show_panel(Panel::Highlights, cx);
            }
            "set_mark" => self.prompt(Prompt::SetMark, cx),
            "goto_mark" => self.prompt(Prompt::JumpMark, cx),
            "add_highlight" if argument.is_empty() => self.prompt(Prompt::Highlight, cx),
            "add_highlight_with_current_type" => self.highlight(self.highlight_type, cx),
            "add_highlight" => self.highlight(argument.chars().next().unwrap_or('a'), cx),
            "set_select_highlight_type" => {
                if let Some(kind) = argument
                    .chars()
                    .next()
                    .filter(|kind| kind.is_ascii_alphabetic() || *kind == '_')
                {
                    self.highlight_type = kind;
                }
            }
            "toggle_highlight" => self.show_highlights = !self.show_highlights,
            "toggle_statusbar" => self.show_statusbar = !self.show_statusbar,
            "set_status_string" => self.set_status(argument.into()),
            "clear_status_string" => self.status.clear(),
            "close_overview" => {
                self.close_preview();
            }
            "close_window" => {
                self.persist();
                window.remove_window();
            }
            "goto_next_highlight"
            | "goto_prev_highlight"
            | "goto_next_highlight_of_type"
            | "goto_prev_highlight_of_type" => {
                if let Some(doc) = &self.document {
                    let groups = self.library.documents[&doc.id].highlight_groups();
                    let mut targets: Vec<_> = groups
                        .iter()
                        .map(|fragments| fragments[0])
                        .filter(|h| !name.ends_with("_of_type") || h.kind == self.highlight_type)
                        .map(|h| h.page)
                        .collect();
                    targets.sort_unstable();
                    targets.dedup();
                    let target = if name.contains("next") {
                        targets
                            .iter()
                            .copied()
                            .find(|page| *page > self.page)
                            .or(targets.first().copied())
                    } else {
                        targets
                            .iter()
                            .copied()
                            .rev()
                            .find(|page| *page < self.page)
                            .or(targets.last().copied())
                    };
                    if let Some(page) = target {
                        self.goto(page, true, cx);
                    }
                }
            }
            "delete_highlight" => {
                let index = self.selected_highlight().or_else(|| {
                    self.document.as_ref().and_then(|doc| {
                        self.library
                            .documents
                            .get(&doc.id)?
                            .highlights
                            .iter()
                            .rposition(|highlight| highlight.page == self.page)
                    })
                });
                if let Some(index) = index
                    && let Some(doc) = &self.document
                {
                    let uuid = self.library.documents[&doc.id].highlights[index]
                        .meta
                        .uuid
                        .clone();
                    self.library.delete_annotation(&uuid);
                    self.persist();
                }
            }
            "delete_bookmark" => {
                if let Some(doc) = &self.document
                    && let Some(index) = self
                        .library
                        .bookmarks
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| b.location.document == doc.id)
                        .min_by_key(|(_, b)| b.location.page.abs_diff(self.page))
                        .map(|(index, _)| index)
                {
                    let uuid = self.library.bookmarks[index].meta.uuid.clone();
                    self.library.delete_annotation(&uuid);
                    self.persist();
                }
            }
            "delete_portal" => {
                if let Some(index) = self.nearest_portal_index()
                    && let Some(doc) = &self.document
                {
                    let uuid = self.library.documents[&doc.id].portals[index]
                        .meta
                        .uuid
                        .clone();
                    self.library.delete_annotation(&uuid);
                    self.persist();
                }
            }
            "goto_selected_text" => {
                if let Some((word, _)) = self.selection {
                    let page = self.selection_page.unwrap_or(self.page);
                    self.goto(page, true, cx);
                    self.focus_word(word);
                }
            }
            "portal_to_overview" => {
                if let (Some(source), Some(destination)) = (
                    self.location(),
                    self.preview.as_ref().and(self.preview_target.clone()),
                ) {
                    self.library
                        .documents
                        .get_mut(&source.document)
                        .unwrap()
                        .portals
                        .push(Portal {
                            meta: omapdf::state::AnnotationMeta::new(),
                            source_offset: Some(source.offset),
                            source_page: source.page,
                            destination,
                        });
                    self.persist();
                    self.set_status("Portal saved".into());
                } else {
                    self.set_status("Open an overview before creating its portal".into());
                }
            }
            "copy" if self.region_selection.is_some() => {
                self.capture_region(RegionAction::Copy, cx)
            }
            "copy" => self.apply_text_selection(None, cx),
            "prev_state" | "next_state" => {
                if let Some(location) = self.location() {
                    let target = if name == "prev_state" {
                        self.history.back(location)
                    } else {
                        self.history.forward(location)
                    };
                    if let Some(target) = target {
                        self.restore(target, cx);
                    }
                }
            }
            "portal" | "portal_to_definition" => {
                if let Some(source) = self.portal_source.take() {
                    if let Some(destination) = self.location() {
                        self.library
                            .documents
                            .get_mut(&source.document)
                            .unwrap()
                            .portals
                            .push(Portal {
                                meta: omapdf::state::AnnotationMeta::new(),
                                source_offset: Some(source.offset),
                                source_page: source.page,
                                destination,
                            });
                        self.persist();
                        self.set_status("Portal saved".into());
                    }
                } else {
                    self.portal_source = self.location();
                    self.set_status("Navigate to destination, then run portal again".into());
                }
            }
            "overview" => {
                let page = argument
                    .parse::<usize>()
                    .ok()
                    .map(|n| n.saturating_sub(1))
                    .unwrap_or(self.page);
                self.preview(page, cx);
            }
            "overview_to_portal" => {
                if let Some(location) = self.nearest_portal() {
                    self.preview_location(location, cx);
                }
            }
            "goto_portal" => {
                if let Some(location) = self.nearest_portal() {
                    if let Some(current) = self.location() {
                        self.history.push(current);
                    }
                    self.restore(location, cx);
                }
            }
            "goto_overview" => {
                if let Some(location) = self.preview_target.clone() {
                    if let Some(current) = self.location() {
                        self.history.push(current);
                    }
                    self.restore(location, cx);
                    self.close_preview();
                }
            }
            "toggle_window_configuration" => {
                if let Some(handle) = self.helper.take() {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                } else {
                    let bounds = Bounds::centered(None, size(px(650.), px(850.)), cx);
                    let appearance = self.appearance.clone();
                    match cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                            app_id: Some("io.omapdf.helper".into()),
                            ..Default::default()
                        },
                        |_, cx| {
                            cx.new(|_| crate::helper::Helper {
                                title: "Omapdf · Portal".into(),
                                image: None,
                                appearance: appearance.clone(),
                            })
                        },
                    ) {
                        Ok(handle) => {
                            self.helper = Some(handle);
                            if let Some(location) = self.nearest_portal() {
                                self.preview_location(location, cx);
                            } else {
                                self.preview(self.page, cx);
                            }
                        }
                        Err(error) => self.set_status(format!("Cannot open helper: {error:#}")),
                    }
                }
            }
            "reload" => {
                if let Some(doc) = &self.document {
                    let path = doc.path.clone();
                    self.suppress_open_history = true;
                    self.restore_target = self.location();
                    self.open(path, Some(self.page), cx);
                }
            }
            "toggle_fullscreen" => window.toggle_fullscreen(),
            "context_menu" => {
                if self.document.is_some() {
                    let page = self.selection_page.unwrap_or(self.page);
                    let position = self
                        .page_bounds
                        .get(&page)
                        .map(|bounds| {
                            if let Some((start, _)) = self.selection
                                && let Some(word) = self
                                    .document
                                    .as_ref()
                                    .and_then(|doc| doc.pages[page].words.get(start))
                            {
                                let scale = f32::from(bounds.size.width)
                                    / self.document.as_ref().unwrap().pages[page].width;
                                point(
                                    bounds.origin.x + px(word.rect.x * scale + 2.),
                                    bounds.origin.y + px(word.rect.y * scale + 2.),
                                )
                            } else {
                                point(bounds.origin.x + px(80.), bounds.origin.y + px(80.))
                            }
                        })
                        .unwrap_or(point(px(100.), px(100.)));
                    self.open_context(page, position, cx);
                }
            }
            "layout_menu" => {
                self.layout_menu = !self.layout_menu;
                self.layout_index = LayoutMode::ALL
                    .iter()
                    .position(|mode| *mode == self.config.layout)
                    .unwrap_or(0);
            }
            "cycle_layout" => {
                let next = self.config.layout.next();
                self.change_setting("page_layout", next.name(), cx);
            }
            "layout_single" => self.change_setting("page_layout", "single", cx),
            "layout_spread" => self.change_setting("page_layout", "spread", cx),
            "layout_horizontal" => self.change_setting("page_layout", "horizontal", cx),
            "layout_vertical" => self.change_setting("page_layout", "vertical", cx),
            "settings" => {
                self.panel = Some(Panel::Settings);
                self.panel_filter.clear();
                self.prompt = None;
            }
            "prefs_user" => self.open_config("prefs_user.config", cx),
            "prefs" => self.open_config("prefs_reference.config", cx),
            "keys_user" => self.open_config("keys_user.config", cx),
            "keys" => self.open_config("keys_reference.config", cx),
            "filter_settings" => {
                self.panel = Some(Panel::Settings);
                self.prompt(Prompt::Filter, cx);
            }
            "reload_config" => self.reload_config(cx),
            "reset_preferences" => self.reset_preferences(cx),
            "set_preference" => {
                if let Some((key, value)) = argument.split_once(' ') {
                    self.change_setting(key, value, cx);
                } else {
                    self.set_status("Usage: set_preference NAME VALUE".into());
                }
            }
            "help" => self.panel = Some(Panel::Help),
            "close" => {
                self.finish_file_prompt();
                self.panel = None;
                self.prompt = None;
                self.context_menu = None;
                self.layout_menu = false;
                self.close_preview();
            }
            _ => self.set_status(format!("Unknown command: {name}")),
        }
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = menu_keystroke(
            &event.keystroke,
            self.context_menu.is_some() || self.layout_menu,
            self.prompt,
            self.panel,
        );
        if self.context_menu.is_some() {
            let key = &key.key;
            if key == "escape" {
                self.context_menu = None;
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if key == "up" || key == "down" {
                let items = self.context_items();
                if let Some(menu) = &mut self.context_menu {
                    menu.advance(&items, key == "down");
                    self.context_scroll.scroll_to_item(menu.active);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if key == "enter" {
                let active = self.context_menu.as_ref().unwrap().active;
                if let Some(MenuItem::Action {
                    action,
                    enabled: true,
                    ..
                }) = self.context_items().get(active)
                {
                    self.context_action(*action, window, cx);
                }
                cx.stop_propagation();
                return;
            }
            self.context_menu = None;
            cx.notify();
        }
        let key = &key;
        let text = key.key_char.as_deref().unwrap_or(&key.key);
        if self.region_mode && key.key != "escape" {
            cx.stop_propagation();
            return;
        }
        if self.layout_menu {
            let length = LayoutMode::ALL.len();
            match key.key.as_str() {
                "up" => self.layout_index = (self.layout_index + length - 1) % length,
                "down" => self.layout_index = (self.layout_index + 1) % length,
                "enter" => self.change_setting(
                    "page_layout",
                    LayoutMode::ALL[self.layout_index].name(),
                    cx,
                ),
                "escape" => self.layout_menu = false,
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if key.key == "escape" {
            if self.region_mode {
                self.startup_commands.clear();
            }
            self.region_mode = false;
            self.region_drag = None;
            self.region_selection = None;
            if matches!(self.prompt, Some(Prompt::Open | Prompt::ExportImage)) {
                self.startup_commands.clear();
            }
            if let Some(request) = self.pending_unlock.take() {
                self.cancelled_unlock = Some((
                    request.path.clone(),
                    omapdf::reload::Revision::read(&request.path).ok(),
                ));
                self.startup_commands.clear();
                self.input = TextBuffer::default();
                self.set_status("Unlock cancelled".into());
            }
            self.finish_file_prompt();
            self.prompt = None;
            self.panel = None;
            self.layout_menu = false;
            self.close_preview();
            self.portal_source = None;
            self.close_visual_mark();
            self.selection = None;
            self.count.clear();
            self.sequence.clear();

            cx.notify();
            return;
        }
        if key.key == "f3"
            || (self.panel == Some(Panel::Search) && key.modifiers.control && key.key == "g")
        {
            self.execute(
                if key.modifiers.shift {
                    "previous_match"
                } else {
                    "next_match"
                },
                window,
                cx,
            );
            cx.stop_propagation();
            return;
        }
        if let Some(prompt) = self.prompt {
            if prompt == Prompt::Command && self.command_key(key, cx) {
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if prompt == Prompt::Search && matches!(key.key.as_str(), "enter" | "up" | "down") {
                if self.search_busy {
                    self.search_jump_pending = true;
                } else if !self.search_committed && !self.matches.is_empty() {
                    self.search_committed = true;
                    self.jump_match(cx);
                } else if !self.matches.is_empty() {
                    let backwards = key.key == "up" || key.modifiers.shift;
                    self.match_index = if backwards {
                        (self.match_index + self.matches.len() - 1) % self.matches.len()
                    } else {
                        (self.match_index + 1) % self.matches.len()
                    };
                    self.jump_match(cx);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if prompt == Prompt::Search && key.modifiers.control && key.key == "f" {
                self.input.selection = 0..self.input.text.len();
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if key.modifiers.control && key.key == "v" {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.input.replace(None, &text, false);
                }
            } else if prompt == Prompt::Filter
                && self.panel == Some(Panel::Toc)
                && matches!(key.key.as_str(), "left" | "right")
            {
                if let Some(&index) = self.visible_headings().get(self.panel_index) {
                    if key.key == "left" {
                        self.collapsed_headings.insert(index);
                    } else {
                        self.collapsed_headings.remove(&index);
                    }
                }
            } else if prompt == Prompt::Filter && matches!(key.key.as_str(), "up" | "down") {
                let length = self.panel_rows().1.len();
                self.panel_index = if key.key == "up" {
                    self.panel_index.saturating_sub(1)
                } else {
                    (self.panel_index + 1).min(length.saturating_sub(1))
                };
                self.panel_scroll
                    .scroll_to_item(self.panel_index, ScrollStrategy::Top);
            } else if key.key == "backspace" {
                self.input.backspace();
                if prompt == Prompt::Filter {
                    self.panel_index = 0;
                }
            } else if key.key == "enter"
                || matches!(
                    prompt,
                    Prompt::SetMark | Prompt::JumpMark | Prompt::Highlight
                )
            {
                if matches!(
                    prompt,
                    Prompt::SetMark | Prompt::JumpMark | Prompt::Highlight
                ) {
                    self.input.replace(None, text, false);
                }
                let row = if prompt == Prompt::Filter {
                    self.panel_rows().1.get(self.panel_index).cloned()
                } else {
                    None
                };
                let input = std::mem::take(&mut self.input).text;
                self.prompt = None;
                match prompt {
                    Prompt::ExportImage => {
                        self.finish_file_prompt();
                        self.capture_region(RegionAction::Export(PathBuf::from(input)), cx);
                    }
                    Prompt::Password => {
                        if let Some(request) = self.pending_unlock.take() {
                            self.suppress_open_history = !request.record;
                            self.restore_target = request.restore;
                            self.open_password(request.path, request.page, Some(input), cx);
                        }
                    }
                    Prompt::Filter => {
                        self.panel_filter = input;
                        if let Some((label, location)) = row {
                            self.activate_row(label, location, window, cx);
                        }
                    }
                    Prompt::Preference => {
                        if let Some(key) = self.editing_setting.take() {
                            self.change_setting(&key, &input, cx);
                        }
                    }
                    Prompt::Command => self.execute(&input, window, cx),
                    Prompt::Search => self.search(input, cx),
                    Prompt::Page => self.execute(&format!("goto {input}"), window, cx),
                    Prompt::Open => self.open_ordered(PathBuf::from(input), cx),
                    Prompt::EmbedAnnotations => self.embed_annotations(PathBuf::from(input), cx),
                    Prompt::HighlightNote => self.save_highlight_note(&input, cx),
                    Prompt::ImportLibrary => {
                        self.finish_file_prompt();
                        self.import_argument(&input, cx);
                    }
                    Prompt::ExportLibrary => {
                        self.finish_file_prompt();
                        self.export_library(PathBuf::from(input), cx);
                    }

                    Prompt::Bookmark => self.execute(&format!("add_bookmark {input}"), window, cx),
                    Prompt::Highlight => self.highlight(input.chars().next().unwrap_or('a'), cx),
                    Prompt::SetMark => {
                        if let Some(mark) = input.chars().next().filter(|c| c.is_ascii_alphabetic())
                            && let Some(location) = self.location()
                        {
                            if mark.is_uppercase() {
                                self.library.global_marks.insert(mark, location);
                                self.library
                                    .global_mark_metadata
                                    .insert(mark, omapdf::state::AnnotationMeta::new());
                            } else {
                                self.library
                                    .documents
                                    .get_mut(&location.document)
                                    .unwrap()
                                    .marks
                                    .insert(mark, location);
                                self.library
                                    .documents
                                    .get_mut(&self.document.as_ref().unwrap().id)
                                    .unwrap()
                                    .mark_metadata
                                    .insert(mark, omapdf::state::AnnotationMeta::new());
                            }
                            self.persist();
                            self.set_status(format!("Mark {mark} saved"));
                        }
                    }
                    Prompt::JumpMark => {
                        if let Some(mark) = input.chars().next() {
                            let target = if mark.is_uppercase() {
                                self.library.global_marks.get(&mark)
                            } else {
                                self.document.as_ref().and_then(|doc| {
                                    self.library.documents[&doc.id].marks.get(&mark)
                                })
                            }
                            .cloned();
                            if let Some(target) = target {
                                if let Some(location) = self.location() {
                                    self.history.push(location);
                                }
                                self.restore(target, cx);
                            }
                        }
                    }
                }
            } else if key.key == "delete" {
                self.input.delete();
            } else if key.key == "left" {
                self.input.move_cursor(false);
            } else if key.key == "right" {
                self.input.move_cursor(true);
            } else if key.key == "home" {
                self.input.selection = 0..0;
            } else if key.key == "end" {
                let end = self.input.text.len();
                self.input.selection = end..end;
            } else if key.modifiers.control && key.key == "a" {
                self.input.selection = 0..self.input.text.len();
            } else {
                return;
            }
            if prompt == Prompt::Command {
                self.reset_completion();
            }
            if prompt == Prompt::Search && self.input.text != self.query {
                self.live_search(cx);
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        cx.stop_propagation();
        let mut modifiers = vec![];
        if key.modifiers.alt {
            modifiers.push("alt");
        }
        if key.modifiers.control {
            modifiers.push("ctrl");
        }
        let text = if text == "space" { " " } else { text };
        if key.modifiers.shift && (text.chars().count() != 1 || key.key == "space") {
            modifiers.push("shift");
        }
        let token = format!(
            "{}{text}",
            if modifiers.is_empty() {
                String::new()
            } else {
                format!("{}-", modifiers.join("-"))
            }
        );
        if self.sequence.is_empty()
            && modifiers.is_empty()
            && text.len() == 1
            && text.as_bytes()[0].is_ascii_digit()
        {
            if self.count.len() < 6 {
                self.count.push_str(text);
            }
            cx.notify();
            return;
        }
        self.sequence.push(token);
        match self.config.binding(&self.sequence) {
            Binding::Exact(command) => {
                let command = command.to_string();
                self.sequence.clear();
                self.execute(&command, window, cx);
            }
            Binding::Prefix => cx.notify(),
            Binding::Missing => {
                self.sequence.clear();
                self.count.clear();
                cx.notify();
            }
        }
    }

    fn show_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        self.panel = Some(panel);
        self.panel_index = 0;
        self.panel_filter.clear();
        self.prompt(Prompt::Filter, cx);
    }

    fn activate_row(
        &mut self,
        label: String,
        location: Option<Location>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(location) = location {
            if let Some(current) = self.location() {
                self.history.push(current);
            }
            self.restore(location, cx);
            self.prompt = None;
            self.panel = None;
        } else if self.panel == Some(Panel::Commands) {
            self.panel = None;
            self.execute(&label, window, cx);
        }
        cx.notify();
    }

    fn panel_rows(&self) -> (&'static str, Vec<(String, Option<Location>)>) {
        self.panel_rows_for(self.panel)
    }
    fn panel_rows_for(
        &self,
        panel: Option<Panel>,
    ) -> (&'static str, Vec<(String, Option<Location>)>) {
        let Some(panel) = panel else {
            return ("", vec![]);
        };
        let mut rows: Vec<(String, Option<Location>)> = vec![];
        let title = match panel {
            Panel::Recent => {
                for id in &self.library.recent {
                    if let Some(state) = self.library.documents.get(id) {
                        rows.push((
                            state.path.display().to_string(),
                            Some(Location {
                                document: id.clone(),
                                page: state.page,
                                zoom: state.zoom,
                                offset: state.offset,
                                fit: state.fit,
                            }),
                        ));
                    }
                }
                "Recent documents"
            }
            Panel::Bookmarks => {
                let mut bookmarks: Vec<_> = self.library.bookmarks.iter().collect();
                if self.config.sort_bookmarks {
                    bookmarks.sort_by_key(|b| (&b.location.document, b.location.page));
                }
                for bookmark in bookmarks {
                    if !self.panel_global
                        && self
                            .document
                            .as_ref()
                            .is_some_and(|doc| doc.id != bookmark.location.document)
                    {
                        continue;
                    }
                    rows.push((
                        format!("{} · p{}", bookmark.name, bookmark.location.page + 1),
                        Some(bookmark.location.clone()),
                    ));
                }
                "Bookmarks"
            }
            Panel::Highlights => {
                for (id, state) in &self.library.documents {
                    if !self.panel_global && self.document.as_ref().is_some_and(|doc| &doc.id != id)
                    {
                        continue;
                    }
                    for fragments in state.highlight_groups() {
                        let highlight = fragments[0];
                        let end_page = fragments.last().unwrap().page;
                        let pages = if end_page == highlight.page {
                            format!("p{}", highlight.page + 1)
                        } else {
                            format!("p{}–{}", highlight.page + 1, end_page + 1)
                        };
                        let text = fragments
                            .iter()
                            .map(|fragment| fragment.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        rows.push((
                            format!(
                                "{} · {}{}",
                                pages,
                                text,
                                highlight
                                    .note
                                    .as_ref()
                                    .map(|note| format!(" · {note}"))
                                    .unwrap_or_default()
                            ),
                            Some(Location {
                                document: id.clone(),
                                page: highlight.page,
                                zoom: 1.,
                                fit: true,
                                ..Default::default()
                            }),
                        ));
                    }
                }
                "Highlights"
            }
            Panel::Search => {
                if let Some(doc) = &self.document {
                    for &(page, start, end) in &self.matches {
                        let words = &doc.pages[page].words;
                        let snippet = words[start.saturating_sub(4)..(end + 6).min(words.len())]
                            .iter()
                            .map(|w| w.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        rows.push((
                            format!("p{} · {snippet}", page + 1),
                            Some(Location {
                                document: doc.id.clone(),
                                page,
                                zoom: self.zoom,
                                fit: self.fit,
                                ..Default::default()
                            }),
                        ));
                    }
                }
                "Search results"
            }
            Panel::Toc => {
                if let Some(doc) = &self.document {
                    for index in self.visible_headings() {
                        let entry = &doc.outline[index];
                        rows.push((
                            format!(
                                "{}{} · p{}",
                                "  ".repeat(if self.config.flat_toc { 0 } else { entry.depth }),
                                entry.title,
                                entry.page + 1
                            ),
                            Some(Location {
                                document: doc.id.clone(),
                                page: entry.page,
                                zoom: self.zoom,
                                fit: self.fit,
                                offset: entry
                                    .position
                                    .map(|[x, y]| {
                                        [
                                            -x / doc.pages[entry.page].width,
                                            -y / doc.pages[entry.page].width,
                                        ]
                                    })
                                    .unwrap_or([0., 0.]),
                            }),
                        ));
                    }
                }
                "Table of contents"
            }
            Panel::Settings => "Settings",
            Panel::ImportReport => {
                for issue in &self.import_issues {
                    rows.push((issue.clone(), None));
                }
                "Library import / export report"
            }
            Panel::Commands => {
                for command in omapdf::commands::COMMANDS {
                    rows.push((command.to_string(), None));
                }
                "Commands"
            }
            Panel::Help => {
                for text in [
                    "Ctrl+O  Open PDF",
                    "o  Enter PDF path",
                    "O  Recent documents",
                    "j / k  Scroll · Ctrl+PageDown/Up  Page",
                    "123gg  Go to page 123",
                    "+ / −  Zoom · w  Fit width",
                    "/  Search · n / N  Next / previous",
                    "Drag across words to select",
                    "Ctrl+C  Copy selected text",
                    "h  Highlight · H  List highlights",
                    "b  Bookmark · B  List bookmarks",
                    "m<char>  Set mark · '<char>  Jump",
                    "Uppercase marks span documents",
                    "Alt+Left / Right  History",
                    "t  Contents · F8  Dark mode",
                    "Right click / Menu  Context menu",
                    "p  Start / finish portal",
                    "v  Preview nearest portal",
                    ":overview 12  Preview page 12",
                    ":goto_overview  Jump to preview",
                    ":  Command · F11  Fullscreen",
                    "Escape  Close panels / cancel",
                ] {
                    rows.push((text.into(), None));
                }
                "Keyboard reference"
            }
        };
        let filter = if self.prompt == Some(Prompt::Filter) {
            &self.input.text
        } else {
            &self.panel_filter
        };
        if !filter.is_empty() && panel != Panel::Toc {
            let terms: Vec<_> = filter.to_lowercase().chars().collect();
            let matches = |value: &str| {
                let mut terms = terms.iter();
                let mut next = terms.next();
                for ch in value.to_lowercase().chars() {
                    if next == Some(&ch) {
                        next = terms.next();
                    }
                }
                next.is_none()
            };
            rows.retain(|(label, _)| {
                matches(label)
                    || (panel == Panel::Commands && matches(omapdf::commands::description(label)))
            });
            if panel == Panel::Commands {
                // Preserve the existing command-name ordering before help-text matches.
                rows.sort_by_key(|(label, _)| !matches(label));
            }
        }
        (title, rows)
    }

    fn reflow_book(&mut self, cx: &mut Context<Self>) {
        if let Some(doc) = &self.document
            && doc.reflowable
            && doc.epub_options != self.config.epub
        {
            let path = doc.path.clone();
            self.suppress_open_history = true;
            self.open(path, None, cx);
        }
    }
    fn apply_loaded_config(&mut self, cx: &mut Context<Self>) {
        self.reflow_book(cx);
        if self
            .geometry
            .as_ref()
            .is_some_and(|g| g.mode != self.config.layout)
        {
            self.pending_offset = Some(self.local_offset());
            self.tracking_scroll = false;
            self.geometry = None;
        }
        self.dark = self.config.dark;
        self.show_highlights = self.config.highlights;
        self.show_statusbar = self.config.statusbar;
        self.rasterizer.set_contrast(self.config.dark_contrast);
        self.rasterizer.set_quality(self.config.render_scale);
        self.rasterizer.set_sharpness(self.config.render_sharpness);
        self.rasterizer
            .set_palette(self.config.document_theme.then_some(DocumentTheme::new(
                self.appearance.background,
                self.appearance.foreground,
                self.appearance.accent,
            )));
        self.motion.set_response(self.config.scroll_response);
        self.motion.cancel();
        self.generation += 1;
        self.raster_tasks.clear();
        self.rasters_dirty = true;
        cx.notify();
    }
    fn reload_config(&mut self, cx: &mut Context<Self>) {
        if let Some(directory) = &self.config.directory {
            match Config::load(directory) {
                Ok(config) => {
                    self.config = config;
                    self.apply_loaded_config(cx);
                    self.settings_message = "Configuration reloaded".into();
                }
                Err(error) => self.settings_message = format!("{error:#}"),
            }
        }
        self.set_status(self.settings_message.clone());
        cx.notify();
    }
    fn reset_preferences(&mut self, cx: &mut Context<Self>) {
        let defaults = Config::default();
        let mut changes: Vec<_> = SETTINGS
            .iter()
            .map(|s| (s.key.to_string(), defaults.value(s.key)))
            .collect();
        for kind in b'a'..=b'z' {
            let key = format!("highlight_color_{}", kind as char);
            changes.push((key.clone(), defaults.value(&key)));
        }
        match self.config.set_preferences(&changes) {
            Ok(()) => {
                self.apply_loaded_config(cx);
                self.settings_message = "Defaults restored".into();
            }
            Err(error) => self.settings_message = format!("{error:#}"),
        }
        self.set_status(self.settings_message.clone());
        cx.notify();
    }

    fn change_setting(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        match self.config.set_preference(key, value) {
            Ok(()) => {
                if key == "page_layout" {
                    self.tracking_scroll = false;
                    self.pending_offset = Some([0., 0.]);
                    self.geometry = None;
                    self.layout_menu = false;
                }
                if matches!(key, "default_dark_mode" | "dark_mode") {
                    self.dark = self.config.dark;
                }
                if key == "show_highlights" {
                    self.show_highlights = self.config.highlights;
                }
                if key == "show_statusbar" {
                    self.show_statusbar = self.config.statusbar;
                }
                self.rasterizer.set_contrast(self.config.dark_contrast);
                self.rasterizer.set_quality(self.config.render_scale);
                self.rasterizer.set_sharpness(self.config.render_sharpness);
                self.rasterizer
                    .set_palette(self.config.document_theme.then_some(DocumentTheme::new(
                        self.appearance.background,
                        self.appearance.foreground,
                        self.appearance.accent,
                    )));
                self.motion.set_response(self.config.scroll_response);
                self.motion.cancel();
                self.rasters_dirty = true;
                if key == "document_theme" {
                    self.page_jump = None;
                    self.raster_tasks.clear();
                }
                self.settings_message = "Saved".into();
                self.reflow_book(cx);
            }
            Err(error) => {
                self.settings_message = format!("{error:#}");
                self.set_status(self.settings_message.clone());
            }
        }
        self.set_status(self.settings_message.clone());
        cx.notify();
    }

    fn edit_setting(&mut self, key: &str, cx: &mut Context<Self>) {
        self.editing_setting = Some(key.into());
        self.prompt(Prompt::Preference, cx);
        self.input.replace(None, &self.config.value(key), false);
        self.input.selection = 0..self.input.text.len();
    }

    fn step_setting(&mut self, key: &str, step: f32, min: f32, max: f32, cx: &mut Context<Self>) {
        if let Ok(current) = self.config.value(key).parse::<f32>() {
            self.change_setting(key, &format!("{:.2}", (current + step).clamp(min, max)), cx);
        }
    }

    fn open_config(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(directory) = &self.config.directory else {
            return;
        };
        let path = directory.join(name);
        let result = (|| -> anyhow::Result<()> {
            std::fs::create_dir_all(directory)?;
            if !path.exists() {
                let content = match name {
                    "prefs_reference.config" => include_str!("../assets/prefs.config"),
                    "keys_reference.config" => include_str!("../assets/keys.config"),
                    _ => "# Omapdf user configuration\n",
                };
                std::fs::write(&path, content)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => cx.open_with_system(&path),
            Err(error) => self.set_status(format!("{error:#}")),
        }
        cx.notify();
    }

    fn settings_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.appearance;
        let filter = if self.prompt == Some(Prompt::Filter) {
            &self.input.text
        } else {
            &self.panel_filter
        };
        let filter = filter.to_lowercase();
        let mut content = div().flex().flex_col().gap_1();
        let mut section = "";
        for (index, setting) in SETTINGS.iter().copied().enumerate().filter(|(_, s)| {
            format!("{} {} {}", s.title, s.key, s.section)
                .to_lowercase()
                .contains(&filter)
        }) {
            if setting.section != section {
                section = setting.section;
                content = content.child(
                    div()
                        .mt_5()
                        .mb_2()
                        .text_xs()
                        .text_color(rgb(theme.accent))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(section),
                );
            }
            let value = self.config.value(setting.key);
            let mut control = div().flex().items_center().gap_1();
            match setting.kind {
                Kind::Toggle => {
                    let on = value == "1";
                    control = control.child(
                        div()
                            .id(("setting-toggle", index))
                            .w(px(34.))
                            .h(px(20.))
                            .p(px(3.))
                            .rounded_full()
                            .cursor_pointer()
                            .bg(rgb(if on { theme.accent } else { theme.surface }))
                            .child(
                                div()
                                    .size(px(14.))
                                    .rounded_full()
                                    .bg(rgb(theme.foreground))
                                    .when(on, |div| div.ml(px(14.))),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_setting(setting.key, if on { "0" } else { "1" }, cx)
                            })),
                    );
                }
                Kind::Number { min, max, step } => {
                    let hover = theme.surface;
                    control = control
                        .child(
                            div()
                                .id(("setting-dec", index))
                                .size(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.corner_radius(4.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(rgb(hover)))
                                .child(
                                    svg()
                                        .path("icons/minus.svg")
                                        .size(px(12.))
                                        .text_color(rgb(theme.muted)),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.step_setting(setting.key, -step, min, max, cx)
                                })),
                        )
                        .child(
                            div()
                                .id(("setting-value", index))
                                .min_w(px(42.))
                                .text_center()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(theme.foreground))
                                .child(
                                    value
                                        .parse::<f32>()
                                        .map(|v| {
                                            format!("{v:.2}")
                                                .trim_end_matches('0')
                                                .trim_end_matches('.')
                                                .to_string()
                                        })
                                        .unwrap_or(value),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit_setting(setting.key, cx)
                                })),
                        )
                        .child(
                            div()
                                .id(("setting-inc", index))
                                .size(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.corner_radius(4.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(rgb(hover)))
                                .child(
                                    svg()
                                        .path("icons/plus.svg")
                                        .size(px(12.))
                                        .text_color(rgb(theme.muted)),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.step_setting(setting.key, step, min, max, cx)
                                })),
                        );
                }
                Kind::Layout => {
                    control = control.child(
                        div()
                            .id("setting-layout")
                            .px_2()
                            .py_1()
                            .rounded(self.corner_radius(4.))
                            .border_1()
                            .border_color(rgb(theme.border))
                            .cursor_pointer()
                            .text_xs()
                            .child(
                                svg()
                                    .path(match self.config.layout {
                                        LayoutMode::Single => "icons/file.svg",
                                        LayoutMode::Spread => "icons/columns-2.svg",
                                        LayoutMode::Horizontal => "icons/gallery-horizontal.svg",
                                        LayoutMode::Vertical => "icons/gallery-vertical.svg",
                                    })
                                    .size(px(16.))
                                    .text_color(rgb(theme.foreground)),
                            )
                            .tooltip({
                                let appearance = theme.clone();
                                let label =
                                    format!("{} · click to cycle", self.config.layout.label());
                                move |_, cx| {
                                    cx.new(|_| crate::helper::Hint {
                                        label: label.clone().into(),
                                        appearance: appearance.clone(),
                                    })
                                    .into()
                                }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.change_setting(
                                    "page_layout",
                                    this.config.layout.next().name(),
                                    cx,
                                );
                            })),
                    );
                }
                Kind::Color => {
                    let current = COLORS
                        .iter()
                        .position(|(_, color)| *color == value)
                        .unwrap_or(0);
                    let color = if setting.key == "text_highlight_color" {
                        self.config.selection_color
                    } else {
                        self.config.search_color
                    };
                    control = control.child(
                        div()
                            .id(("setting-color", index))
                            .px_2()
                            .py_1()
                            .rounded(self.corner_radius(4.))
                            .border_1()
                            .border_color(rgb(theme.border))
                            .cursor_pointer()
                            .child(
                                div()
                                    .size(px(18.))
                                    .rounded(self.corner_radius(4.))
                                    .bg(rgb(color >> 8)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_setting(
                                    setting.key,
                                    COLORS[(current + 1) % COLORS.len()].1,
                                    cx,
                                )
                            })),
                    );
                }
            }
            content = content.child(
                div()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(theme.border))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .id(("setting-label", index))
                                    .text_sm()
                                    .child(setting.title)
                                    .tooltip({
                                        let appearance = theme.clone();
                                        move |_, cx| {
                                            cx.new(|_| crate::helper::Hint {
                                                label: setting.description.into(),
                                                appearance: appearance.clone(),
                                            })
                                            .into()
                                        }
                                    }),
                            )
                            .child(control),
                    ),
            );
        }
        if filter.is_empty() || "highlight palette".contains(&filter) {
            content = content
                .child(
                    div()
                        .mt_5()
                        .text_xs()
                        .text_color(rgb(theme.accent))
                        .child("Highlight palette"),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children((0..26).map(|index| {
                            let key = format!("highlight_color_{}", (b'a' + index as u8) as char);
                            let color = self.config.highlight_colors[index];
                            div()
                                .id(("palette", index))
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.corner_radius(4.))
                                .cursor_pointer()
                                .bg(rgba(color | 0xff))
                                .text_color(rgb(0x101018))
                                .text_xs()
                                .child(((b'a' + index as u8) as char).to_string())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let value = this.config.value(&key);
                                    let current =
                                        COLORS.iter().position(|(_, c)| *c == value).unwrap_or(0);
                                    this.change_setting(
                                        &key,
                                        COLORS[(current + 1) % COLORS.len()].1,
                                        cx,
                                    );
                                }))
                        })),
                );
        }
        let mut panel = div()
            .id("settings-panel")
            .w_full()
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(theme.border))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Settings"))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(self.tool(
                                "settings-search",
                                "Find a setting",
                                "filter_settings",
                                false,
                                cx,
                            ))
                            .child(self.tool(
                                "settings-close",
                                "Close settings",
                                "close",
                                false,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .track_scroll(&self.settings_scroll)
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .px_4()
                    .pb_4()
                    .when(
                        matches!(self.prompt, Some(Prompt::Filter | Prompt::Preference)),
                        |v| v.child(self.dialog_input("Find a setting…", cx)),
                    )
                    .child(content)
                    .child(
                        div()
                            .mt_5()
                            .text_xs()
                            .text_color(rgb(theme.accent))
                            .child("Configuration"),
                    )
                    .children(
                        [
                            ("preferences", "User preferences", "prefs_user.config"),
                            ("keybindings", "Keybindings", "keys_user.config"),
                            ("reference", "Omapdf defaults", "prefs_reference.config"),
                        ]
                        .into_iter()
                        .map(|(id, label, file)| {
                            let hover = theme.surface;
                            div()
                                .id(id)
                                .mt_2()
                                .p_2()
                                .rounded(self.corner_radius(4.))
                                .border_1()
                                .border_color(rgb(theme.border))
                                .flex()
                                .items_center()
                                .justify_between()
                                .cursor_pointer()
                                .hover(move |style| style.bg(rgb(hover)))
                                .child(label)
                                .child(
                                    svg()
                                        .path("icons/external-link.svg")
                                        .size(px(14.))
                                        .text_color(rgb(theme.muted)),
                                )
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.open_config(file, cx)),
                                )
                        }),
                    ),
            );
        panel = panel.child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(rgb(theme.border))
                .flex()
                .items_center()
                .gap_2()
                .child(self.tool(
                    "reload-settings",
                    "Reload settings from disk",
                    "reload_config",
                    false,
                    cx,
                ))
                .child(div().flex_1())
                .child(self.tool(
                    "reset-settings",
                    "Restore default settings",
                    "reset_preferences",
                    false,
                    cx,
                )),
        );
        panel.into_any_element()
    }

    fn page_point(&self, page: usize, position: Point<Pixels>) -> Option<[f32; 2]> {
        let bounds = self.page_bounds.get(&page)?;
        let content = self.document.as_ref()?.pages.get(page)?;
        let scale = f32::from(bounds.size.width) / content.width;
        Some([
            f32::from(position.x - bounds.origin.x) / scale,
            f32::from(position.y - bounds.origin.y) / scale,
        ])
    }
    fn word_at(&self, page: usize, position: Point<Pixels>) -> Option<usize> {
        let [x, y] = self.page_point(page, position)?;
        self.document.as_ref()?.pages.get(page)?.word_at(x, y)
    }
    fn open_context(&mut self, page: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(doc) = &self.document else {
            return;
        };
        let Some(content) = doc.pages.get(page) else {
            return;
        };
        let word = self.word_at(page, position);
        let pdf_point = self.page_point(page, position);
        let contains = |rect: &omapdf::document::Rect| {
            pdf_point.is_some_and(|[x, y]| {
                x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
            })
        };
        let link = content
            .links
            .iter()
            .find(|link| contains(&link.rect))
            .map(|link| link.target.clone());
        let reference = word.and_then(|word| doc.smart_target(page, word));
        let highlight = self.library.documents[&doc.id]
            .highlights
            .iter()
            .position(|h| {
                h.page == page && omapdf::highlight::line_boxes(&h.rects).iter().any(contains)
            });
        if let Some(word) = word {
            if !self
                .text_span()
                .and_then(|span| span.range(page, content.words.len()))
                .is_some_and(|range| range.contains(&word))
            {
                self.selection_end_page = None;
                self.selection = Some((word, word));
                self.selection_page = Some(page);
            }
        } else if self.selection_page != Some(page) {
            self.selection = None;
            self.selection_page = None;
        }
        self.page = page;
        self.motion.cancel();
        self.tracking_scroll = false;
        self.selecting = false;
        self.layout_menu = false;
        self.context_scroll.set_offset(point(px(0.), px(0.)));
        self.context_menu = Some(MenuState {
            x: f32::from(position.x - self.tab_insets().width),
            y: f32::from(position.y - self.tab_insets().height),
            page,
            word,
            link,
            reference,
            highlight,
            active: 0,
        });
        let items = self.context_items();
        if let Some(menu) = &mut self.context_menu {
            menu.active = items
                .iter()
                .position(|item| matches!(item, MenuItem::Action { enabled: true, .. }))
                .unwrap_or(0);
        }
        cx.notify();
    }
    fn context_items(&self) -> Vec<MenuItem> {
        let Some(menu) = self.context_menu.as_ref().or(self.presented_menu.as_ref()) else {
            return vec![];
        };
        let total = self.document.as_ref().map_or(0, |d| d.pages.len());
        let mut items = menu.items(
            self.selected().is_some(),
            menu.page >= self.config.layout.step(),
            menu.page + self.config.layout.step() < total,
            self.dark,
        );
        items.push(MenuItem::Separator);
        for (action, label, enabled) in [
            (MenuAction::SelectRegion, "Select image area", true),
            (
                MenuAction::CopyImage,
                "Copy selected image",
                self.region_selection.is_some(),
            ),
            (
                MenuAction::ExportImage,
                "Save selected image…",
                self.region_selection.is_some(),
            ),
        ] {
            items.push(MenuItem::Action {
                action,
                label,
                icon: "copy",
                hint: "",
                enabled,
            });
        }
        items
    }
    fn follow_link(&mut self, target: &str, cx: &mut Context<Self>) {
        if let Some(fragment) = target.strip_prefix('#') {
            let (page, coordinates) = fragment
                .split_once('@')
                .map(|(p, c)| (p, Some(c)))
                .unwrap_or((fragment, None));
            if let Some(page) = page
                .parse::<usize>()
                .ok()
                .and_then(|page| page.checked_sub(1))
            {
                self.goto(page, true, cx);
                if let Some((x, y)) = coordinates.and_then(|c| c.split_once(','))
                    && let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>())
                    && x.is_finite()
                    && y.is_finite()
                    && let Some(content) =
                        self.document.as_ref().and_then(|doc| doc.pages.get(page))
                {
                    self.pending_offset = Some([-x / content.width, -y / content.width]);
                }
                cx.notify();
            }
            return;
        }
        if target.starts_with("https://")
            || target.starts_with("http://")
            || target.starts_with("mailto:")
        {
            cx.open_url(target);
            return;
        }
        let (path, page) = target.rsplit_once('#').unwrap_or((target, ""));
        let page = page.parse::<usize>().ok().map(|p| p.saturating_sub(1));
        if path.to_lowercase().ends_with(".pdf") || path.to_lowercase().ends_with(".epub") {
            if let Some(doc) = &self.document {
                let path = doc
                    .path
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(path);
                self.open(path, page, cx);
            }
        } else if let Some(page) = page {
            self.goto(page, true, cx);
        } else {
            self.set_status("Unsupported link target".into());
            cx.notify();
        }
    }
    fn context_action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.context_menu.take() else {
            return;
        };
        self.page = menu.page;
        match action {
            MenuAction::Copy => self.apply_text_selection(None, cx),
            MenuAction::SelectRegion => self.begin_region(RegionAction::Select, cx),
            MenuAction::CopyImage => self.capture_region(RegionAction::Copy, cx),
            MenuAction::ExportImage => self.execute("export_image", window, cx),
            MenuAction::Find => {
                if let Some((text, _)) = self.selected() {
                    self.regex_search = false;
                    self.search(text, cx);
                }
            }
            MenuAction::Highlight => self.highlight(self.highlight_type, cx),
            MenuAction::Bookmark => self.execute("add_bookmark", window, cx),
            MenuAction::OpenLink => {
                if let Some(link) = menu.link {
                    self.follow_link(&link, cx);
                }
            }
            MenuAction::CopyLink => {
                if let Some(link) = menu.link {
                    cx.write_to_clipboard(ClipboardItem::new_string(link));
                    self.set_status("Link copied".into());
                }
            }
            MenuAction::JumpReference => {
                if let Some((page, word)) = menu.reference {
                    self.goto(page, true, cx);
                    self.selection_end_page = None;
                    self.selection = Some((word, word));
                    self.selection_page = Some(page);
                    self.focus_word(word);
                }
            }
            MenuAction::PreviewReference => {
                if let Some((page, _)) = menu.reference {
                    self.preview(page, cx);
                }
            }
            MenuAction::EditHighlightNote => {
                if let Some(index) = menu.highlight {
                    self.begin_highlight_note(index, cx);
                }
            }
            MenuAction::DeleteHighlight => {
                if let Some(doc) = &self.document
                    && let Some(index) = menu.highlight
                {
                    let highlights =
                        &mut self.library.documents.get_mut(&doc.id).unwrap().highlights;
                    if index < highlights.len() {
                        let uuid = highlights[index].meta.uuid.clone();
                        self.library.delete_annotation(&uuid);
                        self.persist();
                    }
                }
            }
            MenuAction::Previous => self.execute("previous_page", window, cx),
            MenuAction::Next => self.execute("next_page", window, cx),
            MenuAction::FitWidth => self.execute("fit_to_page_width", window, cx),
            MenuAction::FitPage => self.execute("fit_to_page", window, cx),
            MenuAction::ToggleDark => self.execute("toggle_dark_mode", window, cx),
            MenuAction::Settings => self.execute("settings", window, cx),
        }
        cx.notify();
    }
    fn context_view(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = self
            .context_menu
            .clone()
            .or_else(|| self.presented_menu.clone())
        else {
            return div().into_any_element();
        };
        let items = self.context_items();
        let height: f32 = items
            .iter()
            .map(|item| match item {
                MenuItem::Separator => 9.,
                MenuItem::Colors => 34.,
                MenuItem::Layouts => 38.,
                _ => 30.,
            })
            .sum::<f32>()
            + 8.;
        let viewport = self.reader_viewport(window);
        let viewport = [f32::from(viewport.width), f32::from(viewport.height)];
        let [x, y] = menu.position(viewport, height);
        let theme = &self.appearance;
        div()
            .id("context-menu")
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .opacity(self.menu_opacity)
            .flex()
            .flex_col()
            .absolute()
            .left(px(x))
            .top(px(y - 5. * (1. - self.menu_opacity)))
            .w(px(260.))
            .max_h(px((viewport[1] - 8.).max(80.)))
            .overflow_y_scroll()
            .track_scroll(&self.context_scroll)
            .p_1()
            .rounded(self.corner_radius(6.))
            .bg(rgb(theme.panel))
            .border_1()
            .border_color(rgb(theme.border))
            .shadow_lg()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.context_menu = None;
                cx.notify();
            }))
            .children(items.into_iter().enumerate().map(|(index, item)| {
                match item {
                    MenuItem::Separator => div()
                        .mx_2()
                        .my_1()
                        .h(px(1.))
                        .bg(rgb(theme.border))
                        .into_any_element(),
                    MenuItem::Colors => div()
                        .id(("menu-colors", index))
                        .flex_shrink_0()
                        .h(px(34.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .children(b"abghxz".iter().copied().enumerate().map(|(i, kind)| {
                            let color = self.config.highlight_colors[(kind - b'a') as usize];
                            div()
                                .id(("menu-color", i))
                                .size(px(22.))
                                .rounded(self.corner_radius(4.))
                                .cursor_pointer()
                                .bg(rgba(color | 0xff))
                                .border_1()
                                .border_color(rgb(theme.border))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.context_menu = None;
                                    this.highlight(kind as char, cx);
                                }))
                        }))
                        .into_any_element(),
                    MenuItem::Layouts => div()
                        .id(("menu-layouts", index))
                        .flex_shrink_0()
                        .h(px(38.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .children(LayoutMode::ALL.into_iter().enumerate().map(|(i, mode)| {
                            let icon = match mode {
                                LayoutMode::Single => "file",
                                LayoutMode::Spread => "columns-2",
                                LayoutMode::Horizontal => "gallery-horizontal",
                                LayoutMode::Vertical => "gallery-vertical",
                            };
                            let hover = theme.surface;
                            div()
                                .id(("menu-layout", i))
                                .w(px(48.))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.corner_radius(4.))
                                .cursor_pointer()
                                .when(mode == self.config.layout, |div| {
                                    div.bg(rgb(theme.selection))
                                })
                                .hover(move |style| style.bg(rgb(hover)))
                                .child(
                                    svg()
                                        .path(format!("icons/{icon}.svg"))
                                        .size(px(16.))
                                        .text_color(rgb(if mode == self.config.layout {
                                            theme.accent
                                        } else {
                                            theme.foreground
                                        })),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.context_menu = None;
                                    this.change_setting("page_layout", mode.name(), cx);
                                }))
                        }))
                        .into_any_element(),
                    MenuItem::Action {
                        action,
                        label,
                        icon,
                        hint,
                        enabled,
                    } => {
                        let hover = theme.surface;
                        div()
                            .id(("context-item", index))
                            .flex_shrink_0()
                            .h(px(30.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded(self.corner_radius(4.))
                            .text_color(rgb(if enabled {
                                theme.foreground
                            } else {
                                theme.muted
                            }))
                            .when(enabled, |div| {
                                div.cursor_pointer()
                                    .hover(move |style| style.bg(rgb(hover)))
                            })
                            .when(enabled && index == menu.active, |div| {
                                div.bg(rgb(theme.selection))
                            })
                            .child(
                                svg()
                                    .path(format!("icons/{icon}.svg"))
                                    .size(px(14.))
                                    .text_color(rgb(if enabled {
                                        theme.foreground
                                    } else {
                                        theme.muted
                                    })),
                            )
                            .child(div().flex_1().text_size(px(12.)).child(label))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(theme.muted))
                                    .child(hint),
                            )
                            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                if enabled
                                    && let Some(menu) = &mut this.context_menu
                                    && menu.active != index
                                {
                                    menu.active = index;
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if enabled {
                                    this.context_action(action, window, cx);
                                }
                            }))
                            .into_any_element()
                    }
                }
            }))
            .into_any_element()
    }

    fn page_element(&self, page_index: usize, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let mut page_view = div()
            .id(("page-input", page_index))
            .relative()
            .flex_shrink_0()
            .w(px(width))
            .bg(rgb(if self.config.document_theme {
                self.appearance.background
            } else if self.dark {
                0x000000
            } else {
                0xffffff
            }))
            .when(!self.config.layout.continuous(), |div| {
                div.rounded(self.corner_radius(4.)).shadow_lg()
            });
        {
            let doc = self.document.as_ref().unwrap();
            let page = &doc.pages[page_index];
            let scale = width / page.width;
            page_view = page_view.h(px(page.height * scale));
            if let Some((_, image)) = self.raster_images.get(&page_index) {
                let image = image.clone();
                let corners: Corners<Pixels> = self
                    .corner_radius(if self.config.layout.continuous() {
                        0.
                    } else {
                        4.
                    })
                    .into();
                page_view = page_view.child(
                    canvas(
                        |bounds, _, _| bounds,
                        move |bounds, _, window, _| {
                            let device_scale = window.scale_factor();
                            let mut painted = bounds;
                            let native = image.size(0);
                            // During zoom, keep the previous raster scaled until its replacement arrives.
                            if (f32::from(bounds.size.width) * device_scale - native.width.0 as f32)
                                .abs()
                                <= 1.
                            {
                                painted.origin.x = px((f32::from(bounds.origin.x) * device_scale)
                                    .round()
                                    / device_scale);
                                painted.origin.y = px((f32::from(bounds.origin.y) * device_scale)
                                    .round()
                                    / device_scale);
                                painted.size.width =
                                    px((native.width.0 as f32 - 0.001) / device_scale);
                                painted.size.height =
                                    px((native.height.0 as f32 - 0.001) / device_scale);
                            }
                            let _ = window.paint_image(painted, corners, image.clone(), 0, false);
                        },
                    )
                    .absolute()
                    .size_full(),
                );
            }
            let paint_box = |rect: omapdf::document::Rect, color: u32| {
                let alpha = (color & 0xff).min(85);
                div()
                    .absolute()
                    .left(px(rect.x * scale - 1.5))
                    .top(px(rect.y * scale - 0.75))
                    .w(px(rect.width * scale + 3.))
                    .h(px(rect.height * scale + 1.5))
                    .rounded(self.corner_radius(4.))
                    .bg(rgba((color & 0xffffff00) | alpha))
            };
            let highlights = &self.library.documents[&doc.id].highlights;
            for highlight in highlights {
                if self.show_highlights && highlight.page == page_index {
                    for rect in omapdf::highlight::line_boxes(&highlight.rects) {
                        page_view = page_view.child(paint_box(
                            omapdf::highlight::decoration(highlight.kind, rect),
                            self.config.highlight_colors[(highlight.kind.to_ascii_lowercase()
                                as usize)
                                .saturating_sub('a' as usize)
                                % 26],
                        ));
                    }
                }
            }
            if let Some(mark) = self.visual_mark.filter(|mark| mark.page == page_index) {
                page_view = page_view.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(mark.rect.y * scale))
                        .w(px(page.width * scale))
                        .h(px(mark.rect.height * scale))
                        .bg(rgba((self.appearance.accent << 8) | 35))
                        .border_b_1()
                        .border_color(rgba((self.appearance.accent << 8) | 180)),
                );
            }
            let mut selected_rects = vec![];
            let mut matched_rects = vec![];
            let selected_range = self
                .text_span()
                .and_then(|span| span.range(page_index, page.words.len()));
            let page_matches = if self.matches.is_empty() {
                &[][..]
            } else {
                let start = self.matches.partition_point(|&(p, _, _)| p < page_index);
                let end = self.matches[start..].partition_point(|&(p, _, _)| p == page_index);
                &self.matches[start..start + end]
            };
            if selected_range.is_some() || !page_matches.is_empty() {
                for (index, word) in page.words.iter().enumerate() {
                    let selected = selected_range
                        .as_ref()
                        .is_some_and(|range| range.contains(&index));
                    let matched = !selected
                        && page_matches
                            .iter()
                            .any(|&(_, a, b)| index >= a && index < b);
                    if !selected && !matched {
                        continue;
                    }
                    let saved = self.show_highlights
                        && highlights
                            .iter()
                            .any(|h| h.page == page_index && h.rects.contains(&word.rect));
                    if saved {
                        continue;
                    }
                    if selected {
                        selected_rects.push(word.rect);
                    } else {
                        matched_rects.push(word.rect);
                    }
                }
            }
            for (rects, color) in [
                (selected_rects, self.config.selection_color),
                (matched_rects, self.config.search_color),
            ] {
                for rect in omapdf::highlight::line_boxes(&rects) {
                    page_view = page_view.child(paint_box(rect, color));
                }
            }
            for (i, link) in page.links.iter().enumerate() {
                let target = link.target.clone();
                let rect = link.rect;
                page_view = page_view.child(
                    div()
                        .id(("link", i))
                        .absolute()
                        .left(px(rect.x * scale))
                        .top(px(rect.y * scale))
                        .w(px(rect.width * scale))
                        .h(px(rect.height * scale))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .hover(|style| style.bg(rgba(0x5d9cec44)))
                        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                            if event.is_right_click() {
                                return;
                            }
                            this.follow_link(&target, cx);
                        })),
                );
            }
        }
        if let Some((_, region)) = &self.region_selection {
            let scale = width / self.document.as_ref().unwrap().pages[page_index].width;
            for slice in region
                .slices
                .iter()
                .filter(|slice| slice.page == page_index)
            {
                page_view = page_view.child(
                    div()
                        .absolute()
                        .left(px(slice.source.x * scale))
                        .top(px(slice.source.y * scale))
                        .w(px(slice.source.width * scale))
                        .h(px(slice.source.height * scale))
                        .border_2()
                        .border_color(rgb(self.appearance.accent))
                        .bg(rgba((self.appearance.accent << 8) | 25)),
                );
            }
        }
        let reader = cx.entity();
        page_view = page_view
            .cursor_text()
            .when(self.region_mode, |view| view.cursor(CursorStyle::Crosshair))
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |_, bounds, _, cx| {
                        reader.update(cx, |reader, _| {
                            reader.page_bounds.insert(page_index, bounds);
                        })
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.context_menu = None;
                    if this.prompt == Some(Prompt::Search) {
                        this.prompt = None;
                    }
                    this.motion.cancel();
                    this.tracking_scroll = false;
                    this.page = page_index;
                    this.selection_end_page = None;
                    this.region_selection = None;
                    let word = this.word_at(page_index, event.position);
                    if event.modifiers.control {
                        if let Some((page, word)) = word
                            .and_then(|word| this.document.as_ref()?.smart_target(page_index, word))
                        {
                            this.goto(page, true, cx);
                            this.selection_page = Some(page);
                            this.selection = Some((word, word));
                            this.focus_word(word);
                        }
                    } else if this.visual_scroll
                        && (this.visual_mark.is_some() || this.visual_task.is_some())
                    {
                        this.move_visual(0, Some(this.visual_seed(event.position)), cx);
                    } else {
                        this.selection_page = Some(page_index);
                        this.selection = word.map(|word| (word, word));
                        this.selecting = word.is_some();
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.open_context(page_index, event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                if this.selecting
                    && let Some(word) = this.word_at(page_index, event.position)
                    && let Some((start, end)) = this.selection
                    && (word != end
                        || this
                            .selection_end_page
                            .unwrap_or(this.selection_page.unwrap_or(this.page))
                            != page_index)
                {
                    this.selection_end_page = Some(page_index);
                    this.selection = Some((start, word));
                    cx.notify();
                }
            }));
        page_view.into_any_element()
    }

    fn tool(
        &self,
        id: &'static str,
        label: &'static str,
        command: &'static str,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = match command {
            "open_document" => "icons/folder-open.svg",
            "goto_toc" => "icons/book-open.svg",
            "collapse_toc" => "icons/chevrons-up.svg",
            "expand_toc" => "icons/chevrons-down.svg",
            "search" | "filter_settings" => "icons/search.svg",
            "previous_match" => "icons/chevron-left.svg",
            "next_match" => "icons/chevron-right.svg",
            "goto_highlight" => "icons/bookmark.svg",
            "previous_page" => "icons/chevron-left.svg",
            "next_page" => "icons/chevron-right.svg",
            "zoom_out" => "icons/minus.svg",
            "zoom_in" => "icons/plus.svg",
            "layout_menu" => match self.config.layout {
                LayoutMode::Single => "icons/file.svg",
                LayoutMode::Spread => "icons/columns-2.svg",
                LayoutMode::Horizontal => "icons/gallery-horizontal.svg",
                LayoutMode::Vertical => "icons/gallery-vertical.svg",
            },
            "fit_to_page_width" if self.fit_height => "icons/file.svg",
            "fit_to_page_width" => "icons/scan.svg",
            "settings" => "icons/settings.svg",
            "toggle_dark_mode" if self.dark || self.config.document_theme => "icons/sun.svg",
            "toggle_dark_mode" => "icons/moon.svg",
            "reload_config" => "icons/refresh-cw.svg",
            "reset_preferences" => "icons/rotate-ccw.svg",
            "close" => "icons/x.svg",
            _ => "icons/command.svg",
        };
        let theme = self.appearance.clone();
        let tooltip_theme = theme.clone();
        let hover = theme.surface;
        div()
            .id(id)
            .size(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(self.corner_radius(4.))
            .cursor_pointer()
            .text_color(rgb(if active {
                theme.accent
            } else {
                theme.foreground
            }))
            .when(active, |div| div.bg(rgb(theme.selection)))
            .hover(move |style| style.bg(rgb(hover)))
            .child(svg().path(path).size(px(16.)).text_color(rgb(if active {
                theme.accent
            } else {
                theme.foreground
            })))
            .when(id.starts_with("welcome"), |div| {
                div.w_auto().px_3().gap_2().child(label)
            })
            .tooltip(move |_, cx| {
                cx.new(|_| crate::helper::Hint {
                    label: label.into(),
                    appearance: tooltip_theme.clone(),
                })
                .into()
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.is_right_click() {
                    return;
                }
                this.focus.focus(window);
                this.context_menu = None;
                if command != "layout_menu" {
                    this.layout_menu = false;
                }
                let panel = match command {
                    "goto_toc" => Some(Panel::Toc),
                    "goto_highlight" => Some(Panel::Highlights),
                    "settings" => Some(Panel::Settings),
                    "search" => Some(Panel::Search),
                    _ => None,
                };
                if id == "fit-button" {
                    this.execute(
                        if this.fit && !this.fit_height {
                            "fit_to_page"
                        } else {
                            "fit_to_page_width"
                        },
                        window,
                        cx,
                    );
                } else if panel.is_some() && this.panel == panel {
                    this.panel = None;
                    this.prompt = None;
                } else {
                    this.execute(command, window, cx);
                }
                cx.notify();
            }))
            .into_any_element()
    }

    fn search_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.appearance;
        let editing = self.prompt == Some(Prompt::Search);
        let text = if editing {
            &self.input.text
        } else {
            &self.query
        };
        let input_display = if text.is_empty() {
            "Search…".to_owned()
        } else if editing {
            format!(
                "{}▏{}",
                &text[..self.input.selection.end],
                &text[self.input.selection.end..]
            )
        } else {
            text.clone()
        };
        let entity = cx.entity();
        let focus = self.focus.clone();
        let input_display = StyledText::new(input_display).with_highlights(
            if editing && !self.input.selection.is_empty() {
                vec![(
                    self.input.selection.clone(),
                    HighlightStyle {
                        background_color: Some(rgb(theme.selection).into()),
                        ..Default::default()
                    },
                )]
            } else {
                vec![]
            },
        );
        let field = div()
            .id("search-field")
            .relative()
            .min_h(px(34.))
            .px_3()
            .py_2()
            .rounded(self.corner_radius(6.))
            .border_1()
            .border_color(rgb(if editing { theme.accent } else { theme.border }))
            .bg(rgb(theme.surface))
            .text_sm()
            .overflow_hidden()
            .cursor_text()
            .child(input_display)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window);
                    if this.prompt != Some(Prompt::Search) {
                        this.prompt = Some(Prompt::Search);
                        this.input = TextBuffer::default();
                        this.input.replace(None, &this.query, false);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .when(editing, |field| {
                field.child(
                    canvas(
                        |bounds, _, _| bounds,
                        move |_, bounds, window, cx| {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, entity.clone()),
                                cx,
                            );
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            });
        let count = if self.search_busy {
            if let Some(doc) = &self.document
                && doc.indexed_pages() < doc.pages.len()
            {
                format!(
                    "Indexing {} / {} pages…",
                    doc.indexed_pages(),
                    doc.pages.len()
                )
            } else {
                "Searching…".to_owned()
            }
        } else if self.query.trim().is_empty() {
            String::new()
        } else if self.matches.is_empty() {
            "No matches".to_owned()
        } else {
            format!("{} / {} matches", self.match_index + 1, self.matches.len())
        };
        let mut panel = div()
            .id("search-panel")
            .occlude()
            .w_full()
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Find in document"),
                    )
                    .child(self.tool("search-close", "Close search · Escape", "close", false, cx)),
            )
            .child(div().px_3().child(field))
            .child(
                div().px_3().py_2().flex().items_center().gap_2().children(
                    [
                        (
                            "case",
                            "icons/case-sensitive.svg",
                            "Match case",
                            self.search_case,
                        ),
                        (
                            "word",
                            "icons/whole-word.svg",
                            "Whole words",
                            self.search_words,
                        ),
                        (
                            "regex",
                            "icons/regex.svg",
                            "Regular expression",
                            self.regex_search,
                        ),
                    ]
                    .into_iter()
                    .map(|(id, label, hint, active)| {
                        let hover = theme.surface;
                        div()
                            .id(id)
                            .px_2()
                            .py_1()
                            .rounded(self.corner_radius(4.))
                            .text_xs()
                            .cursor_pointer()
                            .border_1()
                            .border_color(rgb(if active { theme.accent } else { theme.border }))
                            .when(active, |view| view.bg(rgb(theme.selection)))
                            .hover(move |style| style.bg(rgb(hover)))
                            .child(svg().path(label).size(px(14.)).text_color(rgb(if active {
                                theme.accent
                            } else {
                                theme.muted
                            })))
                            .tooltip({
                                let appearance = theme.clone();
                                move |_, cx| {
                                    cx.new(|_| crate::helper::Hint {
                                        label: hint.into(),
                                        appearance: appearance.clone(),
                                    })
                                    .into()
                                }
                            })
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if event.is_right_click() {
                                    return;
                                }
                                match id {
                                    "case" => this.search_case = !this.search_case,
                                    "word" => this.search_words = !this.search_words,
                                    _ => this.regex_search = !this.regex_search,
                                }
                                if this.prompt != Some(Prompt::Search) {
                                    this.input = TextBuffer::default();
                                    this.input.replace(None, &this.query, false);
                                    this.prompt = Some(Prompt::Search);
                                }
                                this.focus.focus(window);
                                this.live_search(cx);
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(rgb(theme.muted)).child(count))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(self.tool(
                                "search-previous",
                                "Previous match · Shift+Enter",
                                "previous_match",
                                false,
                                cx,
                            ))
                            .child(self.tool(
                                "search-next",
                                "Next match · Enter",
                                "next_match",
                                false,
                                cx,
                            )),
                    ),
            );
        if let Some(error) = &self.search_error {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(theme.accent))
                    .child(format!("Invalid expression: {error}")),
            );
        }
        panel
            .child(
                uniform_list(
                    "search-results",
                    self.matches.len(),
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        range
                            .filter_map(|index| {
                                let &(page, start, end) = this.matches.get(index)?;
                                let content = &this.document.as_ref()?.pages[page];
                                let before = content.words[start.saturating_sub(5)..start]
                                    .iter()
                                    .map(|w| w.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let matched = content.words[start..end]
                                    .iter()
                                    .map(|w| w.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let after = content.words[end..(end + 9).min(content.words.len())]
                                    .iter()
                                    .map(|w| w.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                let theme = &this.appearance;
                                let hover = theme.surface;
                                Some(
                                    div()
                                        .id(("search-result", index))
                                        .h(px(82.))
                                        .px_3()
                                        .py_2()
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .border_b_1()
                                        .border_color(rgb(theme.border))
                                        .when(index == this.match_index, |view| {
                                            view.bg(rgb(theme.selection))
                                        })
                                        .hover(move |style| style.bg(rgb(hover)))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .text_xs()
                                                .text_color(rgb(theme.muted))
                                                .child(
                                                    svg().path("icons/file-text.svg").size(px(12.)),
                                                )
                                                .child(format!("Page {}", page + 1)),
                                        )
                                        .child(
                                            div().mt_1().text_sm().child(
                                                StyledText::new(format!(
                                                    "{before} {matched} {after}"
                                                ))
                                                .with_highlights([(
                                                    (before.len() + 1)
                                                        ..(before.len() + 1 + matched.len()),
                                                    HighlightStyle {
                                                        color: Some(rgb(theme.accent).into()),
                                                        font_weight: Some(FontWeight::SEMIBOLD),
                                                        ..Default::default()
                                                    },
                                                )]),
                                            ),
                                        )
                                        .on_click(cx.listener(
                                            move |this, event: &ClickEvent, window, cx| {
                                                if event.is_right_click() {
                                                    return;
                                                }
                                                this.match_index = index;
                                                this.search_committed = true;
                                                this.jump_match(cx);
                                                this.focus.focus(window);
                                            },
                                        )),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .mx_2()
                .mb_2()
                .track_scroll(self.search_scroll.clone())
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }

    fn visible_headings(&self) -> Vec<usize> {
        let filter = if self.prompt == Some(Prompt::Filter) {
            &self.input.text
        } else {
            &self.panel_filter
        };
        self.document
            .as_ref()
            .map(|doc| {
                omapdf::outline::visible(
                    &doc.outline,
                    &self.collapsed_headings,
                    filter,
                    self.config.flat_toc,
                )
            })
            .unwrap_or_default()
    }

    fn corner_radius(&self, pixels: f32) -> Pixels {
        px(if self.config.rounded_corners {
            pixels
        } else {
            0.
        })
    }

    fn panel_content(&self, panel: Option<Panel>, cx: &mut Context<Self>) -> AnyElement {
        if panel == Some(Panel::Settings) {
            return self.settings_panel(cx);
        }
        if panel == Some(Panel::Search) {
            return self.search_panel(cx);
        }
        let (title, rows) = self.panel_rows_for(panel);
        let headings = self.visible_headings();
        let theme = &self.appearance;
        let field = self.dialog_input("Filter…", cx);
        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                svg()
                                    .path(match panel {
                                        Some(Panel::Toc) => "icons/book-open.svg",
                                        Some(Panel::Bookmarks | Panel::Highlights) => {
                                            "icons/bookmark.svg"
                                        }
                                        Some(Panel::Recent) => "icons/folder-open.svg",
                                        _ => "icons/command.svg",
                                    })
                                    .size(px(16.))
                                    .text_color(rgb(theme.muted)),
                            )
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(panel == Some(Panel::Toc) && !self.config.flat_toc, |v| {
                                v.child(self.tool(
                                    "toc-collapse-all",
                                    "Collapse all",
                                    "collapse_toc",
                                    false,
                                    cx,
                                ))
                                .child(self.tool(
                                    "toc-expand-all",
                                    "Expand all",
                                    "expand_toc",
                                    false,
                                    cx,
                                ))
                            })
                            .child(self.tool("panel-close", "Close · Escape", "close", false, cx)),
                    ),
            )
            .child(div().px_4().pb_3().child(field))
            .child(if rows.is_empty() {
                div()
                    .flex_1()
                    .p_4()
                    .text_color(rgb(theme.muted))
                    .child(
                        if panel == Some(Panel::Toc)
                            && self
                                .document
                                .as_ref()
                                .is_some_and(|doc| doc.outline.is_empty())
                        {
                            "This document has no table of contents."
                        } else {
                            "No results"
                        },
                    )
                    .into_any_element()
            } else {
                uniform_list(
                    "panel-rows",
                    rows.len(),
                    cx.processor(move |this, range: Range<usize>, _, cx| {
                        let theme = &this.appearance;
                        let hover = theme.surface;
                        range
                            .filter_map(|i| {
                                let (label, location) = rows.get(i)?.clone();
                                let heading = if panel == Some(Panel::Toc) {
                                    headings.get(i).and_then(|&index| {
                                        this.document.as_ref().map(|doc| {
                                            (
                                                index,
                                                &doc.outline[index],
                                                doc.outline.get(index + 1).is_some_and(|entry| {
                                                    entry.depth > doc.outline[index].depth
                                                }),
                                            )
                                        })
                                    })
                                } else {
                                    None
                                };
                                let mut row = div()
                                    .id(("row", i))
                                    .h(px(36.))
                                    .px_2()
                                    .py_2()
                                    .rounded(this.corner_radius(4.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(rgb(hover)))
                                    .when(i == this.panel_index, |v| v.bg(rgb(theme.selection)));
                                if let Some((index, entry, children)) = heading {
                                    let depth = if this.config.flat_toc { 0 } else { entry.depth };
                                    let folded = this.collapsed_headings.contains(&index);
                                    row = row
                                        .pl(px(8. + depth as f32 * 18.))
                                        .child(
                                            div()
                                                .id(("heading-fold", index))
                                                .size(px(18.))
                                                .flex_shrink_0()
                                                .cursor_pointer()
                                                .when(children && !this.config.flat_toc, |v| {
                                                    v.child(
                                                        svg()
                                                            .path(if folded {
                                                                "icons/chevron-right.svg"
                                                            } else {
                                                                "icons/chevron-down.svg"
                                                            })
                                                            .size(px(14.))
                                                            .text_color(rgb(theme.muted)),
                                                    )
                                                })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    if children {
                                                        if !this.collapsed_headings.remove(&index) {
                                                            this.collapsed_headings.insert(index);
                                                        }
                                                        this.panel_index = 0;
                                                    }
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_ellipsis()
                                                .when(depth == 0, |v| {
                                                    v.font_weight(FontWeight::SEMIBOLD)
                                                })
                                                .child(entry.title.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_xs()
                                                .text_color(rgb(theme.muted))
                                                .child((entry.page + 1).to_string()),
                                        );
                                } else {
                                    row = row.child(
                                        div().min_w_0().text_ellipsis().child(label.clone()),
                                    );
                                }
                                if panel == Some(Panel::Commands) {
                                    let description = omapdf::commands::description(&label);
                                    if !description.is_empty() {
                                        row = row.child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_ellipsis()
                                                .text_xs()
                                                .text_color(rgb(theme.muted))
                                                .child(description),
                                        );
                                        let appearance = theme.clone();
                                        row = row.tooltip(move |_, cx| {
                                            cx.new(|_| crate::helper::Hint {
                                                label: description.into(),
                                                appearance: appearance.clone(),
                                            })
                                            .into()
                                        });
                                    }
                                }
                                Some(row.on_click(cx.listener(move |this, _, window, cx| {
                                    this.activate_row(label.clone(), location.clone(), window, cx)
                                })))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.panel_scroll.clone())
                .flex_1()
                .min_h_0()
                .px_2()
                .pb_3()
                .into_any_element()
            })
            .into_any_element()
    }

    fn dialog_input(&self, placeholder: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let editing = matches!(self.prompt, Some(Prompt::Filter | Prompt::Preference));
        let text = if editing {
            &self.input.text
        } else {
            &self.panel_filter
        };
        let display = if text.is_empty() {
            placeholder.to_string()
        } else if editing {
            format!(
                "{}▏{}",
                &text[..self.input.selection.end],
                &text[self.input.selection.end..]
            )
        } else {
            text.clone()
        };
        let entity = cx.entity();
        let focus = self.focus.clone();
        div()
            .id("dialog-input")
            .relative()
            .px_3()
            .py_2()
            .rounded(self.corner_radius(6.))
            .border_1()
            .border_color(rgb(self.appearance.border))
            .bg(rgb(self.appearance.surface))
            .child(display)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window);
                    if !matches!(this.prompt, Some(Prompt::Filter | Prompt::Preference)) {
                        this.prompt = Some(Prompt::Filter);
                        this.input = TextBuffer::default();
                        this.input.replace(None, &this.panel_filter, false);
                    }
                    cx.notify();
                }),
            )
            .when(editing, |v| {
                v.child(
                    canvas(
                        |bounds, _, _| bounds,
                        move |_, bounds, window, cx| {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, entity.clone()),
                                cx,
                            )
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
            .into_any_element()
    }

    fn panel_dialog(&self, opacity: f32, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let viewport = self.reader_viewport(window);
        let width = (f32::from(viewport.width) - 48.).clamp(180., 720.);
        let preferred_height = if self.presented_panel == Some(Panel::Toc) {
            self.document
                .as_ref()
                .map(|doc| (doc.outline.len() as f32 * 36. + 110.).clamp(240., 640.))
                .unwrap_or(240.)
        } else {
            640.
        };
        let height = (f32::from(viewport.height) - 120.).clamp(120., preferred_height);
        div()
            .id("panel-backdrop")
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000055))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.panel = None;
                    this.prompt = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("panel-dialog")
                    .occlude()
                    .w(px(width))
                    .h(px(height))
                    .rounded(self.corner_radius(10.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(self.appearance.border))
                    .bg(rgb(self.appearance.panel))
                    .opacity(opacity)
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(self.panel_content(self.presented_panel, cx)),
            )
            .into_any_element()
    }
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.helper.is_some_and(|helper| helper.read(cx).is_err()) {
            self.helper = None;
        }
        let window_title = self
            .document
            .as_ref()
            .map(|doc| format!("{} — Omapdf", doc.title))
            .unwrap_or_else(|| "Omapdf".into());
        if self.last_window_title != window_title {
            window.set_window_title(&window_title);
            self.last_window_title = window_title;
        }
        let now = std::time::Instant::now();
        self.animate_menus(now, window, cx);
        let enabled = self.config.animations;
        let toast_opacity = self.toast_transition.update(
            if self.show_statusbar && self.status_since.is_some() {
                1.
            } else {
                0.
            },
            now,
            enabled,
        );

        if self.panel.is_some() {
            self.presented_panel = self.panel;
        }
        if let Some(menu) = &self.context_menu {
            self.presented_menu = Some(menu.clone());
        }
        if self.preview.is_some() && self.helper.is_none() {
            self.presented_preview = self.preview.clone();
        }
        let panel_opacity =
            self.panel_transition
                .update(if self.panel.is_some() { 1. } else { 0. }, now, enabled);
        self.menu_opacity = self.menu_transition.update(
            if self.context_menu.is_some() { 1. } else { 0. },
            now,
            enabled,
        );
        let layout_opacity =
            self.layout_transition
                .update(if self.layout_menu { 1. } else { 0. }, now, enabled);
        let preview_opacity = self.preview_transition.update(
            if self.preview.is_some() && self.helper.is_none() {
                1.
            } else {
                0.
            },
            now,
            enabled,
        );
        self.ui_animating = [
            &self.panel_transition,
            &self.menu_transition,
            &self.layout_transition,
            &self.preview_transition,
        ]
        .iter()
        .any(|animation| animation.active(now));
        if panel_opacity == 0. && self.panel.is_none() {
            self.presented_panel = None;
        }
        if self.menu_opacity == 0. && self.context_menu.is_none() {
            self.presented_menu = None;
        }
        if preview_opacity == 0. && self.preview.is_none() {
            self.presented_preview = None;
        }
        let padding = self.config.page_padding;
        let viewport = self.reader_viewport(window);
        let available = (f32::from(viewport.width) - padding * 2.).max(150.);
        let mode = self.config.layout;
        let viewport_width = f32::from(viewport.width).max(150.);
        let viewport_height = (f32::from(viewport.height) - 36.).max(100.);
        let columns = if mode == LayoutMode::Spread { 2. } else { 1. };
        let gap = 16.;
        let page_available = ((available - (columns - 1.) * gap) / columns).max(100.);
        let reference = self
            .document
            .as_ref()
            .and_then(|doc| doc.pages.get(if mode.continuous() { 0 } else { self.page }));
        let target_width = if self.fit_height || (mode == LayoutMode::Horizontal && self.fit) {
            reference
                .map(|page| {
                    ((viewport_height - padding * 2.) * page.width / page.height)
                        .max(100.)
                        .min(page_available)
                })
                .unwrap_or(page_available)
        } else if self.fit {
            page_available.min(self.config.max_page_width / columns)
        } else {
            800. * self.zoom
        };
        let width = if let Some(transition) = &mut self.zoom_transition {
            transition.update(target_width, now, enabled)
        } else {
            target_width
        };
        let zoom_active = self
            .zoom_transition
            .as_ref()
            .is_some_and(|transition| transition.active(now));
        if zoom_active {
            self.ui_animating = true;
            self.request_scroll_frame(window, cx);
        } else {
            self.zoom_transition = None;
        }
        self.viewport_origin = [0., 66.];
        self.viewport_size = [viewport_width, viewport_height];
        self.display_width = width;
        if self.motion.target.is_some() {
            let now = std::time::Instant::now();
            let elapsed = self
                .animation_time
                .replace(now)
                .map(|last| now.duration_since(last).as_secs_f32())
                .unwrap_or(1. / 60.);
            let offset = self.scroll.offset();
            let horizontal = mode == LayoutMode::Horizontal;
            let next = self.motion.step(
                f32::from(if horizontal { offset.x } else { offset.y }),
                elapsed,
                f32::from(if horizontal {
                    self.scroll.max_offset().width
                } else {
                    self.scroll.max_offset().height
                }),
            );
            self.scroll.set_offset(if horizontal {
                point(px(next), offset.y)
            } else {
                point(offset.x, px(next))
            });
            if self.motion.target.is_some() {
                self.request_scroll_frame(window, cx);
            } else {
                self.animation_time = None;
            }
        }
        let page_view: AnyElement;
        if let Some(doc) = self.document.clone() {
            let full_viewport = window.viewport_size();
            let scale_factor = window.scale_factor();
            let hint_key = (
                f32::from(full_viewport.width).round() as u32,
                (scale_factor * 100.).round() as u32,
            );
            if hint_key.0 >= 300 && self.saved_viewport_hint != Some(hint_key) {
                self.saved_viewport_hint = Some(hint_key);
                let hint = ViewportHint {
                    width: f32::from(full_viewport.width),
                    height: f32::from(full_viewport.height),
                    scale: scale_factor,
                };
                cx.background_executor()
                    .spawn(async move { save_viewport_hint(hint) })
                    .detach();
            }
            let layout_page = if mode.continuous() { 0 } else { self.page };
            let layout = match &self.layout_cache {
                Some((id, cached_mode, cached_page, cached_w, cached_gap, cached))
                    if *id == doc.id
                        && *cached_mode == mode
                        && *cached_page == layout_page
                        && *cached_w == width.to_bits()
                        && *cached_gap == gap.to_bits() =>
                {
                    cached.clone()
                }
                _ => {
                    let built = PageLayout::new(&doc.pages, mode, self.page, width, gap);
                    self.layout_cache = Some((
                        doc.id.clone(),
                        mode,
                        layout_page,
                        width.to_bits(),
                        gap.to_bits(),
                        built.clone(),
                    ));
                    built
                }
            };
            let content_width = (layout.width + padding * 2.).max(viewport_width);
            let content_height = (layout.height + padding * 2.).max(viewport_height);
            let center_x = (content_width - layout.width) / 2.;
            self.canvas_inset = [center_x, padding];
            if let Some(anchor) = self.zoom_anchor
                && let Some([x, y]) = anchor.offset(
                    &layout,
                    self.canvas_inset,
                    self.viewport_size,
                    [content_width, content_height],
                )
            {
                self.scroll.set_offset(point(px(x), px(y)));
                self.geometry = Some(layout.clone());
                if !zoom_active {
                    self.zoom_anchor = None;
                    self.scroll_save_pending = true;
                }
            }
            if let Some([x, y]) = self.pending_offset.take() {
                self.motion.cancel();
                let origin = layout.origin(self.page);
                self.scroll
                    .set_offset(point(px(x * width - origin[0]), px(y * width - origin[1])));
            }
            let offset = self.scroll.offset();
            let view_x = -f32::from(offset.x) - center_x;
            let view_y = -f32::from(offset.y) - padding;
            if mode.continuous() && self.tracking_scroll {
                let position = if mode == LayoutMode::Horizontal {
                    view_x + viewport_width * 0.25
                } else {
                    view_y + viewport_height * 0.25
                };
                if let Some(page) = layout.page_at(position)
                    && page != self.page
                {
                    self.page = page;
                    self.geometry = Some(layout.clone());
                    // Keep filesystem serialization/fsync out of animation frames.
                    self.scroll_save_pending = true;
                }
            }
            if self.rasters_dirty {
                self.generation += 1;
                self.raster_images.clear();
                self.raster_tasks.clear();
                self.rasters_dirty = false;
            }
            let mut visible = layout.visible(view_x, view_y, viewport_width, viewport_height);
            self.page_bounds
                .retain(|index, _| visible.iter().any(|page| page.page == *index));
            visible.sort_by_key(|p| (p.page != self.page, p.page.abs_diff(self.page)));
            self.raster_images
                .retain(|page, _| visible.iter().any(|p| p.page == *page));
            self.raster_tasks
                .retain(|page, _| visible.iter().any(|p| p.page == *page));
            let mut current_raster_width = None;
            for page in &visible {
                self.request_page_text(page.page, cx);
                let raster_width = (page.width * target_width / width * scale_factor)
                    .round()
                    .clamp(64., 8192.) as u32;
                if page.page == self.page {
                    current_raster_width = Some(raster_width);
                }
                self.rasterize_page(page.page, raster_width, cx);
            }
            // Prefetch neighboring pages once the current page is ready so page
            // turns and continuous scrolling resolve directly from cache.
            if !zoom_active
                && let Some(active_width) = current_raster_width
                && self
                    .raster_images
                    .get(&self.page)
                    .is_some_and(|(w, _)| *w == active_width)
            {
                let step = mode.step();
                let mut candidates = [
                    self.page.checked_add(step),
                    self.page.checked_add(step * 2),
                    self.page.checked_sub(step),
                ];
                if mode == LayoutMode::Spread {
                    candidates[1] = self.page.checked_add(step + 1);
                }
                for neighbor in candidates.into_iter().flatten() {
                    if neighbor < doc.pages.len() && !visible.iter().any(|p| p.page == neighbor) {
                        let neighbor_width = if mode == LayoutMode::Horizontal {
                            let first = &doc.pages[0];
                            let p = &doc.pages[neighbor];
                            let strip_h = target_width * first.height / first.width;
                            (strip_h * p.width / p.height * scale_factor)
                                .round()
                                .clamp(64., 8192.) as u32
                        } else {
                            active_width
                        };
                        self.rasterizer
                            .prefetch(doc.clone(), neighbor, neighbor_width, self.dark);
                    }
                }
            }
            self.image = self
                .raster_images
                .get(&self.page)
                .map(|(_, image)| image.clone());
            let mut canvas = div()
                .id("document-canvas")
                .flex_shrink_0()
                .relative()
                .w(px(content_width))
                .h(px(content_height));
            for page in visible {
                canvas = canvas.child(
                    div()
                        .id(("pdf-page", page.page))
                        .absolute()
                        .left(px(center_x + page.x))
                        .top(px(padding + page.y))
                        .w(px(page.width))
                        .h(px(page.height))
                        .child(self.page_element(page.page, page.width, cx))
                        .when(
                            self.config.page_separators && page.page + 1 < doc.pages.len(),
                            |view| {
                                view.child(
                                    div()
                                        .absolute()
                                        .bg(rgb(self.appearance.border))
                                        .when(
                                            mode == LayoutMode::Horizontal
                                                || mode == LayoutMode::Spread,
                                            |line| line.right_0().top_0().w(px(1.)).h_full(),
                                        )
                                        .when(
                                            mode != LayoutMode::Horizontal
                                                && mode != LayoutMode::Spread,
                                            |line| line.bottom_0().left_0().h(px(1.)).w_full(),
                                        ),
                                )
                            },
                        ),
                );
            }
            self.geometry = Some(layout);
            page_view = canvas.into_any_element();
        } else {
            page_view = div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .pt_16()
                .gap_3()
                .child(img("icons/logo.png").size(px(64.)))
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Omapdf"),
                )
                .child(self.tool(
                    "welcome-open",
                    "Open PDF or EPUB",
                    "open_document",
                    false,
                    cx,
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(self.appearance.muted))
                        .child("Ctrl+O"),
                )
                .child(self.tool("welcome-recent", "Recent documents", "recent", false, cx))
                .into_any_element();
        }
        // Cache hits are installed while constructing page_view. Check readiness
        // afterwards so a warm-cache jump starts its fade in this same frame.
        let page_ready = !self.rasters_dirty && self.raster_images.contains_key(&self.page);
        let page_opacity = if let Some(jump) = &mut self.page_jump {
            let ready = page_ready || now.duration_since(jump.created).as_secs_f32() > 1.;
            jump.fade.update(if ready { 1. } else { 0. }, now, enabled)
        } else {
            1.
        };
        if page_opacity >= 1. {
            self.page_jump = None;
        }
        // Keep ticking while awaiting a raster too, so the timeout can release
        // the outgoing snapshot even if rendering fails without another event.
        self.ui_animating |= self.page_jump.is_some();
        self.ui_animating |= self.toast_transition.active(now);
        if self.ui_animating {
            self.request_scroll_frame(window, cx);
        }
        let title = self
            .document
            .as_ref()
            .map(|doc| doc.title.clone())
            .unwrap_or_else(|| "Omapdf".into());
        let position = self
            .document
            .as_ref()
            .map(|doc| {
                if mode == LayoutMode::Spread {
                    let first = self.page / 2 * 2 + 1;
                    let last = (first + 1).min(doc.pages.len());
                    if first == last {
                        format!("{first} / {}", doc.pages.len())
                    } else {
                        format!("{first}–{last} / {}", doc.pages.len())
                    }
                } else {
                    format!("{} / {}", self.page + 1, doc.pages.len())
                }
            })
            .unwrap_or_default();
        let zoom_label = if self.fit_height {
            "Fit page".to_string()
        } else if self.fit {
            "Fit width".to_string()
        } else {
            format!("{:.0}%", self.zoom * 100.)
        };
        let key_feedback = format!("{}{}", self.count, self.sequence.join(""));
        let prompt = self
            .prompt
            .filter(|prompt| {
                *prompt != Prompt::Search && !(self.panel.is_some() && *prompt == Prompt::Filter)
            })
            .map(|prompt| match prompt {
                Prompt::Command => ":",
                Prompt::Search => "/",
                Prompt::Page => "Page: ",
                Prompt::Open => "Open: ",
                Prompt::ExportImage => "New PNG image: ",
                Prompt::Password => "Password (Enter unlock · Esc cancel): ",
                Prompt::EmbedAnnotations => "New annotated PDF: ",
                Prompt::HighlightNote => "Highlight note: ",
                Prompt::ImportLibrary => "Import library: ",
                Prompt::ExportLibrary => "Export library: ",
                Prompt::Bookmark => "Bookmark: ",
                Prompt::SetMark => "Set mark: ",
                Prompt::JumpMark => "Jump mark: ",
                Prompt::Highlight => "Highlight type (a–z / A–Z / _): ",
                Prompt::Filter => "Filter · ↑↓ select · Enter open: ",
                Prompt::Preference => "Value: ",
            });
        div()
            .id("reader")
            .relative()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(self.appearance.background))
            .text_color(rgb(self.appearance.foreground))
            .font_family(self.appearance.font.clone())
            .text_size(px(self.config.font_size))
            .on_key_down(cx.listener(Self::key))
            .when(self.region_mode, |view| view.cursor(CursorStyle::Crosshair))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                if this.region_mode {
                    if event.button == MouseButton::Left
                        && f32::from(event.position.y) >= this.viewport_origin[1]
                    {
                        let point = [f32::from(event.position.x), f32::from(event.position.y)];
                        this.region_drag = Some((point, point));
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.scrollbar_grab.is_some() {
                    this.drag_scrollbar(event.position, cx);
                    return;
                }
                if this.region_mode && this.region_drag.is_some() {
                    if let Ok(region) = this.region_plan(event.position, window.scale_factor()) {
                        this.region_selection =
                            Some((this.document.as_ref().unwrap().id.clone(), region));
                    }
                    cx.notify();
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, window, cx| {
                if event.button == MouseButton::Left && this.scrollbar_grab.take().is_some() {
                    cx.stop_propagation();
                    cx.notify();
                }
                if this.region_mode && event.button == MouseButton::Left {
                    this.finish_region(event.position, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    this.scrollbar_grab = None;
                    if this.region_mode {
                        this.finish_region(event.position, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.selecting = false;
                    cx.notify();
                }),
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                for path in paths.paths() {
                    this.open_additional(path.clone(), cx);
                }
            }))
            .child(
                div()
                    .h(px(36.))
                    .flex_shrink_0()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .bg(rgb(self.appearance.panel))
                    .border_b_1()
                    .border_color(rgb(self.appearance.border))
                    .child(self.tool(
                        "contents-button",
                        "Contents · t",
                        "goto_toc",
                        self.panel == Some(Panel::Toc),
                        cx,
                    ))
                    .child(self.tool(
                        "open-button",
                        "Open PDF or EPUB · Ctrl+O",
                        "open_document",
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pl_2()
                            .text_ellipsis()
                            .text_size(px(self.config.font_size))
                            .child(title),
                    )
                    .child(self.tool(
                        "previous-button",
                        "Previous page",
                        "previous_page",
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .id("page-chip")
                            .px_2()
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .rounded(self.corner_radius(4.))
                            .cursor_pointer()
                            .text_color(rgb(self.appearance.foreground))
                            .child(if position.is_empty() {
                                "—".into()
                            } else {
                                position
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.prompt(Prompt::Page, cx))),
                    )
                    .child(self.tool("next-button", "Next page", "next_page", false, cx))
                    .child(
                        div()
                            .w(px(1.))
                            .h(px(14.))
                            .mx_1()
                            .bg(rgb(self.appearance.border)),
                    )
                    .child(self.tool("zoom-out-button", "Zoom out · −", "zoom_out", false, cx))
                    .child(
                        div()
                            .w(px(68.))
                            .text_center()
                            .text_color(rgb(self.appearance.muted))
                            .text_xs()
                            .child(zoom_label),
                    )
                    .child(self.tool("zoom-in-button", "Zoom in · +", "zoom_in", false, cx))
                    .child(self.tool(
                        "layout-button",
                        "Page layout · L",
                        "layout_menu",
                        self.layout_menu,
                        cx,
                    ))
                    .child(self.tool(
                        "fit-button",
                        "Toggle fit width / page · w / f",
                        "fit_to_page_width",
                        self.fit,
                        cx,
                    ))
                    .child(
                        div()
                            .w(px(1.))
                            .h(px(14.))
                            .mx_1()
                            .bg(rgb(self.appearance.border)),
                    )
                    .child(self.tool(
                        "search-button",
                        "Search · /",
                        "search",
                        self.panel == Some(Panel::Search),
                        cx,
                    ))
                    .child(self.tool(
                        "notes-button",
                        "Highlights · H",
                        "goto_highlight",
                        self.panel == Some(Panel::Highlights),
                        cx,
                    ))
                    .child(self.tool(
                        "theme-button",
                        "Dark pages · F8",
                        "toggle_dark_mode",
                        self.dark || self.config.document_theme,
                        cx,
                    ))
                    .child(self.tool(
                        "settings-button",
                        "Settings · Ctrl+,",
                        "settings",
                        self.panel == Some(Panel::Settings),
                        cx,
                    )),
            )
            .child(
                div().flex().flex_1().min_h_0().child(
                    div()
                        .id("pages")
                        .relative()
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                if this.document.is_some() {
                                    this.open_context(this.page, event.position, cx);
                                }
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .overflow_scroll()
                        .flex_1()
                        .min_w_0()
                        .child(page_view),
                ),
            )
            .when(self.layout_menu || layout_opacity > 0., |view| {
                view.child(
                    div()
                        .id("layout-menu")
                        .occlude()
                        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                        .opacity(layout_opacity)
                        .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            if event.position.y > px(36.) + this.tab_insets().height {
                                this.layout_menu = false;
                                cx.notify();
                            }
                        }))
                        .absolute()
                        .top(px(36. - 5. * (1. - layout_opacity)))
                        .right_3()
                        .w(px(224.))
                        .p_2()
                        .rounded(self.corner_radius(4.))
                        .shadow_lg()
                        .bg(rgb(self.appearance.panel))
                        .border_1()
                        .border_color(rgb(self.appearance.border))
                        .children(
                            LayoutMode::ALL
                                .into_iter()
                                .enumerate()
                                .map(|(index, mode)| {
                                    let hover = self.appearance.surface;
                                    div()
                                        .id(("layout-option", index))
                                        .px_3()
                                        .py_2()
                                        .rounded(self.corner_radius(4.))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .when(index == self.layout_index, |div| {
                                            div.bg(rgb(self.appearance.selection))
                                                .text_color(rgb(self.appearance.accent))
                                        })
                                        .hover(move |style| style.bg(rgb(hover)))
                                        .child(mode.label())
                                        .when(mode == self.config.layout, |div| {
                                            div.child(
                                                svg()
                                                    .path("icons/check.svg")
                                                    .size(px(14.))
                                                    .text_color(rgb(self.appearance.accent)),
                                            )
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.change_setting("page_layout", mode.name(), cx)
                                        }))
                                }),
                        ),
                )
            })
            .when(self.config.scrollbar && self.document.is_some(), |view| {
                let (horizontal, length, thumb, position, limit) = self.scrollbar_metrics();
                if limit <= 0. {
                    return view;
                }
                view.child(
                    div()
                        .id("document-scrollbar")
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            if this.scrollbar_grab.is_some() {
                                this.drag_scrollbar(event.position, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .absolute()
                        .bg(rgb(self.appearance.panel))
                        .when(horizontal, |bar| {
                            bar.left_0().bottom_0().w(px(length)).h(px(12.))
                        })
                        .when(!horizontal, |bar| {
                            bar.right_0().top(px(36.)).h(px(length)).w(px(12.))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                let coordinate = if horizontal {
                                    f32::from(event.position.x)
                                } else {
                                    f32::from(event.position.y) - 66.
                                };
                                this.scrollbar_grab = Some(
                                    if coordinate >= position && coordinate <= position + thumb {
                                        coordinate - position
                                    } else {
                                        thumb / 2.
                                    },
                                );
                                this.drag_scrollbar(event.position, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .absolute()
                                .rounded_md()
                                .bg(rgb(self.appearance.muted))
                                .when(horizontal, |thumb_view| {
                                    thumb_view
                                        .left(px(position))
                                        .top(px(3.))
                                        .w(px(thumb))
                                        .h(px(6.))
                                })
                                .when(!horizontal, |thumb_view| {
                                    thumb_view
                                        .top(px(position))
                                        .left(px(3.))
                                        .h(px(thumb))
                                        .w(px(6.))
                                }),
                        ),
                )
            })
            .when_some(
                if self.helper.is_none() {
                    self.presented_preview.clone()
                } else {
                    None
                },
                |view, (page, image)| {
                    let image_size = image.size(0);
                    let scale = ((viewport_width - 56.).clamp(1., 396.)
                        / image_size.width.0.max(1) as f32)
                        .min((viewport_height - 92.).max(1.) / image_size.height.0.max(1) as f32);
                    let image_width = image_size.width.0 as f32 * scale;
                    let image_height = image_size.height.0 as f32 * scale;
                    view.child(
                        div()
                            .absolute()
                            .opacity(preview_opacity)
                            .right_4()
                            .top(px(64. + 8. * (1. - preview_opacity)))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .w(px(image_width + 24.))
                            .overflow_hidden()
                            .bg(rgb(self.appearance.panel))
                            .p_3()
                            .shadow_lg()
                            .child(format!("Overview · page {} · Escape to close", page + 1))
                            .child(
                                img(image)
                                    .flex_shrink_0()
                                    .w(px(image_width))
                                    .h(px(image_height)),
                            ),
                    )
                },
            )
            .when(self.pending_unlock.is_some(), |view| {
                view.child(
                    div()
                        .id("password-backdrop")
                        .absolute()
                        .size_full()
                        .bg(rgba(0x00000055))
                        .occlude()
                        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                        .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                )
            })
            .when(self.prompt == Some(Prompt::Command), |view| {
                view.child(self.command_box(viewport_width, cx))
            })
            .when(
                prompt.is_some() && self.prompt != Some(Prompt::Command),
                |view| {
                    view.child(
                        div()
                            .absolute()
                            .bottom(px(24.))
                            .left(px((viewport_width - 560.).max(24.) / 2.))
                            .w(px((viewport_width - 48.).min(560.)))
                            .rounded(self.corner_radius(12.))
                            .shadow_lg()
                            .px_4()
                            .py_3()
                            .border_1()
                            .border_color(rgb(self.appearance.border))
                            .bg(rgb(self.appearance.panel))
                            .text_xs()
                            .text_color(rgb(self.appearance.muted))
                            .when(prompt.is_some(), |div| {
                                let entity = cx.entity();
                                let focus = self.focus.clone();
                                div.child(
                                    canvas(
                                        |bounds, _, _| bounds,
                                        move |_, bounds, window, cx| {
                                            window.handle_input(
                                                &focus,
                                                ElementInputHandler::new(bounds, entity.clone()),
                                                cx,
                                            );
                                        },
                                    )
                                    .absolute()
                                    .size_full(),
                                )
                            })
                            .child(if let Some(prefix) = prompt {
                                if self.prompt == Some(Prompt::Password) {
                                    format!(
                                        "{} · {prefix}{}▏{}",
                                        self.status,
                                        "•".repeat(
                                            self.input.text[..self.input.selection.end]
                                                .chars()
                                                .count()
                                        ),
                                        "•".repeat(
                                            self.input.text[self.input.selection.end..]
                                                .chars()
                                                .count()
                                        )
                                    )
                                } else {
                                    format!(
                                        "{prefix}{}▏{}",
                                        &self.input.text[..self.input.selection.end],
                                        &self.input.text[self.input.selection.end..]
                                    )
                                }
                            } else {
                                format!(
                                    "{}   {}{}",
                                    self.status,
                                    self.count,
                                    self.sequence.join("")
                                )
                            }),
                    )
                },
            )
            .when(self.page_jump.is_some(), |view| {
                let jump = self.page_jump.as_ref().unwrap();
                let image = jump.image.clone();
                let bounds = jump.bounds;
                view.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(36.))
                        .w(px(viewport_width))
                        .h(px(viewport_height))
                        .overflow_hidden()
                        .opacity(1. - page_opacity)
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |_, _, window, _| {
                                    let _ = window.paint_image(
                                        bounds,
                                        Corners::default(),
                                        image.clone(),
                                        0,
                                        false,
                                    );
                                },
                            )
                            .size_full(),
                        ),
                )
            })
            .when(
                self.pending_unlock.is_none()
                    && self.presented_panel.is_some()
                    && panel_opacity > 0.,
                |view| view.child(self.panel_dialog(panel_opacity, window, cx)),
            )
            .when(
                self.pending_unlock.is_none()
                    && (self.context_menu.is_some() || self.menu_opacity > 0.),
                |view| view.child(self.context_view(window, cx)),
            )
            .when(
                (toast_opacity > 0. || !key_feedback.is_empty()) && prompt.is_none(),
                |view| {
                    view.child(
                        div()
                            .absolute()
                            .bottom(px(24.))
                            .w_full()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .id("notification-pill")
                                    .occlude()
                                    .max_w(px((viewport_width - 48.).max(80.)))
                                    .px_4()
                                    .py_2()
                                    .rounded(self.corner_radius(18.))
                                    .shadow_lg()
                                    .border_1()
                                    .border_color(rgb(self.appearance.border))
                                    .bg(rgb(self.appearance.surface))
                                    .text_color(rgb(self.appearance.foreground))
                                    .text_xs()
                                    .opacity(if key_feedback.is_empty() {
                                        toast_opacity
                                    } else {
                                        1.
                                    })
                                    .child(if key_feedback.is_empty() {
                                        self.status.clone()
                                    } else {
                                        key_feedback.clone()
                                    }),
                            ),
                    )
                },
            )
            .child({
                let reader = cx.weak_entity();
                canvas(
                    |bounds, _, _| bounds,
                    move |_, bounds, window, _| {
                        let reader = reader.clone();
                        // Capture on the fixed reader viewport, before GPUI's native
                        // overflow listener can apply an immediate wheel jump.
                        window.on_mouse_event(
                            move |event: &ScrollWheelEvent, phase, window, cx| {
                                if phase == DispatchPhase::Capture
                                    && bounds.contains(&event.position)
                                {
                                    let _ = reader.update(cx, |this, cx| {
                                        if this.pending_unlock.is_some() || this.region_mode {
                                            cx.stop_propagation();
                                            return;
                                        }
                                        if this.panel.is_some()
                                            || this.context_menu.is_some()
                                            || this.prompt == Some(Prompt::Command)
                                        {
                                            this.menu_wheel(event, window, cx);
                                            return;
                                        }
                                        let inside_document = event.position.y
                                            >= bounds.origin.y + px(36.)
                                            && event.position.y < bounds.bottom();
                                        if inside_document
                                            && this.panel.is_none()
                                            && this.context_menu.is_none()
                                            && !this.layout_menu
                                        {
                                            this.wheel(event, window, cx);
                                        }
                                    });
                                }
                            },
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            })
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.index_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.raster_tasks.clear();
        if self.document.is_some() || self.library != self.library_baseline {
            self.persist();
        }
    }
}

impl EntityInputHandler for Reader {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        self.prompt?;
        if self.prompt == Some(Prompt::Password) {
            return None;
        }
        let range = self.input.from_utf16(range);
        *actual = Some(self.input.to_utf16(range.clone()));
        Some(self.input.text[range].to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.prompt?;
        Some(UTF16Selection {
            range: self.input.to_utf16(self.input.selection.clone()),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.input
            .marked
            .clone()
            .map(|range| self.input.to_utf16(range))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.input.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.prompt.is_some() {
            if self.prompt == Some(Prompt::Command) {
                self.reset_completion();
            }
            self.input.replace(range, text, false);
            if self.prompt == Some(Prompt::Filter) {
                self.panel_index = 0;
            }
            if self.prompt == Some(Prompt::Search) {
                self.live_search(cx);
            }
            cx.notify();
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.prompt.is_none() {
            return;
        }
        if self.prompt == Some(Prompt::Command) {
            self.reset_completion();
        }
        let start = self.input.replace(range, text, true);
        if let Some(selected) = selected {
            let base = self.input.to_utf16(start..start).start;
            self.input.selection = self
                .input
                .from_utf16(base + selected.start..base + selected.end);
        }
        if self.prompt == Some(Prompt::Search) {
            self.live_search(cx);
        }
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.input.text.encode_utf16().count())
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::{Panel, Prompt, menu_keystroke};
    use gpui::Keystroke;

    #[test]
    fn vim_menu_keys_preserve_text_input_and_normal_mode() {
        for (input, popup, prompt, panel, expected) in [
            ("j", true, None, None, "down"),
            ("k", true, None, None, "up"),
            ("j", false, None, None, "j"),
            ("j", false, Some(Prompt::Filter), Some(Panel::Toc), "j"),
            ("ctrl-j", false, Some(Prompt::Command), None, "down"),
            ("ctrl-k", false, Some(Prompt::Search), None, "up"),
            (
                "ctrl-h",
                false,
                Some(Prompt::Filter),
                Some(Panel::Toc),
                "left",
            ),
            (
                "ctrl-l",
                false,
                Some(Prompt::Filter),
                Some(Panel::Toc),
                "right",
            ),
            ("ctrl-j", false, Some(Prompt::Password), None, "j"),
        ] {
            let key = Keystroke::parse(input).unwrap();
            assert_eq!(menu_keystroke(&key, popup, prompt, panel).key, expected);
        }
    }
}
