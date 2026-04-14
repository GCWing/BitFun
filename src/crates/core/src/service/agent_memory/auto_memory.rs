use crate::util::errors::*;
use log::debug;
use std::path::{Path, PathBuf};
use tokio::fs;

const MEMORY_DIR_NAME: &str = "memory";
const BITFUN_DIR_NAME: &str = ".bitfun";
const MEMORY_INDEX_FILE: &str = "memory.md";
const MEMORY_INDEX_TEMPLATE: &str = "# Memory Index\n";
const MEMORY_INDEX_MAX_LINES: usize = 200;
const TOPIC_MEMORY_MAX_FILES: usize = 30;

fn memory_dir_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(BITFUN_DIR_NAME).join(MEMORY_DIR_NAME)
}

fn format_path_for_prompt(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

async fn ensure_markdown_placeholder(path: &Path, content: &str) -> BitFunResult<bool> {
    if path.exists() {
        return Ok(false);
    }

    fs::write(path, content)
        .await
        .map_err(|e| BitFunError::service(format!("Failed to create {}: {}", path.display(), e)))?;

    Ok(true)
}

async fn list_memory_files(memory_dir: &Path) -> BitFunResult<Vec<String>> {
    let mut memory_files = Vec::new();
    let mut entries = fs::read_dir(memory_dir).await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to read memory directory {}: {}",
            memory_dir.display(),
            e
        ))
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        BitFunError::service(format!(
            "Failed to iterate memory directory {}: {}",
            memory_dir.display(),
            e
        ))
    })? {
        let file_type = entry.file_type().await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to inspect memory entry {}: {}",
                entry.path().display(),
                e
            ))
        })?;
        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.ends_with(".md") {
            memory_files.push(file_name);
        }
    }

    memory_files.sort();

    Ok(memory_files)
}

pub(crate) async fn ensure_workspace_memory_files_for_prompt(
    workspace_root: &Path,
) -> BitFunResult<()> {
    let memory_dir = memory_dir_path(workspace_root);
    if !memory_dir.exists() {
        fs::create_dir_all(&memory_dir).await.map_err(|e| {
            BitFunError::service(format!(
                "Failed to create memory directory {}: {}",
                memory_dir.display(),
                e
            ))
        })?;
    }
    let created_memory_index =
        ensure_markdown_placeholder(&memory_dir.join(MEMORY_INDEX_FILE), MEMORY_INDEX_TEMPLATE)
            .await?;

    debug!(
        "Ensured workspace agent memory files: path={}, created_memory_index={}",
        workspace_root.display(),
        created_memory_index
    );

    Ok(())
}

