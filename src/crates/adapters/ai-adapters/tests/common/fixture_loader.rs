use std::path::PathBuf;

pub fn load_fixture_bytes(relative_path: &str) -> Vec<u8> {
    let fixture_path = fixtures_root().join(relative_path);
    std::fs::read(&fixture_path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {}", fixture_path.display(), err))
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}
