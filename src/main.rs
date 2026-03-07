use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate chan;
extern crate chan_signal;

extern crate chrono;
extern crate notify_rust;
extern crate systemstat;

use chan_signal::Signal;
use systemstat::{Platform, System};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

struct StatusBar {
    conn: RustConnection,
    root: u32,
}

impl StatusBar {
    fn new() -> Self {
        let (conn, screen_num) = x11rb::connect(None).expect("failed to connect to X11");
        let root = conn.setup().roots[screen_num].root;
        Self { conn, root }
    }

    fn update(&self, s: &str) {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.root,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                s.as_bytes(),
            )
            .ok();
        self.conn.flush().ok();
    }
}

pub const SPARKLINE: &str = "⠁⠂⠄⡀";
pub const NF_PLE_LOWER_RIGHT_TRIANGLE: &str = "";
pub const NF_PLE_LOWER_LEFT_TRIANGLE: &str = "";

trait Plugin {
    fn render(&self) -> String;
    // Required to return 'Self' by value
    fn edit<F: FnOnce(&mut Self) + Sized>(self, f: F) -> Self;
}

struct User {
    name: String,
}

impl User {
    fn new(name: &str) -> Self {
        Self { name: name.into() }
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }
}

impl Plugin for User {
    fn render(&self) -> String {
        format!("  {}'s Arch Linux", self.name)
    }

    fn edit<F: FnOnce(&mut Self) + Sized>(mut self, f: F) -> Self {
        f(&mut self);
        self
    }

    // fn edit<F>(&mut self, f: F)
    // where
    //     F: FnOnce(&mut Self),
    // {
    //     self.name = f(self)
    // }
}

const KB: u64 = 1024;
const MB: u64 = KB * 1024;
const GB: u64 = MB * 1024;

fn format_bytes(bytes: u64) -> String {
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else {
        format!("{:.1} KB", bytes / KB)
    }
}

static LAST_RX: AtomicU64 = AtomicU64::new(0);
static LAST_TX: AtomicU64 = AtomicU64::new(0);

fn plugged(sys: &System) -> String {
    let (current_rx, current_tx) = sys
        .network_stats("wlan0")
        .map_or((0, 0), |s| (s.rx_bytes.as_u64(), s.tx_bytes.as_u64()));

    // Calculate delta (current - previous)
    // We use swap to update the static and get the old value in one go
    let old_rx = LAST_RX.swap(current_rx, Ordering::Relaxed);
    let old_tx = LAST_TX.swap(current_tx, Ordering::Relaxed);

    // If it's the first run or stats reset, speed is 0
    let rx_speed = current_rx.saturating_sub(old_rx);
    let tx_speed = current_tx.saturating_sub(old_tx);

    // Note: Since your loop runs every 500ms, multiply by 2 to get bytes per second
    // or just leave it as 'per update' for simplicity.
    // Let's assume per second:
    format!(
        " [   {}/s ][  {}/s",
        format_bytes(rx_speed * 2),
        format_bytes(tx_speed * 2)
    )
}

pub struct Ram {
    pub total: u64,
    pub usage: u64,
    pub free: u64,
}

impl Ram {
    fn new(sys: &System) -> Self {
        let (total, usage, free) = sys.memory().map_or((0, 0, 0), |m| {
            let t = m.total.0;
            let f = m.free.0;
            let u = t - f;
            (t, u, f)
        });

        Self { total, usage, free }
    }

    fn usage_as_bytes(&self) -> u64 {
        self.usage
    }

    fn usage_as_kilobytes(&self) -> u64 {
        self.usage_as_bytes().saturating_div(1024)
    }

    fn usage_as_megabytes(&self) -> u64 {
        self.usage_as_kilobytes().saturating_div(1024)
    }

    fn usage_as_gigabytes(&self) -> u64 {
        self.usage_as_megabytes().saturating_div(1024)
    }
}

pub trait Cs {
    fn as_kilobytes(&self) -> u64;
    fn as_megabytes(&self) -> u64;
    fn as_gigabytes(&self) -> u64;
}

