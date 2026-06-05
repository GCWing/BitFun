pub const AGENTS_MD_TEMPLATE: &str = r#"# AGENTS.md

This file provides guidance for AI agents working with this codebase.

## Project Overview

<!-- Describe your project here -->

## Directory Structure

<!-- Explain the key directories -->

## Development Guidelines

<!-- Add coding standards and conventions -->

## Common Tasks

<!-- List frequently performed tasks -->
"#;

pub const CLAUDE_MD_TEMPLATE: &str = r#"# CLAUDE.md

This file provides guidance for AI agents working with this codebase.

## Project Overview

<!-- Describe your project here -->

## Directory Structure

<!-- Explain the key directories -->

## Development Guidelines

<!-- Add coding standards and conventions -->

## Common Tasks

<!-- List frequently performed tasks -->
"#;

pub const COPILOT_INSTRUCTIONS_MD_TEMPLATE: &str = r#"# COPILOT_INSTRUCTIONS.md

This file provides guidance for AI agents working with this codebase.

## Project Overview

<!-- Describe your project here -->

## Directory Structure

<!-- Explain the key directories -->

## Development Guidelines

<!-- Add coding standards and conventions -->

## Common Tasks

<!-- List frequently performed tasks -->
"#;

pub const ARCHITECTURE_MD_TEMPLATE: &str = r#"# Architecture

## Overview

<!-- High-level system architecture -->

## Components

<!-- Key components and their responsibilities -->

## Data Flow

<!-- How data flows through the system -->

## Design Decisions

<!-- Important architectural decisions and rationale -->
"#;

pub const API_DESIGN_MD_TEMPLATE: &str = r#"# API Design

## Overview

<!-- API design principles -->

## Endpoints

<!-- List of API endpoints -->

## Data Models

<!-- Request/Response schemas -->

## Authentication

<!-- Authentication mechanisms -->
"#;

pub const DESIGN_SYSTEM_MD_TEMPLATE: &str = r#"# Design System

## Colors

<!-- Color palette -->

## Typography

<!-- Font families and sizes -->

## Components

<!-- UI component library -->

## Spacing & Layout

<!-- Spacing system -->
"#;

pub const DATABASE_DESIGN_MD_TEMPLATE: &str = r#"# Database Design

## Overview

<!-- Database architecture -->

## Schema

<!-- Table definitions -->

## Relationships

<!-- Entity relationships -->

## Indexes

<!-- Index strategy -->
"#;

pub const PRETTIER_TEMPLATE: &str = r#"{
  "tabWidth": 2,
  "useTabs": false
}"#;

pub const EDITORCONFIG_TEMPLATE: &str = r#"root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true

[*.{js,ts,jsx,tsx,json,md,rst,py}]
indent_style = space
indent_size = 2

[*.rs]
indent_style = space
indent_size = 4

[Makefile]
indent_style = tab
"#;

pub const RUSTFMT_TEMPLATE: &str = r#"max_width = 100
tab_spaces = 4
"#;

pub const PYLINT_TEMPLATE: &str = r#"[MASTER]
disable=
    C0111,  # missing-docstring
    C0103,  # invalid-name
    R0903,  # too-few-public-methods

[FORMAT]
max-line-length=100

[BASIC]
good-names=i,j,k,ex,Run,_
"#;

pub const BIOME_TEMPLATE: &str = r#"{
  "$schema": "https://biomejs.dev/schemas/1.9.0/schema.json",
  "formatter": {
    "indentWidth": 2
  }
}"#;

pub const ESLINT_TEMPLATE: &str = r#"export default [
  {
    rules: {
      "no-unused-vars": "warn",
    },
  },
];
"#;