pub(crate) async fn build_workspace_agent_memory_prompt(
    workspace_root: &Path,
) -> BitFunResult<String> {
    ensure_workspace_memory_files_for_prompt(workspace_root).await?;

    let memory_dir = memory_dir_path(workspace_root);
    let memory_dir_display = format_path_for_prompt(&memory_dir);

    Ok(format!(
        r#"# auto memory

You have a persistent, file-based memory system at `{memory_dir_display}`. This directory already exists — write to it directly with the Write/Edit tool (do not run mkdir or check for its existence).

You should build up this memory system over time so that future conversations can have a complete picture of who the user is, how they'd like to collaborate with you, what behaviors to avoid or repeat, and the context behind the work the user gives you.

If the user explicitly asks you to remember something, save it immediately as whichever type fits best. If they ask you to forget something, find and remove the relevant entry.

## Types of memory

There are several discrete types of memory that you can store in your memory system:

<types>
<type>
    <name>user</name>
    <description>Contain information about the user's role, goals, responsibilities, and knowledge. Great user memories help you tailor your future behavior to the user's preferences and perspective. Your goal in reading and writing these memories is to build up an understanding of who the user is and how you can be most helpful to them specifically. For example, you should collaborate with a senior software engineer differently than a student who is coding for the very first time. Keep in mind, that the aim here is to be helpful to the user. Avoid writing memories about the user that could be viewed as a negative judgement or that are not relevant to the work you're trying to accomplish together.</description>
    <when_to_save>When you learn any details about the user's role, preferences, responsibilities, or knowledge</when_to_save>
    <how_to_use>When your work should be informed by the user's profile or perspective. For example, if the user is asking you to explain a part of the code, you should answer that question in a way that is tailored to the specific details that they will find most valuable or that helps them build their mental model in relation to domain knowledge they already have.</how_to_use>
    <examples>
    user: I'm a data scientist investigating what logging we have in place
    assistant: [saves user memory: user is a data scientist, currently focused on observability/logging]

    user: I've been writing Go for ten years but this is my first time touching the React side of this repo
    assistant: [saves user memory: deep Go expertise, new to React and this project's frontend — frame frontend explanations in terms of backend analogues]
    </examples>
</type>
<type>
    <name>feedback</name>
    <description>Guidance the user has given you about how to approach work — both what to avoid and what to keep doing. These are a very important type of memory to read and write as they allow you to remain coherent and responsive to the way you should approach work in the project. Record from failure AND success: if you only save corrections, you will avoid past mistakes but drift away from approaches the user has already validated, and may grow overly cautious.</description>
    <when_to_save>Any time the user corrects your approach ("no not that", "don't", "stop doing X") OR confirms a non-obvious approach worked ("yes exactly", "perfect, keep doing that", accepting an unusual choice without pushback). Corrections are easy to notice; confirmations are quieter — watch for them. In both cases, save what is applicable to future conversations, especially if surprising or not obvious from the code. Include *why* so you can judge edge cases later.</when_to_save>
    <how_to_use>Let these memories guide your behavior so that the user does not need to offer the same guidance twice.</how_to_use>
    <body_structure>Lead with the rule itself, then a **Why:** line (the reason the user gave — often a past incident or strong preference) and a **How to apply:** line (when/where this guidance kicks in). Knowing *why* lets you judge edge cases instead of blindly following the rule.</body_structure>
    <examples>
    user: don't mock the database in these tests — we got burned last quarter when mocked tests passed but the prod migration failed
    assistant: [saves feedback memory: integration tests must hit a real database, not mocks. Reason: prior incident where mock/prod divergence masked a broken migration]

    user: stop summarizing what you just did at the end of every response, I can read the diff
    assistant: [saves feedback memory: this user wants terse responses with no trailing summaries]

    user: yeah the single bundled PR was the right call here, splitting this one would've just been churn
    assistant: [saves feedback memory: for refactors in this area, user prefers one bundled PR over many small ones. Confirmed after I chose this approach — a validated judgment call, not a correction]
    </examples>
</type>
<type>
    <name>project</name>
    <description>Information that you learn about ongoing work, goals, initiatives, bugs, or incidents within the project that is not otherwise derivable from the code or git history. Project memories help you understand the broader context and motivation behind the work the user is doing within this working directory.</description>
    <when_to_save>When you learn who is doing what, why, or by when. These states change relatively quickly so try to keep your understanding of this up to date. Always convert relative dates in user messages to absolute dates when saving (e.g., "Thursday" → "2026-03-05"), so the memory remains interpretable after time passes.</when_to_save>
    <how_to_use>Use these memories to more fully understand the details and nuance behind the user's request and make better informed suggestions.</how_to_use>
    <body_structure>Lead with the fact or decision, then a **Why:** line (the motivation — often a constraint, deadline, or stakeholder ask) and a **How to apply:** line (how this should shape your suggestions). Project memories decay fast, so the why helps future-you judge whether the memory is still load-bearing.</body_structure>
    <examples>
    user: we're freezing all non-critical merges after Thursday — mobile team is cutting a release branch
    assistant: [saves project memory: merge freeze begins 2026-03-05 for mobile release cut. Flag any non-critical PR work scheduled after that date]

    user: the reason we're ripping out the old auth middleware is that legal flagged it for storing session tokens in a way that doesn't meet the new compliance requirements
    assistant: [saves project memory: auth middleware rewrite is driven by legal/compliance requirements around session token storage, not tech-debt cleanup — scope decisions should favor compliance over ergonomics]
    </examples>
</type>
<type>
    <name>reference</name>
    <description>Stores pointers to where information can be found in external systems. These memories allow you to remember where to look to find up-to-date information outside of the project directory.</description>
    <when_to_save>When you learn about resources in external systems and their purpose. For example, that bugs are tracked in a specific project in Linear or that feedback can be found in a specific Slack channel.</when_to_save>
    <how_to_use>When the user references an external system or information that may be in an external system.</how_to_use>
    <examples>
    user: check the Linear project "INGEST" if you want context on these tickets, that's where we track all pipeline bugs
    assistant: [saves reference memory: pipeline bugs are tracked in Linear project "INGEST"]

    user: the Grafana board at grafana.internal/d/api-latency is what oncall watches — if you're touching request handling, that's the thing that'll page someone
    assistant: [saves reference memory: grafana.internal/d/api-latency is the oncall latency dashboard — check it when editing request-path code]
    </examples>
</type>
</types>

## What NOT to save in memory

- Code patterns, conventions, architecture, file paths, or project structure — these can be derived by reading the current project state.
- Git history, recent changes, or who-changed-what — `git log` / `git blame` are authoritative.
- Debugging solutions or fix recipes — the fix is in the code; the commit message has the context.
- Anything already documented in AGENTS.md files.
- Ephemeral task details: in-progress work, temporary state, current conversation context.

These exclusions apply even when the user explicitly asks you to save. If they ask you to save a PR list or activity summary, ask what was *surprising* or *non-obvious* about it — that is the part worth keeping.

## How to save memories

Saving a memory is a two-step process:

**Step 1** — write the memory to its own file (e.g., `user_role.md`, `feedback_testing.md`) using this frontmatter format:

```markdown
---
name: {{{{memory name}}}}
description: {{{{one-line description — used to decide relevance in future conversations, so be specific}}}}
type: {{{{user, feedback, project, reference}}}}
---

{{{{memory content — for feedback/project types, structure as: rule/fact, then **Why:** and **How to apply:** lines}}}}
```

**Step 2** — add a pointer to that file in `memory.md`. `memory.md` is an index, not a memory — each entry should be one line, under ~150 characters: `- [Title](file.md) — one-line hook`. It has no frontmatter. Never write memory content directly into `memory.md`.

- `memory.md` is always loaded into your conversation context — lines after 200 will be truncated, so keep the index concise
- Keep the name, description, and type fields in memory files up-to-date with the content
- Organize memory semantically by topic, not chronologically
- Update or remove memories that turn out to be wrong or outdated
- Do not write duplicate memories. First check if there is an existing memory you can update before writing a new one.

## When to access memories
- When memories seem relevant, or the user references prior-conversation work.
- You MUST access memory when the user explicitly asks you to check, recall, or remember.
- If the user says to *ignore* or *not use* memory: Do not apply remembered facts, cite, compare against, or mention memory content.
- Memory records can become stale over time. Use memory as context for what was true at a given point in time. Before answering the user or building assumptions based solely on information in memory records, verify that the memory is still correct and up-to-date by reading the current state of the files or resources. If a recalled memory conflicts with current information, trust what you observe now — and update or remove the stale memory rather than acting on it.

## Before recommending from memory

A memory that names a specific function, file, or flag is a claim that it existed *when the memory was written*. It may have been renamed, removed, or never merged. Before recommending it:

- If the memory names a file path: check the file exists.
- If the memory names a function or flag: grep for it.
- If the user is about to act on your recommendation (not just asking about history), verify first.

"The memory says X exists" is not the same as "X exists now."

A memory that summarizes repo state (activity logs, architecture snapshots) is frozen in time. If the user asks about *recent* or *current* state, prefer `git log` or reading the code over recalling the snapshot.

## Memory and other forms of persistence
Memory is one of several persistence mechanisms available to you as you assist the user in a given conversation. The distinction is often that memory can be recalled in future conversations and should not be used for persisting information that is only useful within the scope of the current conversation.
- When to use or update a plan instead of memory: If you are about to start a non-trivial implementation task and would like to reach alignment with the user on your approach you should use a Plan rather than saving this information to memory. Similarly, if you already have a plan within the conversation and you have changed your approach persist that change by updating the plan rather than saving a memory.
- When to use or update tasks instead of memory: When you need to break your work in current conversation into discrete steps or keep track of your progress use tasks instead of saving to memory. Tasks are great for persisting information about the work that needs to be done in the current conversation, but memory should be reserved for information that will be useful in future conversations.
"#
    ))
}

pub(crate) async fn build_workspace_memory_files_context(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    let memory_dir = memory_dir_path(workspace_root);
    let memory_files_section = build_memory_space_files_section(&memory_dir).await?;
    if memory_files_section.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(memory_files_section))
    }
}

