mod helper;
mod reader;
mod software_renderer;
mod workspace;

use clap::Parser;
use gpui::*;
use std::{path::PathBuf, sync::Arc};

#[derive(Parser)]
#[command(name = "Omapdf", bin_name = "omapdf", version, about)]
struct Arguments {
    /// PDF or EPUB to open
    files: Vec<PathBuf>,
    /// Initial page (one based)
    #[arg(long)]
    page: Option<usize>,
    /// Inspect a PDF or EPUB without starting the GUI
    #[arg(long)]
    inspect: bool,
    /// Use a separate library file
    #[arg(long)]
    library: Option<PathBuf>,
    /// Directory containing keys_user.config and prefs_user.config
    #[arg(long)]
    config: Option<PathBuf>,
    /// Execute a command after the document loads (repeatable)
    #[arg(long = "execute-command")]
    commands: Vec<String>,
    /// Stay resident in the background after closing windows for instant document opens
    #[arg(long)]
    daemon: bool,
}

fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    if arguments.inspect {
        let epub = arguments
            .config
            .as_ref()
            .map(|directory| omapdf::config::Config::load(directory).map(|config| config.epub))
            .transpose()?
            .unwrap_or_default();
        let document = omapdf::document::Document::open_with_options(
            arguments
                .files
                .first()
                .map(|p| p.as_path())
                .ok_or_else(|| anyhow::anyhow!("--inspect requires a file"))?,
            epub,
        )?;
        println!(
            "{}\n{} pages\nSHA256 {}",
            document.title,
            document.pages.len(),
            document.id
        );
        return Ok(());
    }
    let library = arguments.library.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omapdf/library.json")
    });
    let request = omapdf::ipc::Request {
        file: arguments
            .files
            .first()
            .map(|p| p.canonicalize())
            .transpose()?,
        page: arguments.page.map(|p| p.saturating_sub(1)),
        commands: arguments.commands.clone(),
    };
    if let Some(mut command) = software_renderer::fallback_command(&library)? {
        use std::os::unix::process::CommandExt;
        // Restart before acquiring the primary-instance lock or accepting IPC.
        return Err(command.exec().into());
    }
    let mut server = match omapdf::ipc::Server::start(&library, &request)? {
        omapdf::ipc::Instance::Forwarded => {
            for file in arguments.files.iter().skip(1) {
                omapdf::ipc::Server::start(
                    &library,
                    &omapdf::ipc::Request {
                        file: Some(file.canonicalize()?),
                        ..Default::default()
                    },
                )?;
            }
            return Ok(());
        }
        omapdf::ipc::Instance::Primary(server) => server,
    };
    let requests = server.take_receiver();

    let config_path = arguments.config.unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omapdf")
    });
    let config = omapdf::config::Config::load(&config_path)?;
    let prepared_host_font = std::thread::spawn(|| {
        let appearance = omapdf::appearance::Appearance::load();
        appearance
            .font_file()
            .and_then(|path| std::fs::read(path).ok())
    });
    let rasterizer = Arc::new(omapdf::raster::Rasterizer::new());
    rasterizer.set_contrast(config.dark_contrast);
    rasterizer.set_quality(config.render_scale);
    rasterizer.set_sharpness(config.render_sharpness);
    let prepared_library = {
        let library_path = library.clone();
        std::thread::spawn(move || omapdf::state::Library::load(&library_path).unwrap_or_default())
    };
    let prepared = request.file.clone().map(|path| {
        reader::PreparedOpen::new(
            path,
            request.page,
            config.clone(),
            library.clone(),
            rasterizer.clone(),
        )
    });
    let daemon = arguments.daemon;
    let has_initial = request.file.is_some() || !request.commands.is_empty();
    Application::new()
        .with_assets(omapdf::appearance::Icons)
        .run(move |cx: &mut App| {
            let mut startup_fonts = vec![
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/ReaderMono.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/ReaderSans-Regular.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/ReaderSans-Medium.ttf").as_slice(),
                ),
                std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/ReaderSans-Bold.ttf").as_slice(),
                ),
            ];
            if let Ok(Some(bytes)) = prepared_host_font.join() {
                startup_fonts.push(std::borrow::Cow::Owned(bytes));
            }
            cx.text_system()
                .add_fonts(startup_fonts)
                .expect("Cannot load bundled UI font");
            cx.on_window_closed(move |cx| {
                if !daemon && cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let initial_library = prepared_library.join().ok();
            let mut initial_handle: Option<WindowHandle<workspace::Workspace>> = None;
            if !daemon || has_initial {
                let mut initial = vec![request];
                for path in arguments.files.into_iter().skip(1) {
                    initial.push(omapdf::ipc::Request {
                        file: Some(path),
                        ..Default::default()
                    });
                }
                initial_handle = Some(workspace::open_window_prepared(
                    library.clone(),
                    config_path.clone(),
                    config.clone(),
                    initial_library,
                    rasterizer.clone(),
                    initial,
                    prepared,
                    cx,
                ));
            }
            let ipc_library = library.clone();
            let ipc_config_path = config_path.clone();
            let ipc_rasterizer = rasterizer.clone();
            cx.spawn(async move |cx| {
                while let Some(request) = requests.recv_async().await {
                    let mut pending = vec![request];
                    pending.extend(requests.try_iter());
                    let _ = cx.update(|cx| {
                        for request in pending {
                            let target = cx
                                .active_window()
                                .and_then(|w| w.downcast::<workspace::Workspace>())
                                .or_else(|| {
                                    cx.windows()
                                        .into_iter()
                                        .find_map(|w| w.downcast::<workspace::Workspace>())
                                })
                                .or(initial_handle);
                            let delivered = target.is_some_and(|handle| {
                                handle
                                    .update(cx, |workspace, window, cx| {
                                        workspace.receive(request.clone(), window, cx)
                                    })
                                    .is_ok()
                            });
                            if !delivered {
                                if request.file.is_none()
                                    && request.commands.iter().any(|c| {
                                        c.split(';').any(|part| matches!(part.trim(), "q" | "quit"))
                                    })
                                {
                                    cx.quit();
                                    return;
                                }
                                let fresh_config = omapdf::config::Config::load(&ipc_config_path)
                                    .unwrap_or_default();
                                let handle = workspace::open_window_prepared(
                                    ipc_library.clone(),
                                    ipc_config_path.clone(),
                                    fresh_config,
                                    None,
                                    ipc_rasterizer.clone(),
                                    vec![request],
                                    None,
                                    cx,
                                );
                                initial_handle = Some(handle);
                            }
                        }
                    });
                }
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}
