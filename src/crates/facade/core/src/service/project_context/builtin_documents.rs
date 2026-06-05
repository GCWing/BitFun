//! Built-in document and category definitions

use super::document_template::{
    AGENTS_MD_TEMPLATE, API_DESIGN_MD_TEMPLATE, ARCHITECTURE_MD_TEMPLATE, BIOME_TEMPLATE,
    CLAUDE_MD_TEMPLATE, COPILOT_INSTRUCTIONS_MD_TEMPLATE, DATABASE_DESIGN_MD_TEMPLATE,
    DESIGN_SYSTEM_MD_TEMPLATE, EDITORCONFIG_TEMPLATE, ESLINT_TEMPLATE, PRETTIER_TEMPLATE,
    PYLINT_TEMPLATE, RUSTFMT_TEMPLATE,
};
use super::generation_prompt::{
    AGENTS_MD_GENERATION_PROMPT, API_DESIGN_GENERATION_PROMPT, ARCHITECTURE_MD_GENERATION_PROMPT,
    CLAUDE_MD_GENERATION_PROMPT, COPILOT_INSTRUCTIONS_GENERATION_PROMPT,
    DATABASE_DESIGN_GENERATION_PROMPT, DESIGN_SYSTEM_GENERATION_PROMPT,
    README_MD_GENERATION_PROMPT,
};
use super::types::DocumentPriority;

/// Built-in document definition (static configuration)
#[derive(Debug, Clone)]
pub struct BuiltinDocument {
    /// Document ID
    pub id: &'static str,
    /// Document name
    pub name: &'static str,
    /// Category ID
    pub category_id: &'static str,
    /// Whether AI generation is supported
    pub can_generate: bool,
    /// Generation prompt
    pub generation_prompt: &'static str,
    /// Default template
    pub default_template: &'static str,
    /// Priority
    pub priority: DocumentPriority,
    /// Possible file paths (in priority order)
    pub possible_paths: &'static [&'static str],
    /// Whether the document is enabled by default when it exists
    ///
    /// Only core AI agent instruction files (AGENTS.md, CLAUDE.md,
    /// copilot-instructions.md) default to `true`; other documents
    /// require the user to opt-in explicitly.
    pub default_enabled: bool,
}

/// Built-in category ID list
pub fn get_builtin_categories() -> Vec<&'static str> {
    vec!["general", "coding", "design", "review"]
}

