use crate::define_readonly_subagent;

define_readonly_subagent!(
    FileFinderAgent,
    "FileFinder",
    "FileFinder",
    r#"Agent specialized for semantically searching and locating relevant files and directories.
Output: File paths, line ranges (optional), and brief descriptions. You need to read the files yourself after receiving the results. This is very helpful to avoid information loss.
Usage: Just describe what you want to find. Do NOT specify output format.
Recommended for: finding files based on semantic descriptions, content concepts, or when you don't know exact filenames.

Examples:
- "Find files that implement authentication"
- "Locate files that define the UI layout of the login page"  
- "Search for files related to error handling""#,
    "file_finder_agent",
    &["LS", "Read", "Grep", "Glob"]
);
