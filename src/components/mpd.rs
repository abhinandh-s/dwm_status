pub fn mpd() -> String {
    match mpd::Client::connect("127.0.0.1:6600") {
        Ok(mut conn) => conn.currentsong().map_or("".to_owned(), |f| {
            f.map_or("".to_owned(), |f| f.title.unwrap_or_default())
        }),
        Err(_) => "".to_owned(),
    }
}
