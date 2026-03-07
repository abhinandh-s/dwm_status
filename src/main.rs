#![allow(unused)]

use std::fmt::Display;
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

fn plugged(sys: &System) -> String {
    if let Ok(plugged) = sys.on_ac_power() {
        if plugged {
            "🔌 ✓".to_string()
        } else {
            "🔌 ✘".to_string()
        }
    } else {
        "🔌".to_string()
    }
}

fn battery(sys: &System) -> String {
    if let Ok(bat) = sys.battery_life() {
        format!("🔋 {:.1}%", bat.remaining_capacity * 100.)
    } else {
        "".to_string()
    }
}

struct Ram {
    total: u64,
    usage: u64,
    free: u64,
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

    fn set_usage(&mut self, usage: u64) {
        self.usage = usage;
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

pub const RAM_ICON: &str = "   ";

impl Display for Ram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let usage = self.usage_as_gigabytes();
        write!(f, "{} {} GB", RAM_ICON, usage)
    }
}

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
        .format("   %a, %d %h  |  󰥔   %R    ")
        .to_string()
}

fn separated(s: String) -> String {
    if s.is_empty() { s } else { s + "  |  " }
}

fn status(sys: &System) -> String {
    let user = User::new("Charlie")
        .edit(|user| {
            user.set_name("Abhi");
        })
        .render();

    separated(plugged(sys))
        + &separated(Ram::new(sys).usage_as_gigabytes().to_string())
        + &separated(cpu(sys))
        + &separated(rand_num())
        + &separated(user)
        + &date()
}

use x11rb::wrapper::ConnectionExt;

fn update_status(s: &str) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    if let Ok((conn, screen_num)) = x11rb::connect(None) {
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        conn.change_property8(
            PropMode::REPLACE,
            root,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            s.as_bytes(),
        )
        .ok();
        conn.flush().ok();
    }
}

use std::collections::HashMap;
use std::sync::Mutex;
use zbus::zvariant::OwnedValue;
use zbus::{blocking::connection, interface};

struct NotifServer {
    sender: std::sync::mpsc::SyncSender<(String, String, i32)>,
    id: Mutex<u32>,
}

#[allow(clippy::too_many_arguments)]
#[interface(name = "org.freedesktop.Notifications")]
impl NotifServer {
    fn notify(
        &self,
        _app_name: String,
        _replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        self.sender.send((summary, body, expire_timeout)).ok();
        let mut id = self.id.lock().unwrap();
        *id += 1;
        *id
    }

    fn close_notification(&self, _id: u32) {}

    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into()]
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("slstatus", "slstatus", "0.1.0", "1.2")
    }
}

fn run(_sdone: chan::Sender<()>) {
    let sys = System::new();
    let (sender, receiver) = std::sync::mpsc::sync_channel::<(String, String, i32)>(8);

    std::thread::spawn(move || {
        let server = NotifServer {
            sender,
            id: Mutex::new(0),
        };

        let _conn = connection::Builder::session()
            .expect("session bus failed")
            .name("org.freedesktop.Notifications")
            .expect("name already taken — stop dunst/mako first")
            .serve_at("/org/freedesktop/Notifications", server)
            .expect("serve_at failed")
            .build()
            .expect("connection build failed");

        // Keep thread alive — _conn must not drop
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });

    let mut banner = String::new();
    loop {
        if let Ok((summary, body, timeout)) = receiver.try_recv() {
            banner = format!("{} {}", summary, body);
            update_status(&banner);
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
            update_status(&banner);
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn main() {
    // Signal gets a value when the OS sent a INT or TERM signal.
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    // When our work is complete, send a sentinel value on `sdone`.
    let (sdone, rdone) = chan::sync(0);
    // Run work.
    std::thread::spawn(move || run(sdone));

    // Wait for a signal or for work to be done.
    chan_select! {
        signal.recv() -> signal => {
            update_status(&format!("rust-dwm-status stopped with signal {:?}.", signal));
        },
        rdone.recv() => {
            update_status("rust-dwm-status: done.");
        }
    }
}

use std::sync::atomic::{AtomicU64, Ordering};

static STATE: AtomicU64 = AtomicU64::new(0x517cc1b727220a95);

fn rand_num() -> String {
    let mut x = STATE.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    format!("  {}", x % 1000 + 1)
}
