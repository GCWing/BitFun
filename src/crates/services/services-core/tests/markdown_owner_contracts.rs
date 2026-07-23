use bitfun_services_core::markdown::FrontMatterMarkdown;
use std::fs;

#[test]
fn front_matter_markdown_preserves_metadata_and_trimmed_body_contract() {
    let content = "---\ntitle: Demo\ntags:\n  - one\n---\n\n# Body\n";

    let (metadata, body) = FrontMatterMarkdown::load_str(content).expect("front matter");
    assert_eq!(metadata["title"].as_str(), Some("Demo"));
    assert_eq!(body, "# Body\n");

    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("doc.md");
    FrontMatterMarkdown::save(path.to_str().expect("utf8 path"), &metadata, "  # Saved\n")
        .expect("save");
    let saved = fs::read_to_string(path).expect("saved");
    assert!(saved.starts_with("---\n"));
    assert!(saved.contains("title: Demo\n"));
    assert!(saved.contains("tags:\n- one\n"));
    assert!(saved.ends_with("---\n\n# Saved\n"));
}
