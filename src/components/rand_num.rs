use std::sync::atomic::{AtomicU64, Ordering};

use crate::fmt_with_sep;

static STATE: AtomicU64 = AtomicU64::new(0x517cc1b727220a95);

pub fn rand_num() -> String {
    let mut old = STATE.load(Ordering::Relaxed);
    let new = loop {
        let mut x = old;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        match STATE.compare_exchange_weak(old, x, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break x,
            Err(current) => old = current, // retry with updated value
        }
    };
    fmt_with_sep!("{}", new % 1000 + 1)
}
