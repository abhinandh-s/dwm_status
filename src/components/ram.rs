use systemstat::{System, Platform};

use crate::{Icons, fmt_with_sep};

pub struct Ram {
    pub total: u64,
    pub usage: u64,
    pub free: u64,
}

impl Ram {
    pub fn new(sys: &System) -> Self {
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

pub fn ram(sys: &System) -> String {
    let r = Ram::new(sys).usage_as_gigabytes();
    fmt_with_sep!("{}  {} GB", Icons::RAM, r,)
}
