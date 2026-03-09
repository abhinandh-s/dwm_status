pub fn date() -> String {
    chrono::Local::now()
        .format("[  %a, %d %h ~ 󰥔 %R ] ")
        .to_string()
}


