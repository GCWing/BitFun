use bitfun_agent_runtime::prompt_cache::{
    prompt_cache_scope_key, CachedSystemPrompt, CachedUserContext, PromptCacheLookup,
    PromptCachePolicy, PromptCacheScope, SessionPromptCacheStore, SystemPromptCacheIdentity,
    UserContextCacheIdentity, DEFAULT_PROMPT_CACHE_PERSISTENCE_TTL,
};
use std::time::Duration;

#[test]
fn prompt_cache_policy_keeps_existing_default_persistence_ttl() {
    let policy = PromptCachePolicy::default();

    assert_eq!(policy.cache_ttl, None);
    assert_eq!(
        policy.persistence_ttl,
        Some(DEFAULT_PROMPT_CACHE_PERSISTENCE_TTL)
    );
}

#[test]
fn prompt_cache_lookup_preserves_identity_and_expiry_semantics() {
    let store = SessionPromptCacheStore::new();
    store.create_session("session-1");
    store.set_system_prompt(
        "session-1",
        CachedSystemPrompt::new(
            SystemPromptCacheIdentity::new("template:agentic_mode"),
            "system prompt",
        ),
    );
    store.set_user_context(
        "session-1",
        CachedUserContext::new(
            UserContextCacheIdentity::new("workspace_context"),
            "user context",
        ),
    );

    assert!(matches!(
        store.lookup_system_prompt(
            "session-1",
            &SystemPromptCacheIdentity::new("template:debug_mode"),
            None
        ),
        PromptCacheLookup::Miss
    ));
    assert!(matches!(
        store.lookup_user_context(
            "session-1",
            &UserContextCacheIdentity::new("workspace_context"),
            Some(Duration::from_millis(0))
        ),
        PromptCacheLookup::Expired
    ));
    assert!(store
        .get_cache("session-1")
        .expect("session cache")
        .user_context
        .is_none());

    assert!(store.invalidate("session-1", PromptCacheScope::All));
    let cache = store.get_cache("session-1").expect("session cache");
    assert!(cache.system_prompt.is_none());
    assert!(cache.user_context.is_none());
}

#[test]
fn prompt_cache_scope_key_preserves_legacy_mode_switch_shape() {
    assert_eq!(
        prompt_cache_scope_key(
            &SystemPromptCacheIdentity::new("template:agentic_mode"),
            &UserContextCacheIdentity::new("workspace_context|workspace_instructions"),
        ),
        "template:agentic_mode||workspace_context|workspace_instructions"
    );
}
