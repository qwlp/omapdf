use crate::reader::{PreparedOpen, Reader, Startup};
use gpui::{prelude::*, *};
use omapdf::{config::Config, ipc::Request, state::Library};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::mpsc};

actions!(workspace, [NextTab, PreviousTab, CloseTab]);

pub(crate) const TAB_SIDEBAR_WIDTH: f32 = 192.;
pub(crate) const TAB_BAR_HEIGHT: f32 = 32.;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Session {
    paths: Vec<PathBuf>,
    active: usize,
}

impl Session {
    fn from_tabs(tabs: impl Iterator<Item = Option<PathBuf>>, active: usize) -> Self {
        let mut session = Self::default();
        for (index, path) in tabs.enumerate() {
            if let Some(path) = path {
                if index <= active {
                    session.active = session.paths.len();
                }
                session.paths.push(path);
            }
        }
        session
    }

    fn restore(path: &std::path::Path) -> Self {
        let Some(session): Option<Self> = fs::read(path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
        else {
            return Self::default();
        };
        Self::from_tabs(
            session
                .paths
                .into_iter()
                .map(|path| path.is_file().then_some(path)),
            session.active,
        )
    }

    fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        fs::create_dir_all(parent)?;
        let file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(file.as_file(), self)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
}

pub struct Workspace {
    tabs: Vec<(Entity<Reader>, mpsc::Sender<Request>)>,
    active: usize,
    library: PathBuf,
    config: PathBuf,
    opens: mpsc::Sender<Request>,
    pending: mpsc::Receiver<Request>,
    session_path: Option<PathBuf>,
    saved_session: Option<Session>,
    tab_scroll: ScrollHandle,
    horizontal_tabs: bool,
}

pub fn open_window(
    library: PathBuf,
    config: PathBuf,
    requests: Vec<Request>,
    cx: &mut App,
) -> WindowHandle<Workspace> {
    open_workspace(library, config, requests, None, false, cx)
}

pub fn open_window_prepared(
    library: PathBuf,
    config: PathBuf,
    requests: Vec<Request>,
    prepared: Option<PreparedOpen>,
    cx: &mut App,
) -> WindowHandle<Workspace> {
    open_workspace(library, config, requests, prepared, true, cx)
}

fn open_workspace(
    library: PathBuf,
    config: PathBuf,
    requests: Vec<Request>,
    mut prepared: Option<PreparedOpen>,
    primary: bool,
    cx: &mut App,
) -> WindowHandle<Workspace> {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(900.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Omapdf".into()),
                ..Default::default()
            }),
            app_id: Some("io.omapdf".into()),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                let (opens, pending) = mpsc::channel();
                let session_path = primary.then(|| {
                    let mut path = library.as_os_str().to_os_string();
                    path.push(".tabs.json");
                    PathBuf::from(path)
                });
                let restored = session_path
                    .as_ref()
                    .filter(|_| requests.iter().all(|request| request.file.is_none()))
                    .map(|path| Session::restore(path))
                    .unwrap_or_default();
                let mut workspace = Workspace {
                    tabs: vec![],
                    active: 0,
                    library,
                    config,
                    opens,
                    pending,
                    session_path,
                    saved_session: None,
                    tab_scroll: ScrollHandle::new(),
                    horizontal_tabs: false,
                };
                for path in restored.paths {
                    workspace.add(
                        Request {
                            file: Some(path),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                }
                if !workspace.tabs.is_empty() {
                    workspace.activate(restored.active, window, cx);
                }
                for request in requests {
                    if workspace.tabs.is_empty() {
                        workspace.add_prepared(request, prepared.take(), window, cx);
                    } else {
                        workspace.receive(request, window, cx);
                    }
                }
                if workspace.tabs.is_empty() {
                    workspace.add(Request::default(), window, cx);
                }
                workspace.save_session(cx);
                let executor = cx.background_executor().clone();
                let handle = window.window_handle();
                cx.spawn(async move |this, cx| {
                    loop {
                        executor.timer(std::time::Duration::from_millis(100)).await;
                        if handle
                            .update(cx, |_, window, cx| {
                                this.update(cx, |this, cx| {
                                    let requests: Vec<_> = this.pending.try_iter().collect();
                                    for request in requests {
                                        this.receive(request, window, cx);
                                    }
                                })
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .detach();
                workspace
            })
        },
    )
    .expect("Cannot open reader window")
}

impl Workspace {
    fn add(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        self.add_prepared(request, None, window, cx);
    }

    fn add_prepared(
        &mut self,
        request: Request,
        prepared: Option<PreparedOpen>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (sender, requests) = mpsc::channel();
        let config = Config::load(&self.config).unwrap_or_default();
        let reader = cx.new(|cx| {
            Reader::new(
                Startup {
                    library: Library::load(&self.library).unwrap_or_default(),
                    library_path: self.library.clone(),
                    config,
                    file: request.file,
                    page: request.page,
                    commands: request.commands,
                    requests,
                    prepared,
                },
                window,
                cx,
            )
        });
        reader.update(cx, |reader, _| reader.opens = Some(self.opens.clone()));
        cx.observe(&reader, |this, _, cx| {
            this.save_session(cx);
            cx.notify();
        })
        .detach();
        self.tabs.push((reader, sender));
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        cx.notify();
    }

    pub fn receive(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        if request.file.is_none() {
            let _ = self.tabs[self.active].1.send(request);
        } else {
            let new_windows =
                Config::load(&self.config).is_ok_and(|c| c.value("open_in_new_windows") == "1");
            let empty = self.tabs.len() == 1 && self.tabs[0].0.read(cx).is_empty();
            if empty {
                self.tabs[0]
                    .0
                    .update(cx, |reader, cx| reader.receive_request(request, cx));
            } else if new_windows {
                open_window(self.library.clone(), self.config.clone(), vec![request], cx);
                return;
            } else {
                self.add(request, window, cx);
            }
        }
        self.save_session(cx);
        window.activate_window();
        cx.notify();
    }

    fn save_session(&mut self, cx: &App) {
        let Some(path) = &self.session_path else {
            return;
        };
        let session = Session::from_tabs(
            self.tabs
                .iter()
                .map(|(reader, _)| reader.read(cx).tab_path().cloned()),
            self.active,
        );
        if self.saved_session.as_ref() == Some(&session) {
            return;
        }
        match session.save(path) {
            Ok(()) => self.saved_session = Some(session),
            Err(error) => eprintln!("Could not save tabs to {}: {error}", path.display()),
        }
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = index;
        self.tabs[index].0.read(cx).focus_tab(window);
        self.tab_scroll.scroll_to_item(index);
        self.save_session(cx);
        cx.notify();
    }

    fn cycle(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.tabs.len();
        if len > 1 {
            let step = if backwards { len - 1 } else { 1 };
            self.activate((self.active + step) % len, window, cx);
        }
    }

    fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.save_session(cx);
            window.remove_window();
        } else {
            let active = self.active.saturating_sub(usize::from(index < self.active));
            self.activate(active.min(self.tabs.len() - 1), window, cx);
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tabs.is_empty() {
            return div().into_any_element();
        }
        let reader = self.tabs[self.active].0.read(cx);
        let horizontal = reader.horizontal_tabs();
        if self.horizontal_tabs != horizontal {
            self.horizontal_tabs = horizontal;
            self.tab_scroll = ScrollHandle::new();
            self.tab_scroll.scroll_to_item(self.active);
        }
        let (theme, font_size) = reader.ui_appearance();
        let theme = theme.clone();
        let bar =
            div()
                .id("document-tabs")
                .flex()
                .flex_shrink_0()
                .when(horizontal, |bar| {
                    bar.items_center()
                        .px_2()
                        .w_full()
                        .h(px(TAB_BAR_HEIGHT))
                        .overflow_x_scroll()
                        .border_b_1()
                })
                .when(!horizontal, |bar| {
                    bar.p_2()
                        .flex_col()
                        .w(px(TAB_SIDEBAR_WIDTH))
                        .h_full()
                        .overflow_y_scroll()
                        .border_r_1()
                })
                .track_scroll(&self.tab_scroll)
                .gap_1()
                .border_color(rgb(theme.border))
                .bg(rgb(theme.panel))
                .text_color(rgb(theme.foreground))
                .children(self.tabs.iter().enumerate().map(|(index, (reader, _))| {
                    let title = reader.read(cx).tab_title();
                    div()
                        .id(("tab", index))
                        .flex_shrink_0()
                        .flex()
                        .gap_2()
                        .px_2()
                        .h(px(if horizontal { 24. } else { 32. }))
                        .when(horizontal, |tab| tab.w(px(TAB_SIDEBAR_WIDTH)))
                        .rounded(px(4.))
                        .items_center()
                        .cursor_pointer()
                        .when(index == self.active, |row| {
                            row.bg(rgb(theme.selection)).text_color(rgb(theme.accent))
                        })
                        .hover({
                            let surface = theme.surface;
                            move |row| row.bg(rgb(surface))
                        })
                        .child(div().flex_1().min_w_0().truncate().child(title))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.activate(index, window, cx);
                        }))
                        .child(div().id(("close-tab", index)).px_1().child("×").on_click(
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(index, window, cx);
                            }),
                        ))
                }));
        div()
            .size_full()
            .font_family(theme.font.clone())
            .text_size(px(font_size))
            .flex()
            .when(horizontal, |workspace| workspace.flex_col())
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.cycle(false, window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.cycle(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &CloseTab, window, cx| this.close(this.active, window, cx)),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.control {
                    match event.keystroke.key.as_str() {
                        "tab" => this.cycle(event.keystroke.modifiers.shift, window, cx),
                        "w" => this.close(this.active, window, cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }
            }))
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.tabs[self.active].0.clone()),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use omapdf::config::{Binding, Config};
    use std::fs;

    #[test]
    fn vim_tab_sequences_share_the_reader_keymap() {
        let config = Config::default();
        assert!(matches!(config.binding(&["g".into()]), Binding::Prefix));
        for (key, expected) in [("t", "next_tab"), ("T", "previous_tab"), ("q", "close_tab")] {
            assert!(
                matches!(config.binding(&["g".into(), key.into()]), Binding::Exact(command) if command == expected)
            );
        }
        assert!(matches!(
            config.binding(&["g".into(), "g".into()]),
            Binding::Exact("goto_beginning")
        ));
    }

    #[test]
    fn session_preserves_tab_order_and_selection_and_skips_missing_documents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("library.json.tabs.json");
        let first = directory.path().join("first.pdf");
        let second = directory.path().join("second.epub");
        fs::write(&first, []).unwrap();
        fs::write(&second, []).unwrap();
        let session = Session::from_tabs(
            [None, Some(first.clone()), Some(second.clone())].into_iter(),
            2,
        );
        session.save(&path).unwrap();
        assert_eq!(Session::restore(&path), session);
        fs::remove_file(first).unwrap();
        assert_eq!(
            Session::restore(&path),
            Session {
                paths: vec![second],
                active: 0
            }
        );
        Session::default().save(&path).unwrap();
        assert_eq!(Session::restore(&path), Session::default());
        fs::write(&path, b"{ incomplete").unwrap();
        assert_eq!(Session::restore(&path), Session::default());
    }
}
