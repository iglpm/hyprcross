//! The warm container's agent: hyprcross itself, running inside the container, starting
//! commands for the hyprcross on the host over a Unix socket in a mounted directory. A build then
//! costs a socket connection and a fork, not a `podman exec` (≈130 ms) or a `docker exec`.
//!
//! Frames are a tag byte, a little-endian `u32` length and the payload. The client sends one
//! `REQUEST` (JSON), then `STDIN` data and `STDIN_EOF` if it forwards its input; the agent sends
//! `STDOUT` and `STDERR` data, then `EXIT` with the status, or `FAILED` with why nothing ran.
//! The agent kills the command's process group when the client goes away (Ctrl+C on the host).

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Bumped on any change to frames or `Request`; a mismatch makes the host recreate the container.
pub const PROTOCOL: u32 = 1;

const REQUEST: u8 = b'R';
const STDIN: u8 = b'I';
const STDIN_EOF: u8 = b'E';
const STDOUT: u8 = b'O';
const STDERR: u8 = b'e';
const EXIT: u8 = b'X';
const FAILED: u8 = b'F';

const MAX_FRAME: usize = 64 << 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub protocol: u32,
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: String,
    /// Run on a pseudo-terminal of this size (rows, columns): progress bars and colours, with
    /// stdout and stderr merged as a terminal shows them.
    pub pty: Option<(u16, u16)>,
    /// Whether the client forwards its stdin; the command gets an empty one otherwise.
    pub stdin: bool,
}

fn write_frame(w: &mut impl Write, tag: u8, data: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(5 + data.len());
    frame.push(tag);
    frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
    frame.extend_from_slice(data);
    w.write_all(&frame)
}

/// The next frame; `None` when the other side closed the connection.
fn read_frame(r: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut header = [0u8; 5];
    match r.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "agent frame too large",
        ));
    }
    let mut data = vec![0; len];
    r.read_exact(&mut data)?;
    Ok(Some((header[0], data)))
}

// ------------------------------------------------------------------------------------- agent

/// Serve requests on `socket` until no command has run for `idle` (forever for zero).
pub fn serve(socket: &Path, idle: Duration) -> io::Result<()> {
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)?;
    let activity = Arc::new(Activity {
        running: AtomicUsize::new(0),
        last: Mutex::new(Instant::now()),
    });

    if !idle.is_zero() {
        let activity = activity.clone();
        let socket = socket.to_path_buf();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(idle.min(Duration::from_secs(30)));
                let last = *activity.last.lock().unwrap_or_else(|e| e.into_inner());
                if activity.running.load(Ordering::SeqCst) == 0 && last.elapsed() >= idle {
                    // The container stops with its agent; the next build starts it again.
                    let _ = std::fs::remove_file(&socket);
                    std::process::exit(0);
                }
            }
        });
    }

    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let activity = activity.clone();
        activity.running.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            let _ = handle(conn);
            *activity.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            activity.running.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}

struct Activity {
    running: AtomicUsize,
    last: Mutex<Instant>,
}

type Output = (u8, Box<dyn Read + Send>);

