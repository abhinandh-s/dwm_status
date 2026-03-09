use systemstat::{System, Platform};

use crate::{Icons, fmt_with_sep};

pub struct Cpu<'a> {
    sys: &'a System,
}

impl<'a> Cpu<'a> {
    pub fn new(sys: &'a System) -> Self {
        Self { sys }
    }

    pub fn load(&self) -> String {
        if let Ok(load) = self.sys.load_average() {
            fmt_with_sep!("  {:.2}", load.one)
        } else {
            "⚙ _".to_string()
        }
    }

    pub fn heat(&self) -> String {
        if let Ok(load) = self.sys.cpu_temp() {
            fmt_with_sep!("{} {:.2}", Icons::FIRE, load)
        } else {
            "".to_string()
        }
    }
}


