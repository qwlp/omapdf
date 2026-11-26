//! One primary process per library. Other invocations forward structured requests.
use anyhow::{Context, Result};
use parking_lot::{Condvar, Mutex};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    future::Future,
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context as TaskContext, Poll, Waker},
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct ChannelState<T> {
    queue: VecDeque<T>,
    waker: Option<Waker>,
    closed: bool,
}

/// Zero-polling multi-producer single-consumer channel supporting both
/// blocking `recv_timeout` (for tests/CLI) and async `recv_async` (for GPUI).
pub struct SignalSender<T> {
    shared: Arc<(Mutex<ChannelState<T>>, Condvar)>,
}

impl<T> Clone for SignalSender<T> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> SignalSender<T> {
    pub fn send(&self, value: T) -> std::result::Result<(), T> {
        let (lock, condvar) = &*self.shared;
        let waker = {
            let mut state = lock.lock();
            if state.closed {
                return Err(value);
            }
            state.queue.push_back(value);
            state.waker.take()
        };
        condvar.notify_one();
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
}

pub struct SignalReceiver<T> {
    shared: Arc<(Mutex<ChannelState<T>>, Condvar)>,
}

impl<T> Drop for SignalReceiver<T> {
    fn drop(&mut self) {
        let (lock, condvar) = &*self.shared;
        lock.lock().closed = true;
        condvar.notify_all();
    }
}

impl<T> SignalReceiver<T> {
    pub fn try_recv(&self) -> Option<T> {
        self.shared.0.lock().queue.pop_front()
    }

    pub fn try_iter(&self) -> Vec<T> {
        self.shared.0.lock().queue.drain(..).collect()
    }

    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> std::result::Result<T, mpsc::RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        let (lock, condvar) = &*self.shared;
        let mut state = lock.lock();
        loop {
            if let Some(item) = state.queue.pop_front() {
                return Ok(item);
            }
            if Arc::strong_count(&self.shared) == 1 {
                return Err(mpsc::RecvTimeoutError::Disconnected);
            }
            if condvar.wait_until(&mut state, deadline).timed_out() {
                return state
                    .queue
                    .pop_front()
                    .ok_or(mpsc::RecvTimeoutError::Timeout);
            }
        }
    }

    pub fn recv_async(&self) -> RecvFuture<'_, T> {
        RecvFuture { receiver: self }
    }
}

pub struct RecvFuture<'a, T> {
    receiver: &'a SignalReceiver<T>,
}

impl<T> Future for RecvFuture<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let mut state = self.receiver.shared.0.lock();
        if let Some(item) = state.queue.pop_front() {
            return Poll::Ready(Some(item));
        }
        if Arc::strong_count(&self.receiver.shared) == 1 {
            return Poll::Ready(None);
        }
        state.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

pub fn signal_channel<T>() -> (SignalSender<T>, SignalReceiver<T>) {
    let shared = Arc::new((
        Mutex::new(ChannelState {
            queue: VecDeque::new(),
            waker: None,
            closed: false,
        }),
        Condvar::new(),
    ));
    (
        SignalSender {
            shared: shared.clone(),
        },
        SignalReceiver { shared },
    )
}

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
    receiver: Option<SignalReceiver<Request>>,
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
                for _ in 0..100 {
                    match UnixStream::connect(&path) {
                        Ok(stream) => {
                            connection = Some(stream);
                            break;
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
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
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let (sender, receiver) = signal_channel();
        let worker = std::thread::Builder::new()
            .name("ipc-server".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if thread_stop.load(Ordering::Acquire) {
                                break;
                            }
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
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .context("Cannot start IPC listener thread")?;
        Ok(Instance::Primary(Self {
            _lock: lock,
            path,
            stop,
            worker: Some(worker),
            receiver: Some(receiver),
        }))
    }
    pub fn take_receiver(&mut self) -> SignalReceiver<Request> {
        self.receiver.take().expect("receiver taken once")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Unblock the blocking accept() immediately on shutdown.
        let _ = UnixStream::connect(&self.path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
