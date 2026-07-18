use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use std::time::{Duration, Instant};

const FOLLOW_UP_WAIT: Duration = Duration::from_millis(5);
const CONTINUATION_IDLE_WAIT: Duration = Duration::from_millis(50);
const MAX_BATCH_DURATION: Duration = Duration::from_millis(50);
// Bound one read cycle by both count and time so sustained input cannot starve
// redraws. EventReader keeps only enough state to preserve an Enter that lands
// in the short tail of a rapid Windows paste.
const MAX_BATCH_EVENTS: usize = 256;
// Require a substantial cut batch before treating newline-free keys as paste;
// short key sequences must retain navigation and form semantics.
const LARGE_TEXT_BATCH_MIN_EVENTS: usize = 32;

#[derive(Default)]
pub(crate) struct EventReader {
    continuing_text_burst: bool,
}

impl EventReader {
    /// Read and normalize one bounded burst of terminal input.
    pub(crate) fn read_event_batch(&mut self, timeout: Duration) -> io::Result<Option<Vec<Event>>> {
        let poll_started = Instant::now();
        if self.continuing_text_burst {
            if !event::poll(CONTINUATION_IDLE_WAIT)? {
                self.continuing_text_burst = false;
                let remaining = timeout.saturating_sub(poll_started.elapsed());
                if !event::poll(remaining)? {
                    return Ok(None);
                }
            }
        } else if !event::poll(timeout)? {
            return Ok(None);
        }

        let batch_started = Instant::now();
        let mut events = Vec::with_capacity(8);
        events.push(event::read()?);
        let mut batch_was_cut = false;
        loop {
            if events.len() >= MAX_BATCH_EVENTS {
                batch_was_cut = true;
                break;
            }

            let remaining = MAX_BATCH_DURATION.saturating_sub(batch_started.elapsed());
            if remaining.is_zero() {
                batch_was_cut = true;
                break;
            }
            if !event::poll(remaining.min(FOLLOW_UP_WAIT))? {
                break;
            }
            events.push(event::read()?);
        }

        Ok(Some(self.normalize_batch(events, batch_was_cut)))
    }

    fn normalize_batch(&mut self, events: Vec<Event>, batch_was_cut: bool) -> Vec<Event> {
        let continues_previous = self.continuing_text_burst;
        self.continuing_text_burst = batch_was_cut && rapid_text_candidate(&events);
        normalize_event_batch(events, continues_previous, batch_was_cut)
    }
}

fn rapid_text_candidate(events: &[Event]) -> bool {
    let forbidden_modifiers = KeyModifiers::CONTROL
        | KeyModifiers::ALT
        | KeyModifiers::SUPER
        | KeyModifiers::HYPER
        | KeyModifiers::META;

    let mut has_active_text = false;
    for event in events {
        match event {
            Event::Key(key)
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
            {
                let text_key = matches!(
                    key.code,
                    KeyCode::Char(character) if !character.is_control()
                ) || matches!(key.code, KeyCode::Enter | KeyCode::Tab);
                if key.modifiers.intersects(forbidden_modifiers) || !text_key {
                    return false;
                }
                has_active_text = true;
            }
            Event::Key(_) | Event::Resize(_, _) => {}
            _ => return false,
        }
    }
    has_active_text
}

fn normalize_event_batch(
    events: Vec<Event>,
    continues_previous: bool,
    batch_was_cut: bool,
) -> Vec<Event> {
    let mut active_key_count = 0;
    let mut has_enter = false;
    let mut has_printable = false;
    let mut rapid_paste_text = String::new();
    let mut rapid_paste_eligible = true;
    let forbidden_modifiers = KeyModifiers::CONTROL
        | KeyModifiers::ALT
        | KeyModifiers::SUPER
        | KeyModifiers::HYPER
        | KeyModifiers::META;

    for event in &events {
        match event {
            Event::Key(key)
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
            {
                active_key_count += 1;
                if key.modifiers.intersects(forbidden_modifiers) {
                    rapid_paste_eligible = false;
                    continue;
                }

                match key.code {
                    KeyCode::Char(character) if !character.is_control() => {
                        has_printable = true;
                        rapid_paste_text.push(character);
                    }
                    KeyCode::Enter => {
                        has_enter = true;
                        rapid_paste_text.push('\n');
                    }
                    KeyCode::Tab => rapid_paste_text.push('\t'),
                    _ => rapid_paste_eligible = false,
                }
            }
            Event::Paste(_) => rapid_paste_eligible = false,
            _ => {}
        }
    }

    let current_batch_looks_like_paste = active_key_count >= 3 && has_printable && has_enter;
    let large_cut_text_batch =
        batch_was_cut && active_key_count >= LARGE_TEXT_BATCH_MIN_EVENTS && has_printable;
    let continues_multiline_paste = continues_previous && active_key_count > 0 && has_enter;
    if rapid_paste_eligible
        && (current_batch_looks_like_paste || large_cut_text_batch || continues_multiline_paste)
    {
        let mut normalized = Vec::with_capacity(events.len() - active_key_count + 1);
        normalized.push(Event::Paste(rapid_paste_text));
        normalized.extend(events.into_iter().filter(|event| {
            !matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
        }));
        return normalized;
    }

    events
}

