Analyze this OpenBitFun usage data and suggest improvements.

## OPENBITFUN FEATURES REFERENCE (pick from these for features_to_try):

1. **Skills**: Create reusable prompt templates as markdown files that run with a single button or command.
   - How to use: Create `.openbitfun/skills/commit/SKILL.md` with instructions. Then trigger it from the Skills panel.
   - Good for: repetitive workflows - commit messages, code reviews, testing, deployment, or complex multi-step workflows
   - Example SKILL.md content:
     ```markdown
     # Commit Skill
     Review all staged changes with `git diff --cached`.
     Write a conventional commit message following the project's style.
     Run `git commit -m "<message>"` and report the result.
     ```
   - Advanced: Skills can reference other files, include conditional logic, and chain multiple steps.
   - Authoring new skills: Invoke the built-in `writing-skills` skill for guidance on creating well-structured skill files.

2. **SubAgents (Task Agents)**: Custom agents you define for specific domains or tasks. SubAgents run in parallel and return results to the parent agent.
   - How to use: Create agents in `.openbitfun/agents/` with custom prompts and tool configurations.
   - Good for: domain-specific tasks, parallel exploration, focused code review
   - Example agent config (`.openbitfun/agents/security-reviewer/agent.json`):
     ```json
     {
       "name": "Security Reviewer",
       "description": "Reviews code for security vulnerabilities",
       "prompt_file": "prompt.md",
       "tools": ["Read", "Grep", "Glob"]
     }
     ```
   - Parallel exploration: Launch multiple SubAgents to investigate different parts of the codebase simultaneously, then synthesize their findings.

3. **MCP Servers**: Connect OpenBitFun to external tools, databases, and APIs via Model Context Protocol.
   - How to use: Configure MCP servers in settings to connect to external services.
   - Good for: database queries, API integration, connecting to internal tools
   - Common integrations:
     - **Database**: Query PostgreSQL/MySQL directly from chat — `SELECT * FROM users WHERE ...`
     - **GitHub**: Create issues, review PRs, manage releases without leaving OpenBitFun
     - **Slack/Discord**: Post messages, read channels, manage notifications
     - **Notion/Linear**: Create and update project management items
   - Example config:
     ```json
     {
       "mcpServers": {
         "postgres": {
           "command": "npx",
           "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"]
         }
       }
     }
     ```

4. **Agents and Skills**: Switch between Agentic and Cowork for different tasks, and use skills such as Plan for task-specific workflows.
   - How to use: Select the appropriate mode from the mode switcher based on your task.
   - Mode comparison:
     | Mode | Best for | AI behavior |
     |------|----------|-------------|
     | **Agentic** | Autonomous implementation | AI plans and executes independently |
     | **Cowork** | Collaborative editing | AI suggests, you approve each change |
     | **Plan** | Architecture & design | AI creates detailed plans before coding |
   - For work with independent branches, enable or invoke the built-in `multitask` skill so the Agent can coordinate parallel subagents without changing the session's main Agent.
   - Tip: Use the Plan skill for complex tasks, then ask Agentic to implement the resulting plan.

5. **CLI Exec (Headless)**: Run OpenBitFun non-interactively from scripts and CI/CD pipelines.
   - How to use: `openbitfun exec "fix lint errors" --tools "Edit,Read,ExecCommand"`
   - Good for: CI/CD integration, batch code fixes, automated reviews
   - CI/CD examples:
     ```bash
     # Pre-commit hook: auto-fix lint errors
     openbitfun exec "fix all lint errors in staged files" --tools "Edit,Read,ExecCommand"

     # PR review bot
     openbitfun exec "review changes in this PR for security issues" --tools "Read,Grep,Glob"

     # Automated documentation
     openbitfun exec "update API docs for all changed endpoints" --tools "Read,Edit,Glob"
     ```

RESPOND WITH ONLY A VALID JSON OBJECT:
{
  "openbitfun_md_additions": [
    {"section": "Section name in OPENBITFUN.md", "content": "A specific line or block to add based on workflow patterns", "rationale": "1 sentence explaining why this would help based on actual sessions"}
  ],
  "features_to_try": [
    {"feature": "Feature name from OPENBITFUN FEATURES REFERENCE above", "description": "What it does", "example_usage": "Actual command or config to copy", "benefit": "Why this would help YOU based on your sessions"}
  ],
  "usage_patterns": [
    {"pattern": "Short title", "description": "1-2 sentence summary of the pattern", "detail": "3-4 sentences explaining how this applies to YOUR work", "suggested_prompt": "A specific prompt to copy and try"}
  ]
}

IMPORTANT for openbitfun_md_additions: PRIORITIZE instructions that appear MULTIPLE TIMES in the user data. If user told AI the same thing in 2+ sessions (e.g., 'always run tests', 'use TypeScript'), that's a PRIME candidate - they shouldn't have to repeat themselves.

IMPORTANT for features_to_try: Pick 2-3 from the OPENBITFUN FEATURES REFERENCE above. Include concrete, copy-pasteable example_usage for each. Tailor the benefit to the user's actual workflow patterns.

DATA:
{aggregate_json}

SESSION SUMMARIES:
{summaries}

FRICTION DETAILS:
{friction_details}

USER INSTRUCTIONS TO AI:
{user_instructions}
