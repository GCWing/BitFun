mod anthropic;
mod gemini;
mod inline_think;
mod openai;
mod responses;
mod stream_stats;

use futures::{Stream, StreamExt};
use std::time::Duration;

pub use anthropic::handle_anthropic_stream;
pub use gemini::handle_gemini_stream;
pub use openai::handle_openai_stream;
pub use responses::handle_responses_stream;

pub(super) enum TimedStreamItem<T> {
    Item(T),
    End,
    TimedOut,
}

pub(super) async fn next_stream_item<S>(
    stream: &mut S,
    idle_timeout: Option<Duration>,
) -> TimedStreamItem<S::Item>
where
    S: Stream + Unpin,
{
    match idle_timeout {
        Some(idle_timeout) => match tokio::time::timeout(idle_timeout, stream.next()).await {
            Ok(Some(item)) => TimedStreamItem::Item(item),
            Ok(None) => TimedStreamItem::End,
            Err(_) => TimedStreamItem::TimedOut,
        },
        None => match stream.next().await {
            Some(item) => TimedStreamItem::Item(item),
            None => TimedStreamItem::End,
        },
    }
}
