use std::collections::HashMap;

pub fn noninteractive_terminal_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("BITFUN_NONINTERACTIVE".to_string(), "1".to_string());
    env.insert("GIT_PAGER".to_string(), "cat".to_string());
    env.insert("PAGER".to_string(), "cat".to_string());
    env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
    env.insert("GIT_EDITOR".to_string(), "true".to_string());
    env
}