#[cfg(test)]
mod tests {
    use super::{normalize_event_batch, EventReader, MAX_BATCH_EVENTS};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        key_with(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn key_with(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            state: KeyEventState::empty(),
        })
    }

    #[test]
    fn rapid_printable_keys_with_enter_become_one_paste_event() {
        let normalized = normalize_event_batch(
            vec![
                key(KeyCode::Char('a')),
                key_with(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat),
                key(KeyCode::Char('b')),
                Event::Resize(120, 40),
            ],
            false,
            false,
        );

        assert_eq!(normalized.len(), 2);
        assert!(matches!(&normalized[0], Event::Paste(text) if text == "a\nb"));
        assert_eq!(normalized[1], Event::Resize(120, 40));
    }

    #[test]
    fn two_keys_remain_individual_input_events() {
        let normalized = normalize_event_batch(
            vec![key(KeyCode::Char('a')), key(KeyCode::Enter)],
            false,
            false,
        );

        assert_eq!(normalized.len(), 2);
        assert!(matches!(normalized[0], Event::Key(_)));
        assert!(matches!(normalized[1], Event::Key(_)));
    }

    #[test]
    fn explicit_paste_keeps_content_for_the_focused_input_owner() {
        let normalized =
            normalize_event_batch(vec![Event::Paste("a\r\nb\rc".to_string())], false, false);

        assert_eq!(normalized, vec![Event::Paste("a\r\nb\rc".to_string())]);
    }

    #[test]
    fn command_chords_are_never_collapsed_into_paste() {
        let normalized = normalize_event_batch(
            vec![
                key_with(
                    KeyCode::Char('c'),
                    KeyModifiers::CONTROL,
                    KeyEventKind::Press,
                ),
                key(KeyCode::Enter),
                key(KeyCode::Char('x')),
            ],
            false,
            false,
        );

        assert_eq!(normalized.len(), 3);
        assert!(matches!(normalized[0], Event::Key(_)));
    }

    #[test]
    fn non_text_keys_are_never_dropped_from_a_candidate_batch() {
        let normalized = normalize_event_batch(
            vec![
                key(KeyCode::Char('a')),
                key(KeyCode::Enter),
                key(KeyCode::Left),
            ],
            false,
            false,
        );

        assert_eq!(normalized.len(), 3);
        assert!(matches!(normalized[2], Event::Key(key) if key.code == KeyCode::Left));
    }

    #[test]
    fn a_cut_large_printable_batch_becomes_one_paste_event() {
        let mut reader = EventReader::default();
        let normalized = reader.normalize_batch(
            (0..MAX_BATCH_EVENTS)
                .map(|_| key(KeyCode::Char('a')))
                .collect(),
            true,
        );

        assert_eq!(normalized.len(), 1);
        assert!(matches!(&normalized[0], Event::Paste(text) if text.len() == MAX_BATCH_EVENTS));
    }

    #[test]
    fn an_enter_tail_after_a_saturated_text_batch_stays_paste_input() {
        let mut reader = EventReader::default();
        let first_batch = reader.normalize_batch(
            (0..MAX_BATCH_EVENTS)
                .map(|_| key(KeyCode::Char('a')))
                .collect(),
            true,
        );
        let tail_batch = reader.normalize_batch(vec![key(KeyCode::Enter)], false);

        assert!(first_batch.iter().chain(&tail_batch).all(|event| {
            !matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter)
        }));
    }

    #[test]
    fn a_tab_inside_rapid_multiline_text_is_kept_as_paste_content() {
        let normalized = normalize_event_batch(
            vec![
                key(KeyCode::Char('a')),
                key(KeyCode::Tab),
                key(KeyCode::Char('b')),
                key(KeyCode::Enter),
            ],
            false,
            false,
        );

        assert_eq!(normalized, vec![Event::Paste("a\tb\n".to_string())]);
    }

    #[test]
    fn a_tab_inside_a_short_single_line_batch_keeps_key_routing() {
        let normalized = normalize_event_batch(
            vec![key(KeyCode::Char('a')), key(KeyCode::Tab)],
            false,
            false,
        );

        assert_eq!(normalized.len(), 2);
        assert!(normalized
            .iter()
            .all(|event| matches!(event, Event::Key(_))));
    }

    #[test]
    fn release_events_do_not_trigger_or_disappear_during_normalization() {
        let released = key_with(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        let normalized = normalize_event_batch(
            vec![
                released.clone(),
                key(KeyCode::Enter),
                key(KeyCode::Char('b')),
            ],
            false,
            false,
        );

        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[0], released);
    }

    #[test]
    fn a_release_only_batch_does_not_arm_paste_continuation() {
        let released = key_with(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        let mut reader = EventReader::default();
        reader.normalize_batch(vec![released; MAX_BATCH_EVENTS], true);

        let tail = reader.normalize_batch(vec![key(KeyCode::Enter)], false);

        assert!(matches!(tail[0], Event::Key(key) if key.code == KeyCode::Enter));
    }
}
