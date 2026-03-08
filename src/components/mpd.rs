use mpd::State;

pub fn mpd() -> Option<String> {
    let mut conn = mpd::Client::connect("127.0.0.1:6600").ok()?;

    let is_playing = conn.status().ok()?.state;
    
    if is_playing != State::Play {
        return None;
    }

    conn.currentsong().ok()??.title
}
