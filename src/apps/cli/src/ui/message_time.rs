pub(super) fn format_message_timestamp(timestamp: std::time::SystemTime) -> Option<String> {
    if timestamp == std::time::SystemTime::UNIX_EPOCH {
        return None;
    }
    let local = chrono::DateTime::<chrono::Local>::from(timestamp);
    let format = if local.date_naive() == chrono::Local::now().date_naive() {
        "%H:%M"
    } else {
        "%Y-%m-%d %H:%M"
    };
    Some(local.format(format).to_string())
}