fn handle(stream: UnixStream) -> io::Result<()> {
    let mut reader = stream.try_clone()?;
    let writer = Arc::new(Mutex::new(stream));
    let send = |tag: u8, data: &[u8]| {
        let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
        write_frame(&mut *w, tag, data)
    };

    let Some((REQUEST, body)) = read_frame(&mut reader)? else {
        return Ok(());
    };
    let req: Request = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return send(FAILED, format!("bad request: {e}").as_bytes()),
    };
    if req.protocol != PROTOCOL {
        let msg = format!("agent speaks protocol {PROTOCOL}, not {}", req.protocol);
        return send(FAILED, msg.as_bytes());
    }

    let mut cmd = Command::new(&req.program);
    cmd.args(&req.args).current_dir(&req.cwd);
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    let spawned = match req.pty {
        Some((rows, cols)) => spawn_pty(cmd, rows, cols),
        None => spawn_pipes(cmd, req.stdin),
    };
    let (mut child, outputs, mut input) = match spawned {
        Ok(s) => s,
        Err(e) => {
            return send(
                FAILED,
                format!("cannot run `{}`: {e}", req.program).as_bytes(),
            );
        }
    };
    let group = child.id() as libc::pid_t;
    let done = Arc::new(AtomicBool::new(false));

    let readers = outputs.len();
    let (drained_tx, drained) = mpsc::channel();
    for (tag, mut out) in outputs {
        let writer = writer.clone();
        let drained_tx = drained_tx.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                match out.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
                        if write_frame(&mut *w, tag, &buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = drained_tx.send(());
        });
    }
    drop(drained_tx);

    {
        let done = done.clone();
        std::thread::spawn(move || {
            loop {
                match read_frame(&mut reader) {
                    Ok(Some((STDIN, data))) => {
                        if let Some(i) = input.as_mut()
                            && i.write_all(&data).and_then(|_| i.flush()).is_err()
                        {
                            input = None;
                        }
                    }
                    Ok(Some((STDIN_EOF, _))) => input = None,
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => {
                        // The client is gone: so is the reason to run the command.
                        if !done.load(Ordering::SeqCst) {
                            // SAFETY: plain signal to the command's own process group.
                            unsafe { libc::kill(-group, libc::SIGKILL) };
                        }
                        return;
                    }
                }
            }
        });
    }

    let status = child.wait()?;
    done.store(true, Ordering::SeqCst);
    // Output the command wrote just before exiting is still in the pipes. Something it left
    // running in the background may keep them open, so don't wait for that forever.
    let deadline = Instant::now() + Duration::from_millis(500);
    for _ in 0..readers {
        let left = deadline.saturating_duration_since(Instant::now());
        if drained.recv_timeout(left).is_err() {
            break;
        }
    }
    let code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1));
    send(EXIT, &code.to_le_bytes())
}

type Spawned = (Child, Vec<Output>, Option<Box<dyn Write + Send>>);

