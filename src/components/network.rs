use std::sync::atomic::{AtomicU64, Ordering};

use systemstat::{Platform, System};

use crate::{Icons, Seperator, fmt_with_sep, format_bytes};

static LAST_RX: AtomicU64 = AtomicU64::new(0);
static LAST_TX: AtomicU64 = AtomicU64::new(0);

pub fn network_speed(sys: &System) -> String {
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
    fmt_with_sep!(
        "{} {}/s{}{} {}/s",
        Icons::ARROW_DOWN_THICK,
        format_bytes(rx_speed * 2),
        Seperator::Mid,
        Icons::ARROW_UP_THICK,
        format_bytes(tx_speed * 2),
    )
}
