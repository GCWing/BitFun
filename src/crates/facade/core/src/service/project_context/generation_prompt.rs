macro_rules! make_ai_guide_prompt {
    ($file_name:literal) => {
        concat!(
            r##"Please analyze this codebase and generate the content of a "##,
            $file_name,
            r##" file, which will be given to future instances of coding agents to operate in this repository.

What to add:
1. Commands that will be commonly used, such as how to build, lint, and run tests. Include the necessary commands to develop in this codebase, such as how to run a single test.
2. High-level code architecture and structure so that future instances can be productive more quickly. Focus on the "big picture" architecture that requires reading multiple files to understand.

Usage notes:
- "AGENTS.md", "CLAUDE.md", and ".github/copilot-instructions.md" serves the same purpose. If these files already exist, suggest improvements to them (but you still need to output the full content).
- When you make the initial "##,
            $file_name,
            r##", do not repeat yourself and do not include obvious instructions like "Provide helpful error messages to users", "Write unit tests for all new utilities", "Never include sensitive information (API keys, tokens) in code or commits".
- Avoid listing every component or file structure that can be easily discovered.
- Don't include generic development practices.
- If there are Cursor rules (in .cursor/rules/ or .cursorrules), make sure to include the important parts.
- If there is a README.md, make sure to include the important parts.
- Do not make up information such as "Common Development Tasks", "Tips for Development", "Support and Documentation" unless this is expressly included in other files that you read.
"##
        )
    };
}

pub const AGENTS_MD_GENERATION_PROMPT: &str = make_ai_guide_prompt!("AGENTS.md");
pub const CLAUDE_MD_GENERATION_PROMPT: &str = make_ai_guide_prompt!("CLAUDE.md");
pub const COPILOT_INSTRUCTIONS_GENERATION_PROMPT: &str =
    make_ai_guide_prompt!(".github/copilot-instructions.md");

pub const README_MD_GENERATION_PROMPT: &str = "Please analyze this codebase and generate the content of a README.md file.

You can refer to the following sections for inspiration, but determine the actual content based on the project's specific situation:

- Project overview and purpose
- Installation instructions
- Quick start / Usage examples
- Project structure
- Configuration options
- Contributing guidelines
- License information

Note: Only include sections that are relevant and meaningful for this specific project. Do not include sections that don't apply.";

pub const API_DESIGN_GENERATION_PROMPT: &str = "Please analyze this codebase and generate the content of a API-DESIGN.md file.

You can refer to the following sections for inspiration, but determine the actual content based on the project's specific situation:

- API design principles and philosophy
- Endpoint descriptions with methods, paths, and parameters
- Request/response schemas and data models
- Authentication and authorization mechanisms
- Error handling and status codes
- Rate limiting and throttling policies
- Versioning strategy

Note: Only include sections that are relevant and meaningful for this specific project. Do not include sections that don't apply.";

pub const DESIGN_SYSTEM_GENERATION_PROMPT: &str = "Please analyze this codebase and generate the content of a DESIGN-SYSTEM.md file.

You can refer to the following sections for inspiration, but determine the actual content based on the project's specific situation:

- Design principles and philosophy
- Color palette with usage guidelines
- Typography system (fonts, sizes, weights, line heights)
- Spacing and layout system (margins, paddings, grids)
- Component library overview
- Icon system
- Animation and interaction patterns

Note: Only include sections that are relevant and meaningful for this specific project. Do not include sections that don't apply.";

pub const DATABASE_DESIGN_GENERATION_PROMPT: &str = "Please analyze this codebase and generate the content of a DATABASE-DESIGN.md file.

You can refer to the following sections for inspiration, but determine the actual content based on the project's specific situation:

- Database architecture and overview
- Schema definitions with table structures
- Entity relationships
- Indexes and optimization strategies
- Data migration approach
- Backup and recovery procedures

Note: Only include sections that are relevant and meaningful for this specific project. Do not include sections that don't apply.";

pub const ARCHITECTURE_MD_GENERATION_PROMPT: &str = "Please analyze this codebase and generate the content of a ARCHITECTURE.md file.

You can refer to the following sections for inspiration, but determine the actual content based on the project's specific situation:

- System overview
- Component relationships
- Data flow diagrams (using mermaid)
- Design decisions and rationale

Note: Only include sections that are relevant and meaningful for this specific project. Do not include sections that don't apply.";
