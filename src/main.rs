use std::sync::atomic::AtomicUsize;
use std::thread;
use std::time::Duration;

use chan::chan_select;
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

pub trait Plugin {
    fn render(&self) -> String;
    // Required to return 'Self' by value
    fn edit<F: FnOnce(&mut Self) + Sized>(self, f: F) -> Self;
}

pub struct User {
    name: String,
}

impl User {
    pub fn new(name: &str) -> Self {
        Self { name: name.into() }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }
}

impl Plugin for User {
    fn render(&self) -> String {
        fmt_with_sep!(" {}'s Arch Linux", self.name)
    }

    fn edit<F: FnOnce(&mut Self) + Sized>(mut self, f: F) -> Self {
        f(&mut self);
        self
    }
}
use slstatus::{Cpu, Icons, date, fmt_with_sep, network_speed, ram};

pub struct Spinner {
    frames: &'static [&'static str],
    index: usize,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            index: 0,
        }
    }

    pub fn tick(&mut self) -> &'static str {
        let frame = self.frames[self.index];
        self.index = (self.index + 1) % self.frames.len();
        frame
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}


// Animation frames constant
const SPINNER_FRAMES: &[&str] = &[" ", "▃", "▄", "▅", "▆", "▇", "▆", "▅", "▄", "▃"];
//const SPINNER_FRAMES: &[&str] = &["    ", "=   ", "==  ", "=== ", " ===", "  ==", "   =", "    "];
// const SPINNER_FRAMES: &[&str] = &["┤", "┘", "┴", "└", "├", "┌", "┬", "┐"]; // Snake
// ["[    ]", "[=   ]", "[==  ]", "[=== ]", "[ ===]", "[  ==]", "[   =]", "[    ]"]
// Bouncing ["[ ]", "[= ]", "[== ]", "[=== ]", "[ ===]", "[ ==]", "[ =]", "[ ]"]
// [" ", "▃", "▄", "▅", "▆", "▇", "▆", "▅", "▄", "▃"]

pub fn get_spinner() -> &'static str {
    // Get milliseconds since epoch
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    
    // Divide by 80 to change frames every 80ms
    let index = (ms / 80) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[index]
}

// pub const SPINNER_FRAMES: &[&str] = &["○", "◎", "●", "◎"];

static STATE: AtomicUsize = AtomicUsize::new(0);


fn music() -> String {
    slstatus::mpd().map_or(String::new(), |music| {
        fmt_with_sep!("{}  {}", Icons::MUSIC, music)
    })
}

pub struct Ctx(String);

impl Default for Ctx {
    fn default() -> Self {
        Self(String::from(" "))
    }
}

impl Ctx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<P: Into<String>>(&mut self, plugin: P) -> &mut Self {
        {
            self.0.push_str(plugin.into().as_str());
        }
        self
    }

    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

fn status(sys: &System) -> String {
    let cpu = Cpu::new(sys);
    let mut ctx = Ctx::new();

    ctx.add(music());
    ctx.add(network_speed(sys));
    ctx.add(ram(sys));
    ctx.add(cpu.load());
    ctx.add(cpu.heat());
    ctx.add(date());

    ctx.finish()
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
        thread::sleep(Duration::from_millis(500)); // prev: 80
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