async fn build_memory_space_files_section(memory_dir: &Path) -> BitFunResult<String> {
    let index_path = memory_dir.join(MEMORY_INDEX_FILE);
    let memory_dir_display = format_path_for_prompt(memory_dir);
    let (index_content, index_description_suffix) = match fs::read_to_string(&index_path).await {
        Ok(content) if !content.trim().is_empty() => {
            let lines = content.lines().collect::<Vec<_>>();
            let was_truncated = lines.len() > MEMORY_INDEX_MAX_LINES;
            (
                lines
                    .into_iter()
                    .take(MEMORY_INDEX_MAX_LINES)
                    .collect::<Vec<_>>()
                    .join("\n"),
                if was_truncated {
                    format!(" Showing up to {MEMORY_INDEX_MAX_LINES} lines.")
                } else {
                    String::new()
                },
            )
        }
        _ => (String::new(), String::new()),
    };
    let index_body = if index_content.trim().is_empty() {
        "(memory.md is empty)".to_string()
    } else {
        index_content
    };

    let memory_files = list_memory_files(memory_dir).await?;

    let topic_description_suffix = if memory_files.len() > TOPIC_MEMORY_MAX_FILES {
        format!(" Showing up to {TOPIC_MEMORY_MAX_FILES} entries.")
    } else {
        String::new()
    };
    let topic_files_content = if memory_files.is_empty() {
        "(no topic memory files yet)".to_string()
    } else {
        memory_files
            .into_iter()
            .take(TOPIC_MEMORY_MAX_FILES)
            .map(|file_name| format!("- `{}`", file_name))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(format!(
        r#"# memory_files
Persistent memory files currently available in `{memory_dir_display}`.

## memory.md
High-level index for the durable workspace memory space.{index_description_suffix}
{index_body}

## topic_memory_files
Topic-oriented durable memory files available in this workspace.{topic_description_suffix}
{topic_files_content}"#
    ))
}
