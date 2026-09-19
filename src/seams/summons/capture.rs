//! Bounded capture owns its pipes in the waiting thread, so an escaped pipe
//! holder cannot strand a reader or its descriptors (§FS-016-browser-opening.2).

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use super::{spawn, EphorError, ExecutionError};

pub(super) fn captured(
    mut command: Command,
    verb: &str,
    timeout: Duration,
) -> Result<(ExitStatus, String, String), ExecutionError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Cleanup owns this group, not descendants that create other sessions
        // (§FS-016-browser-opening.2, §AR-002-summons.2).
        command.process_group(0);
    }
    let deadline = Instant::now() + timeout;
    let mut child = spawn(&mut command).map_err(|error| {
        ExecutionError::BeforeSpawn(EphorError::Command(format!(
            "{verb}: failed to run: {error}"
        )))
    })?;
    let result = collect(&mut child, verb, timeout, deadline);
    if result.is_err() {
        #[cfg(unix)]
        // SAFETY: process_group(0) gave this child a group identified by its
        // PID. This targets that group alone; the direct child is also reaped.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn collect(
    child: &mut Child,
    verb: &str,
    timeout: Duration,
    deadline: Instant,
) -> Result<(ExitStatus, String, String), ExecutionError> {
    let failed = |error| {
        ExecutionError::AfterSpawn(EphorError::Command(format!(
            "{verb}: capture failed: {error}"
        )))
    };
    let mut stdout = Capture::new(child.stdout.take().expect("piped stdout")).map_err(failed)?;
    let mut stderr = Capture::new(child.stderr.take().expect("piped stderr")).map_err(failed)?;
    let mut status = None;
    loop {
        // One deadline covers exit, both EOFs, and busy writers. Each turn reads
        // at most one chunk per pipe so neither stream can starve the clock
        // (§FS-016-browser-opening.2, §AR-002-summons.2).
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ExecutionError::TimedOut(EphorError::Command(format!(
                "{verb}: timed out after {}s",
                timeout.as_secs()
            ))));
        }
        if status.is_none() {
            status = child.try_wait().map_err(|error| {
                ExecutionError::AfterSpawn(EphorError::Command(format!(
                    "{verb}: failed waiting: {error}"
                )))
            })?;
        }
        let out_progress = stdout.drain().map_err(failed)?;
        let err_progress = stderr.drain().map_err(failed)?;
        if let Some(status) = status {
            if stdout.eof && stderr.eof {
                return Ok((
                    status,
                    stdout.text().map_err(failed)?,
                    stderr.text().map_err(failed)?,
                ));
            }
        }
        if !out_progress && !err_progress {
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(10)),
            );
        }
    }
}

struct Capture<P> {
    pipe: P,
    bytes: Vec<u8>,
    eof: bool,
}

impl<P: Pipe> Capture<P> {
    fn new(pipe: P) -> io::Result<Self> {
        pipe.prepare()?;
        Ok(Self {
            pipe,
            bytes: Vec::new(),
            eof: false,
        })
    }

    fn drain(&mut self) -> io::Result<bool> {
        if self.eof {
            return Ok(false);
        }
        let mut chunk = [0; 16 * 1024];
        match self.pipe.read_ready(&mut chunk) {
            Ok(0) => {
                self.eof = true;
                Ok(false)
            }
            Ok(count) => {
                self.bytes.extend_from_slice(&chunk[..count]);
                Ok(true)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    fn text(self) -> io::Result<String> {
        String::from_utf8(self.bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

trait Pipe: Read {
    fn prepare(&self) -> io::Result<()>;
    fn read_ready(&mut self, bytes: &mut [u8]) -> io::Result<usize>;
}

#[cfg(unix)]
impl<P: Read + std::os::fd::AsRawFd> Pipe for P {
    fn prepare(&self) -> io::Result<()> {
        // SAFETY: the owned pipe stays alive across both fcntl calls. Only the
        // read end is made nonblocking; the child's writes keep their semantics.
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFL);
            if flags == -1
                || libc::fcntl(self.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) == -1
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn read_ready(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.read(bytes)
    }
}

#[cfg(windows)]
impl<P: Read + std::os::windows::io::AsRawHandle> Pipe for P {
    fn prepare(&self) -> io::Result<()> {
        Ok(())
    }

    fn read_ready(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        use std::ffi::c_void;
        use std::ptr::null_mut;
        #[link(name = "kernel32")]
        extern "system" {
            fn PeekNamedPipe(
                pipe: *mut c_void,
                buffer: *mut c_void,
                size: u32,
                read: *mut u32,
                available: *mut u32,
                left: *mut u32,
            ) -> i32;
        }
        let mut available = 0;
        // SAFETY: this is an owned anonymous pipe handle and available is a
        // live out parameter. There is one reader, so bytes observed by Peek
        // cannot be consumed elsewhere before the bounded read.
        if unsafe {
            PeekNamedPipe(
                self.as_raw_handle(),
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            )
        } == 0
        {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(109) {
                Ok(0)
            } else {
                Err(error)
            };
        }
        if available == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let count = bytes.len().min(available as usize);
        self.read(&mut bytes[..count])
    }
}
