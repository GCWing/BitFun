use bitfun_product_domains::function_agents::{
    git_func_agent::{CommitFormat, CommitMessageOptions},
    startchat_func_agent::WorkStateOptions,
    Language,
};

#[test]
fn git_commit_options_preserve_existing_defaults() {
    let options = CommitMessageOptions::default();

    assert_eq!(options.format, CommitFormat::Conventional);
    assert!(options.include_files);
    assert!(options.include_body);
    assert_eq!(options.max_title_length, 72);
    assert_eq!(options.language, Language::Chinese);
}

#[test]
fn startchat_options_preserve_existing_defaults() {
    let options = WorkStateOptions::default();

    assert!(options.analyze_git);
    assert!(options.predict_next_actions);
    assert!(options.include_quick_actions);
    assert_eq!(options.language, Language::English);
}