fn spawn_pipes(mut cmd: Command, stdin: bool) -> io::Result<Spawned> {
    cmd.stdin(if stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = cmd.spawn()?;
    let out = child.stdout.take().expect("piped");
    let err = child.stderr.take().expect("piped");
    let input = child
        .stdin
        .take()
        .map(|i| Box::new(i) as Box<dyn Write + Send>);
    Ok((
        child,
        vec![(STDOUT, Box::new(out)), (STDERR, Box::new(err))],
        input,
    ))
}

fn spawn_pty(mut cmd: Command, rows: u16, cols: u16) -> io::Result<Spawned> {
    use std::fs::File;
    use std::os::fd::FromRawFd;

    let (mut master, mut slave) = (0, 0);
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty writes two descriptors; the name and termios pointers may be null. The
    // pointers are `*mut` on macOS and `*const` on Linux: mutable ones coerce to both.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: both descriptors were just opened and are owned here from now on.
    let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
    for fd in [&master, &slave] {
        use std::os::fd::AsRawFd;
        // SAFETY: setting close-on-exec on a descriptor we own.
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    // The host's terminal already echoes what is typed.
    {
        use std::os::fd::AsRawFd;
        // SAFETY: termios is plain data; tcgetattr fills it in.
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        unsafe {
            if libc::tcgetattr(slave.as_raw_fd(), &mut t) == 0 {
                t.c_lflag &= !(libc::ECHO | libc::ECHONL);
                libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &t);
            }
        }
    }

    cmd.stdin(slave.try_clone()?)
        .stdout(slave.try_clone()?)
        .stderr(slave.try_clone()?);
    // SAFETY: setsid and ioctl are async-signal-safe; the child makes the terminal its own.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    // Only the child may hold the terminal's other end, or reading never ends.
    drop(cmd);
    drop(slave);
    let reader = master.try_clone()?;
    Ok((
        child,
        vec![(STDOUT, Box::new(reader))],
        Some(Box::new(master)),
    ))
}

// ------------------------------------------------------------------------------------ client

/// Connect to the agent; an error means it isn't running (yet).
pub fn connect(socket: &Path) -> io::Result<UnixStream> {
    UnixStream::connect(socket)
}

/// Run `req` on the agent behind `stream`, copying its output here; the command's exit code.
pub fn call(stream: UnixStream, req: &Request) -> io::Result<i32> {
    let mut w = stream.try_clone()?;
    let body = serde_json::to_vec(req).map_err(io::Error::other)?;
    // Not even the request got through: nothing ran.
    write_frame(&mut w, REQUEST, &body)
        .map_err(|e| io::Error::new(io::ErrorKind::NotConnected, e))?;

    if req.stdin {
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            let mut stdin = io::stdin();
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = write_frame(&mut w, STDIN_EOF, &[]);
                        return;
                    }
                    Ok(n) => {
                        if write_frame(&mut w, STDIN, &buf[..n]).is_err() {
                            return;
                        }
                    }
                }
            }
        });
    }

    let mut r = io::BufReader::with_capacity(64 * 1024, stream);
    let (mut out, mut err) = (io::stdout(), io::stderr());
    let mut answered = false;
    loop {
        let frame = match read_frame(&mut r) {
            Ok(f) => f,
            // Reset before a word: the agent was going away; nothing ran.
            Err(e) if !answered => return Err(io::Error::new(io::ErrorKind::NotConnected, e)),
            Err(e) => return Err(e),
        };
        if frame.is_none() && !answered {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the container's agent closed the connection before answering",
            ));
        }
        answered = true;
        match frame {
            // A reader that went away (`| head`) doesn't change how the build ends.
            Some((STDOUT, data)) => {
                let _ = out.write_all(&data).and_then(|_| out.flush());
            }
            Some((STDERR, data)) => {
                let _ = err.write_all(&data).and_then(|_| err.flush());
            }
            Some((EXIT, data)) if data.len() == 4 => {
                return Ok(i32::from_le_bytes([data[0], data[1], data[2], data[3]]));
            }
            Some((FAILED, data)) => {
                return Err(io::Error::other(
                    String::from_utf8_lossy(&data).into_owned(),
                ));
            }
            Some(_) => {}
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "the container's agent closed the connection",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("agent.sock");
        let s = socket.clone();
        std::thread::spawn(move || serve(&s, Duration::ZERO));
        for _ in 0..200 {
            if connect(&socket).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        (dir, socket)
    }

    fn request(program: &str, args: &[&str]) -> Request {
        Request {
            protocol: PROTOCOL,
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: vec![("HYPRCROSS_TEST".into(), "42".into())],
            cwd: "/".into(),
            pty: None,
            stdin: false,
        }
    }

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, STDOUT, b"hello").unwrap();
        write_frame(&mut buf, EXIT, &7i32.to_le_bytes()).unwrap();
        let mut r = &buf[..];
        assert_eq!(
            read_frame(&mut r).unwrap(),
            Some((STDOUT, b"hello".to_vec()))
        );
        assert_eq!(
            read_frame(&mut r).unwrap(),
            Some((EXIT, 7i32.to_le_bytes().to_vec()))
        );
        assert_eq!(read_frame(&mut r).unwrap(), None);
    }

    #[test]
    fn exit_codes_and_environment_come_through() {
        let (_dir, socket) = agent();
        let req = request("sh", &["-c", "test \"$HYPRCROSS_TEST\" = 42 && exit 3"]);
        assert_eq!(call(connect(&socket).unwrap(), &req).unwrap(), 3);
        let killed = request("sh", &["-c", "kill -9 $$"]);
        assert_eq!(call(connect(&socket).unwrap(), &killed).unwrap(), 128 + 9);
    }

    #[test]
    fn a_missing_program_is_an_error_not_an_exit_code() {
        let (_dir, socket) = agent();
        let err = call(connect(&socket).unwrap(), &request("/no/such/program", &[])).unwrap_err();
        assert!(err.to_string().contains("cannot run"), "{err}");
    }

    #[test]
    fn commands_run_on_a_terminal_when_asked() {
        let (_dir, socket) = agent();
        let mut req = request("sh", &["-c", "test -t 1 && test -t 2"]);
        assert_ne!(
            call(connect(&socket).unwrap(), &req).unwrap(),
            0,
            "pipes by default"
        );
        req.pty = Some((24, 80));
        assert_eq!(call(connect(&socket).unwrap(), &req).unwrap(), 0);
    }

    #[test]
    fn an_agent_that_hangs_up_without_answering_is_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("agent.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        // An agent stopping for being idle: it takes the connection and closes it.
        std::thread::spawn(move || drop(listener.accept()));
        let err = call(connect(&socket).unwrap(), &request("true", &[])).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotConnected, "{err}");
    }

    #[test]
    fn a_wrong_protocol_is_refused() {
        let (_dir, socket) = agent();
        let mut req = request("true", &[]);
        req.protocol = PROTOCOL + 1;
        assert!(call(connect(&socket).unwrap(), &req).is_err());
    }
}
