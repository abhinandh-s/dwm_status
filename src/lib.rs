mod components;

use std::fmt::Display;

pub use components::*;

pub struct Icons;

//
impl Icons {
    pub const MUSIC: &str = "";
    pub const RAM: &str = "";
    pub const FIRE: &str = "";
    pub const CARRET_UP: &str = "";
    pub const CARRET_DOWN: &str = "";
    pub const TRIANGLE_UP: &str = "";
    pub const TRIANGLE_DOWN: &str = "";
    pub const ARROW_UP_THICK: &str = "󰁞";
    pub const ARROW_DOWN_THICK: &str = "󰁆";
}

pub enum Seperator {
    Open,
    Mid,
    Close,
}

impl Display for Seperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Seperator::Open => write!(f, "[ "),
            Seperator::Mid => write!(f, " ][ "),
            Seperator::Close => write!(f, " ]"),
        }
    }
}

#[macro_export]
macro_rules! fmt_with_sep {
    ($($arg:tt)*) => {{
        let mut s = String::new();
        s.push_str($crate::Seperator::Open.to_string().as_str());
        let f = format!($($arg)*);
        s.push_str(&f);
        s.push_str($crate::Seperator::Close.to_string().as_str());
        s
    }}
}