/// Built-in document list
pub static BUILTIN_DOCUMENTS: &[BuiltinDocument] = &[
    BuiltinDocument {
        id: "agents-md",
        name: "AGENTS.md",
        category_id: "general",
        can_generate: true,
        generation_prompt: AGENTS_MD_GENERATION_PROMPT,
        default_template: AGENTS_MD_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &["AGENTS.md"],
        default_enabled: true,
    },
    BuiltinDocument {
        id: "claude-md",
        name: "CLAUDE.md",
        category_id: "general",
        can_generate: true,
        generation_prompt: CLAUDE_MD_GENERATION_PROMPT,
        default_template: CLAUDE_MD_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &["CLAUDE.md"],
        default_enabled: true,
    },
    BuiltinDocument {
        id: "readme-md",
        name: "README.md",
        category_id: "general",
        can_generate: true,
        generation_prompt: README_MD_GENERATION_PROMPT,
        default_template: "",
        priority: DocumentPriority::High,
        possible_paths: &["README.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "copilot-instructions-md",
        name: "copilot-instructions.md",
        category_id: "general",
        can_generate: true,
        generation_prompt: COPILOT_INSTRUCTIONS_GENERATION_PROMPT,
        default_template: COPILOT_INSTRUCTIONS_MD_TEMPLATE,
        priority: DocumentPriority::Low,
        possible_paths: &[".github/copilot-instructions.md"],
        default_enabled: true,
    },
    BuiltinDocument {
        id: "editorconfig",
        name: "EditorConfig",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: EDITORCONFIG_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &[".editorconfig"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "eslint",
        name: "ESLint",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: ESLINT_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &[
            "eslint.config.js",
            "eslint.config.mjs",
            "eslint.config.cjs",
            "eslint.config.ts",
            "eslint.config.mts",
            "eslint.config.cts",
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".eslintrc.yaml",
            ".eslintrc.yml",
            ".eslintrc.json",
        ],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "prettier",
        name: "Prettier",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: PRETTIER_TEMPLATE,
        priority: DocumentPriority::Medium,
        possible_paths: &[
            ".prettierrc",
            ".prettierrc.json",
            ".prettierrc.yml",
            ".prettierrc.yaml",
            ".prettierrc.json5",
            ".prettierrc.js",
            "prettier.config.js",
            ".prettierrc.ts",
            "prettier.config.ts",
            ".prettierrc.mjs",
            "prettier.config.mjs",
            ".prettierrc.mts",
            "prettier.config.mts",
            ".prettierrc.cjs",
            "prettier.config.cjs",
            ".prettierrc.cts",
            "prettier.config.cts",
            ".prettierrc.toml",
        ],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "rustfmt",
        name: "Rustfmt",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: RUSTFMT_TEMPLATE,
        priority: DocumentPriority::Medium,
        possible_paths: &["rustfmt.toml", ".rustfmt.toml"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "biome",
        name: "Biome",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: BIOME_TEMPLATE,
        priority: DocumentPriority::Low,
        possible_paths: &["biome.json", "biome.jsonc"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "pylint",
        name: "Pylint",
        category_id: "coding",
        can_generate: false,
        generation_prompt: "",
        default_template: PYLINT_TEMPLATE,
        priority: DocumentPriority::Low,
        possible_paths: &[".pylintrc", "pylintrc", "pyproject.toml"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "architecture-md",
        name: "ARCHITECTURE.md",
        category_id: "design",
        can_generate: true,
        generation_prompt: ARCHITECTURE_MD_GENERATION_PROMPT,
        default_template: ARCHITECTURE_MD_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &[
            "ARCHITECTURE.md",
            "docs/ARCHITECTURE.md",
            "doc/ARCHITECTURE.md",
        ],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "api-design-md",
        name: "API-DESIGN.md",
        category_id: "design",
        can_generate: true,
        generation_prompt: API_DESIGN_GENERATION_PROMPT,
        default_template: API_DESIGN_MD_TEMPLATE,
        priority: DocumentPriority::High,
        possible_paths: &["API-DESIGN.md", "docs/API-DESIGN.md", "API.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "design-system-md",
        name: "DESIGN-SYSTEM.md",
        category_id: "design",
        can_generate: true,
        generation_prompt: DESIGN_SYSTEM_GENERATION_PROMPT,
        default_template: DESIGN_SYSTEM_MD_TEMPLATE,
        priority: DocumentPriority::Medium,
        possible_paths: &["DESIGN-SYSTEM.md", "docs/DESIGN-SYSTEM.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "database-design-md",
        name: "DATABASE-DESIGN.md",
        category_id: "design",
        can_generate: true,
        generation_prompt: DATABASE_DESIGN_GENERATION_PROMPT,
        default_template: DATABASE_DESIGN_MD_TEMPLATE,
        priority: DocumentPriority::Medium,
        possible_paths: &[
            "DATABASE-DESIGN.md",
            "docs/DATABASE-DESIGN.md",
            "DATABASE.md",
        ],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "codeowners",
        name: "CODEOWNERS",
        category_id: "review",
        can_generate: false,
        generation_prompt: "",
        default_template: "",
        priority: DocumentPriority::High,
        possible_paths: &["CODEOWNERS", ".github/CODEOWNERS", "docs/CODEOWNERS"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "contributing-md",
        name: "CONTRIBUTING.md",
        category_id: "review",
        can_generate: false,
        generation_prompt: "",
        default_template: "",
        priority: DocumentPriority::High,
        possible_paths: &["CONTRIBUTING.md", ".github/CONTRIBUTING.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "code-of-conduct-md",
        name: "CODE_OF_CONDUCT.md",
        category_id: "review",
        can_generate: false,
        generation_prompt: "",
        default_template: "",
        priority: DocumentPriority::Medium,
        possible_paths: &["CODE_OF_CONDUCT.md", ".github/CODE_OF_CONDUCT.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "security-md",
        name: "SECURITY.md",
        category_id: "review",
        can_generate: false,
        generation_prompt: "",
        default_template: "",
        priority: DocumentPriority::Medium,
        possible_paths: &["SECURITY.md", ".github/SECURITY.md"],
        default_enabled: false,
    },
    BuiltinDocument {
        id: "pr-template",
        name: "PULL_REQUEST_TEMPLATE.md",
        category_id: "review",
        can_generate: false,
        generation_prompt: "",
        default_template: "",
        priority: DocumentPriority::Low,
        possible_paths: &[
            "PULL_REQUEST_TEMPLATE.md",
            ".github/PULL_REQUEST_TEMPLATE.md",
        ],
        default_enabled: false,
    },
];

/// Finds a built-in document by ID.
pub fn find_builtin_document(id: &str) -> Option<&'static BuiltinDocument> {
    BUILTIN_DOCUMENTS.iter().find(|doc| doc.id == id)
}

/// Returns built-in documents for the given category.
pub fn get_documents_by_category(category_id: &str) -> Vec<&'static BuiltinDocument> {
    BUILTIN_DOCUMENTS
        .iter()
        .filter(|doc| doc.category_id == category_id)
        .collect()
}
