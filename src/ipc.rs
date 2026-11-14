//! One primary process per library. Other invocations forward structured requests.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
    time::Duration,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Request {
    pub file: Option<PathBuf>,
    pub page: Option<usize>,
    pub commands: Vec<String>,
}

pub struct Server {
    _lock: File,
    path: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    receiver: Option<Receiver<Request>>,
}

pub enum Instance {
    Primary(Server),
    Forwarded,
}

impl Server {
    pub fn start(library: &Path, request: &Request) -> Result<Instance> {
        let parent = library
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let library = parent
            .canonicalize()?
            .join(library.file_name().context("Library must be a file")?);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(library.with_extension("lock"))?;
        let path = library.with_extension("sock");
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut connection = None;
                for _ in 0..20 {
                    match UnixStream::connect(&path) {
                        Ok(stream) => {
                            connection = Some(stream);
                            break;
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(25)),
                    }
                }
                let mut stream = connection.context(
                    "Another reader owns the library, but its control socket is unavailable",
                )?;
                stream.set_read_timeout(Some(Duration::from_secs(3)))?;
                stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                serde_json::to_writer(&mut stream, request)?;
                stream.shutdown(std::net::Shutdown::Write)?;
                let mut response = [0u8; 2];
                stream.read_exact(&mut response)?;
                anyhow::ensure!(&response == b"OK", "Reader rejected request");
                return Ok(Instance::Forwarded);
            }
            Err(error) => return Err(error.into()),
        }
        if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            anyhow::ensure!(
                metadata.file_type().is_socket(),
                "Control socket path is occupied by a non-socket file: {}",
                path.display()
            );
            std::fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path).context("Cannot create reader control socket")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut data = vec![];
                        if (&mut stream).take(65537).read_to_end(&mut data).is_ok()
                            && data.len() <= 65536
                            && let Ok(request) = serde_json::from_slice(&data)
                            && sender.send(request).is_ok()
                        {
                            let _ = stream.write_all(b"OK");
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Instance::Primary(Self {
            _lock: lock,
            path,
            stop,
            worker: Some(worker),
            receiver: Some(receiver),
        }))
    }
    pub fn take_receiver(&mut self) -> Receiver<Request> {
        self.receiver.take().expect("receiver taken once")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
