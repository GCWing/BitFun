mod common;

use common::sse_fixture_server::FixtureSseServerOptions;
use common::stream_test_harness::{
    run_stream_fixture_with_options, StreamFixtureProvider, StreamFixtureRunOptions,
};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[should_panic(expected = "stream fixture processing timed out")]
async fn stream_test_harness_fails_fast_when_fixture_processing_stalls() {
    run_stream_fixture_with_options(
        StreamFixtureProvider::OpenAi,
        "stream/openai/tool_args_split_with_usage.sse",
        StreamFixtureRunOptions {
            server_options: FixtureSseServerOptions {
                chunk_size: 1,
                chunk_delay: Duration::from_millis(50),
                ..Default::default()
            },
            process_timeout: Duration::from_millis(20),
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ttft_timeout_waits_for_first_effective_stream_output_not_http_200() {
    let output = run_stream_fixture_with_options(
        StreamFixtureProvider::OpenAi,
        "stream/openai/tool_args_split_with_usage.sse",
        StreamFixtureRunOptions {
            server_options: FixtureSseServerOptions {
                initial_delay: Duration::from_millis(60),
                ..Default::default()
            },
            ttft_timeout: Some(Duration::from_millis(20)),
            process_timeout: Duration::from_secs(1),
            ..Default::default()
        },
    )
    .await;

    let error = output
        .result
        .expect_err("fixture should fail with TTFT timeout");
    assert!(
        error
            .error
            .to_string()
            .contains("TTFT timeout after 0s waiting for first effective output"),
        "unexpected error: {}",
        error.error
    );
}
