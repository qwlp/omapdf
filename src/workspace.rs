use crate::reader::{Reader, Startup};
use gpui::{prelude::*, *};
use omapdf::{config::Config, ipc::Request, state::Library};
use std::{path::PathBuf, sync::mpsc};

pub struct Workspace {
    tabs: Vec<(Entity<Reader>, mpsc::Sender<Request>)>,
    active: usize,
    library: PathBuf,
    config: PathBuf,
    opens: mpsc::Sender<Request>,
    pending: mpsc::Receiver<Request>,
}

pub fn open_window(
    library: PathBuf,
    config: PathBuf,
    requests: Vec<Request>,
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
                let mut workspace = Workspace {
                    tabs: vec![],
                    active: 0,
                    library,
                    config,
                    opens,
                    pending,
                };
                for request in requests {
                    if workspace.tabs.is_empty() {
                        workspace.add(request, window, cx);
                    } else {
                        workspace.receive(request, window, cx);
                    }
                }
                if workspace.tabs.is_empty() {
                    workspace.add(Request::default(), window, cx);
                }
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
                },
                window,
                cx,
            )
        });
        reader.update(cx, |reader, _| reader.opens = Some(self.opens.clone()));
        cx.observe(&reader, |_, _, cx| cx.notify()).detach();
        self.tabs.push((reader, sender));
        self.active = self.tabs.len() - 1;
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
        window.activate_window();
        cx.notify();
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tabs.is_empty() {
            return div().into_any_element();
        }
        let (theme, font_size) = self.tabs[self.active].0.read(cx).ui_appearance();
        let theme = theme.clone();
        let mut bar = div()
            .id("document-tabs")
            .overflow_x_scroll()
            .flex()
            .h(px(30.))
            .bg(rgb(theme.panel))
            .text_color(rgb(theme.foreground));
        for (index, (reader, _)) in self.tabs.iter().enumerate() {
            bar =
                bar.child(
                    div()
                        .id(("tab", index))
                        .flex_shrink_0()
                        .flex()
                        .gap_2()
                        .px_3()
                        .items_center()
                        .cursor_pointer()
                        .bg(rgb(if index == self.active {
                            theme.surface
                        } else {
                            theme.panel
                        }))
                        .child(reader.read(cx).tab_title())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.active = index;
                            this.tabs[index].0.read(cx).focus_tab(window);
                            cx.notify();
                        }))
                        .child(div().id(("close-tab", index)).px_1().child("×").on_click(
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.tabs.remove(index);
                                if this.tabs.is_empty() {
                                    window.remove_window();
                                } else {
                                    this.active = this.active.min(this.tabs.len() - 1);
                                    this.tabs[this.active].0.read(cx).focus_tab(window);
                                }
                                cx.notify();
                            }),
                        )),
                );
        }
        div()
            .size_full()
            .font_family(theme.font.clone())
            .text_size(px(font_size))
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.control && event.keystroke.key == "tab" {
                    let step = if event.keystroke.modifiers.shift {
                        this.tabs.len() - 1
                    } else {
                        1
                    };
                    this.active = (this.active + step) % this.tabs.len();
                    this.tabs[this.active].0.read(cx).focus_tab(window);
                    cx.stop_propagation();
                    cx.notify();
                } else if event.keystroke.modifiers.control && event.keystroke.key == "w" {
                    this.tabs.remove(this.active);
                    if this.tabs.is_empty() {
                        window.remove_window();
                    } else {
                        this.active = this.active.min(this.tabs.len() - 1);
                        this.tabs[this.active].0.read(cx).focus_tab(window);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.tabs[self.active].0.clone()),
            )
            .into_any_element()
    }
}