impl Cs for u64 {
    fn as_kilobytes(&self) -> u64 {
        self.saturating_div(1024)
    }

    fn as_megabytes(&self) -> u64 {
        self.as_kilobytes().saturating_div(1024)
    }

    fn as_gigabytes(&self) -> u64 {
        self.as_megabytes().saturating_div(1024)
    }
}

pub const RAM_ICON: &str = "   ";

fn ram(sys: &System) -> String {
    let r = Ram::new(sys).usage_as_gigabytes();
    format!("{} {} GB", RAM_ICON, r)
}

fn cpu(sys: &System) -> String {
    if let Ok(load) = sys.load_average() {
        format!("⚙ CPU: {:.2}", load.one)
    } else {
        "⚙ _".to_string()
    }
}

fn date() -> String {
    chrono::Local::now()
        .format("   %a, %d %h  ~  󰥔   %R ]    ")
        .to_string()
}

fn separated(s: String) -> String {
    if s.is_empty() { s } else { s + "   ][   " }
}

use slstatus::{Icons, rand_num};

fn music() -> String {
    let r = slstatus::mpd();
    format!("{}     {}", Icons::MUSIC, r)
}
fn start() -> String {
    "[   ".to_owned()
}
fn status(sys: &System) -> String {
    let user = User::new("Charlie")
        .edit(|user| {
            user.set_name("Abhi");
        })
        .render();

    start() + &separated(music())
    + &separated(plugged(sys))
        + &separated(ram(sys))
        + &separated(cpu(sys))
        + &separated(rand_num())
        + &separated(user)
        + &date()
}

use x11rb::wrapper::ConnectionExt;



fn run(_sdone: chan::Sender<()>, bar: &StatusBar) {
    let sys = System::new();
    let (sender, receiver) = std::sync::mpsc::sync_channel::<(String, String, i32)>(8);

    // FIFO listener thread (from previous answer)
    std::thread::spawn(move || notify_pipe_listener(sender));

    let mut banner = String::new();
    loop {
        if let Ok((summary, body, timeout)) = receiver.try_recv() {
            banner = format!("{} {}", summary, body);
            bar.update(&banner);
            const MAX_TIMEOUT: i32 = 60_000;
            let t = if timeout <= 0 || timeout > MAX_TIMEOUT {
                MAX_TIMEOUT
            } else {
                timeout
            };
            thread::sleep(Duration::from_millis(t as u64));
        }
        let next_banner = status(&sys);
        if next_banner != banner {
            banner = next_banner;
            bar.update(&banner);
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn main() {
    let bar = std::sync::Arc::new(StatusBar::new());
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    let (sdone, rdone) = chan::sync(0);

    let bar_run = bar.clone();
    std::thread::spawn(move || run(sdone, &bar_run));

    chan_select! {
        signal.recv() -> signal => {
            bar.update(&format!("rust-dwm-status stopped with signal {:?}.", signal));
        },
        rdone.recv() => {
            bar.update("rust-dwm-status: done.");
        }
    }
}

// Remove ALL zbus imports and the NotifServer struct entirely.
// Replace the channel + thread + zbus block in run() with this:

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};

fn notify_pipe_listener(sender: std::sync::mpsc::SyncSender<(String, String, i32)>) {
    let path = "/tmp/dwm-notify";

    // Create the FIFO if it doesn't exist
    if !std::path::Path::new(path).exists() {
        unsafe {
            let cpath = std::ffi::CString::new(path).unwrap();
            libc::mkfifo(cpath.as_ptr(), 0o622);
        }
    }

    loop {
        // open() blocks until a writer connects — that's intentional
        if let Ok(file) = OpenOptions::new().read(true).open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                // Format: "SUMMARY\tBODY\tTIMEOUT_MS"  or just "SUMMARY"
                let mut parts = line.splitn(3, '\t');
                let summary = parts.next().unwrap_or("").to_string();
                let body = parts.next().unwrap_or("").to_string();
                let timeout = parts.next().and_then(|t| t.parse().ok()).unwrap_or(5000);
                sender.send((summary, body, timeout)).ok();
            }
        }
    }
}
