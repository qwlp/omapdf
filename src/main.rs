mod helper;
mod reader;
mod software_renderer;
mod workspace;

use clap::Parser;
use gpui::*;
use std::path::PathBuf;

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
    std::thread::spawn(omapdf::appearance::Appearance::load);
    let prepared = request
        .file
        .clone()
        .map(|path| reader::PreparedOpen::new(path, config.epub));
    Application::new()
        .with_assets(omapdf::appearance::Icons)
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![
                    std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/ReaderMono.ttf")),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../assets/fonts/ReaderSans-Regular.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../assets/fonts/ReaderSans-Medium.ttf"
                    )),
                    std::borrow::Cow::Borrowed(include_bytes!(
                        "../assets/fonts/ReaderSans-Bold.ttf"
                    )),
                ])
                .expect("Cannot load bundled UI font");
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let mut initial = vec![request];
            for path in arguments.files.into_iter().skip(1) {
                initial.push(omapdf::ipc::Request {
                    file: Some(path),
                    ..Default::default()
                });
            }
            let handle =
                workspace::open_window_prepared(library, config_path, initial, prepared, cx);
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;
                    for request in requests.try_iter() {
                        let target = cx.update(|cx| {
                            cx.active_window()
                                .and_then(|w| w.downcast::<workspace::Workspace>())
                                .or_else(|| {
                                    cx.windows()
                                        .into_iter()
                                        .find_map(|w| w.downcast::<workspace::Workspace>())
                                })
                                .unwrap_or(handle)
                        });
                        if let Ok(target) = target {
                            let _ = target.update(cx, |workspace, window, cx| {
                                workspace.receive(request, window, cx)
                            });
                        }
                    }
                }
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}
