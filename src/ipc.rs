//! Engine <-> dashboard protocol over a local named pipe (JSON lines).
//! The pipe rejects remote clients; with the default pipe DACL only the same
//! (elevated) user can write to it, so only the owner can control the engine.

use crate::sys::{wide, OwnedHandle};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;

pub const PIPE_NAME: &str = r"\\.\pipe\avix.engine.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    /// Boost automatically when a profile's app is running.
    #[default]
    Auto,
    /// Boost right now regardless of what's running.
    On,
    /// Never boost (paused).
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    Info,
    Boost,
    Warn,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub time: String,
    pub kind: EventKind,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ActiveInfo {
    pub profile: String,
    pub target: String,
    pub target_pid: u32,
    pub reason: String,
    pub since_secs: u64,
    pub power_plan: String,
    pub throttled: u32,
    pub raised: u32,
    pub services_paused: Vec<String>,
    pub apps_closed: u32,
    pub standby_purges: u32,
    pub timer_resolution: bool,
    pub protected_target: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Status {
    pub version: String,
    pub elevated: bool,
    pub mode: Mode,
    pub forced_profile: Option<String>,
    pub active: Option<ActiveInfo>,
    pub foreground: String,
    pub cpu_percent: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub standby: Option<u64>,
    pub free: Option<u64>,
    pub engine_ram: u64,
    pub engine_cpu_percent: f32,
    pub uptime_secs: u64,
    pub detected_games: Vec<String>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    SetMode { mode: Mode, profile: Option<String> },
    ReloadConfig,
    PurgeStandby,
    OpenUi,
    Shutdown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Status { status: Box<Status> },
    Ok,
    Error { message: String },
}

// ---------------------------------------------------------------- server

pub fn start_server<F>(handler: F)
where
    F: Fn(Request) -> Response + Send + Sync + Clone + 'static,
{
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_PIPE_CONNECTED};
    use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    std::thread::Builder::new()
        .name("avix-ipc".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let name = wide(PIPE_NAME);
            loop {
                let h = unsafe {
                    CreateNamedPipeW(
                        name.as_ptr(),
                        PIPE_ACCESS_DUPLEX,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                        PIPE_UNLIMITED_INSTANCES,
                        64 * 1024,
                        64 * 1024,
                        0,
                        std::ptr::null(),
                    )
                };
                let Some(owned) = OwnedHandle::new(h) else {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                };
                let connected = unsafe {
                    ConnectNamedPipe(owned.0, std::ptr::null_mut()) != 0 || GetLastError() == ERROR_PIPE_CONNECTED
                };
                if !connected {
                    continue;
                }
                let raw = owned.0;
                std::mem::forget(owned); // ownership moves to File
                let file = unsafe { File::from_raw_handle(raw as _) };
                let handler = handler.clone();
                let _ = std::thread::Builder::new()
                    .name("avix-ipc-client".into())
                    .stack_size(256 * 1024)
                    .spawn(move || serve(file, handler));
            }
        })
        .expect("spawn ipc thread");
}

fn serve<F: Fn(Request) -> Response>(file: File, handler: F) {
    let Ok(mut writer) = file.try_clone() else { return };
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handler(req),
            Err(e) => Response::Error { message: format!("bad request: {e}") },
        };
        let Ok(text) = serde_json::to_string(&resp) else { break };
        if writeln!(writer, "{text}").is_err() || writer.flush().is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------- client

pub struct Client {
    reader: BufReader<File>,
    writer: File,
}

impl Client {
    pub fn connect() -> Option<Self> {
        let f = OpenOptions::new().read(true).write(true).open(PIPE_NAME).ok()?;
        let writer = f.try_clone().ok()?;
        Some(Self { reader: BufReader::new(f), writer })
    }

    pub fn request(&mut self, req: &Request) -> Option<Response> {
        let text = serde_json::to_string(req).ok()?;
        writeln!(self.writer, "{text}").ok()?;
        self.writer.flush().ok()?;
        let mut line = String::new();
        if self.reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        serde_json::from_str(&line).ok()
    }

    pub fn send_once(req: &Request) -> bool {
        Self::connect().and_then(|mut c| c.request(req)).is_some()
    }
}
