//! Gitignore pattern matching on provider-relative POSIX paths.
//!
//! `ignore::Gitignore` uses host `Path`/globset candidates, which reinterpret a
//! remote filename containing a backslash on Windows. Keep rule matching in
//! bytes; IO discovery and rule precedence are owned by the workspace walker.

#[derive(Debug, Clone)]
struct Rule {
    matcher: regex::bytes::Regex,
    directory_only: bool,
    ignored: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceIgnoreRules(Vec<Rule>);

impl WorkspaceIgnoreRules {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut rules = Vec::new();
        for original in text.lines() {
            let mut line = original;
            if line.starts_with('#') {
                continue;
            }
            if !line.ends_with("\\ ") {
                line = line.trim_end();
            }
            if line.is_empty() {
                continue;
            }
            let ignored = !line.starts_with('!');
            if !ignored || line.starts_with("\\!") || line.starts_with("\\#") {
                line = &line[1..];
            }
            let anchored = line.starts_with('/');
            if anchored {
                line = &line[1..];
            }
            let directory_only = line.ends_with('/');
            if directory_only {
                line = &line[..line.len() - 1];
                if line.ends_with('\\') {
                    line = &line[..line.len() - 1];
                }
            }
            let mut pattern =
                if !anchored && !line.contains('/') && !line.starts_with("**/") && line != "**" {
                    format!("**/{line}")
                } else {
                    line.to_string()
                };
            if pattern.ends_with("/**") {
                pattern.push_str("/*");
            }
            let glob = globset::GlobBuilder::new(&pattern)
                .literal_separator(true)
                .backslash_escape(true)
                .allow_unclosed_class(true)
                .build()
                .map_err(|error| error.to_string())?;
            rules.push(Rule {
                matcher: regex::bytes::Regex::new(glob.regex())
                    .map_err(|error| error.to_string())?,
                directory_only,
                ignored,
            });
        }
        Ok(Self(rules))
    }

    pub(crate) fn matched(&self, relative_path: &str, is_dir: bool) -> Option<bool> {
        self.0
            .iter()
            .rev()
            .find(|rule| {
                (!rule.directory_only || is_dir) && rule.matcher.is_match(relative_path.as_bytes())
            })
            .map(|rule| rule.ignored)
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceIgnoreRules;

    #[test]
    fn portable_rules_follow_gitignore_grammar_without_normalizing_backslashes() {
        let rules = WorkspaceIgnoreRules::parse(
            "# comment\n*.tmp\n!keep.tmp\n/root.txt\ncache/\n\\#literal\na\\\\b.txt\n",
        )
        .unwrap();
        assert_eq!(rules.matched("nested/file.tmp", false), Some(true));
        assert_eq!(rules.matched("keep.tmp", false), Some(false));
        assert_eq!(rules.matched("root.txt", false), Some(true));
        assert_eq!(rules.matched("nested/root.txt", false), None);
        assert_eq!(rules.matched("cache", false), None);
        assert_eq!(rules.matched("cache", true), Some(true));
        assert_eq!(rules.matched("#literal", false), Some(true));
        assert_eq!(rules.matched("a\\b.txt", false), Some(true));
        assert_eq!(rules.matched("a/b.txt", false), None);
    }

    #[test]
    fn portable_rules_match_native_ignore_library_for_relative_fixtures() {
        let text = "*.tmp\n!keep.tmp\n/root.txt\ncache/\nfolder/**\n\\#literal\nspace\\ \n";
        let portable = WorkspaceIgnoreRules::parse(text).unwrap();
        let mut native = ignore::gitignore::GitignoreBuilder::new(".");
        for line in text.lines() {
            native.add_line(None, line).unwrap();
        }
        let native = native.build().unwrap();
        for (path, is_dir) in [
            ("file.tmp", false),
            ("nested/file.tmp", false),
            ("keep.tmp", false),
            ("root.txt", false),
            ("nested/root.txt", false),
            ("cache", true),
            ("cache", false),
            ("folder", true),
            ("folder/child", false),
            ("#literal", false),
            ("space ", false),
            ("visible", false),
        ] {
            let expected = match native.matched(path, is_dir) {
                ignore::Match::None => None,
                ignore::Match::Ignore(_) => Some(true),
                ignore::Match::Whitelist(_) => Some(false),
            };
            assert_eq!(portable.matched(path, is_dir), expected, "{path}");
        }
    }
}
