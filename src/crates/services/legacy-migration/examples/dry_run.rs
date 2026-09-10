use openbitfun_legacy_migration::{probe_legacy_source, MigrationRoots, ProbeLimits};

fn main() {
    let result = MigrationRoots::resolve_current_user()
        .and_then(|roots| probe_legacy_source(&roots, ProbeLimits::default()));
    match result {
        Ok(source) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&source).expect("probe result should serialize")
            );
        }
        Err(error) => {
            eprintln!("Legacy migration dry run failed: {error}");
            std::process::exit(1);
        }
    }
}
