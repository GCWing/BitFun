---
name: find-skills
description: Discover and install reusable agent skills when users ask for capabilities, workflows, or domain-specific help that may already exist as an installable skill.
description_zh: 当用户询问能力、工作流或领域化需求时，帮助发现并安装可复用的技能，而不是从零实现。
allowed-tools: Bash(npx -y skills:*), Bash(npx skills:*), Bash(skills:*)
---

# Find and Install Skills

Use this skill when users ask for capabilities that might already exist as installable skills, for example:
- "is there a skill for X"
- "find me a skill for X"
- "can you help with X" where X is domain-specific or repetitive
- "how do I extend the agent for X"

## Objective

1. Understand the user's domain and task.
2. Search the skill ecosystem.
3. Present the best matching options with install commands.
4. Install only after explicit user confirmation.

## Skills CLI

The Skills CLI package manager is available via:

```bash
npx -y skills <command>
```

Key commands:
- `npx -y skills find [query]`
- `npx -y skills add <owner/repo@skill> -y`
- `npx -y skills check`
- `npx -y skills update`

Reference:
- `https://skills.sh/`

## Workflow

### 1) Clarify intent

Extract:
- Domain (react/testing/devops/docs/design/productivity/etc.)
- Specific task (e2e tests, changelog generation, PR review, deployment, etc.)
- Constraints (stack, language, local/global install preference)

### 2) Search

Run:

```bash
npx -y skills find <query>
```

Use concrete queries first (for example, `react performance`, `pr review`, `changelog`, `playwright e2e`).
If no useful results, retry with close synonyms.

### 3) Present options

For each relevant match, provide:
- Skill id/name
- What it helps with
- Popularity signal (prefer higher install count when shown by CLI output)
- Install command
- Skills page link

Template:

```text
I found a relevant skill: <owner/repo@skill>
What it does: <short description>
Install: npx -y skills add <owner/repo@skill> -y
Learn more: <skills.sh url>
```

### 4) Install (confirmation required)

Only install after user says yes.

Recommended install command:

```bash
npx -y skills add <owner/repo@skill> -g -y
```

If user does not want global install, omit `-g`.

### 5) Verify

After installation, list or check installed skills and report result clearly.

## When no skill is found

If search returns no good match:
1. Say no relevant skill was found.
2. Offer to complete the task directly.
3. Suggest creating a custom skill for recurring needs.

Example:

```text
I couldn't find a strong skill match for "<query>".
I can still handle this task directly.
If this is recurring, we can create a custom skill with:
npx -y skills init <skill-name>
```
