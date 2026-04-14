use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use codgrep::{advanced::IndexSearcher, QueryConfig, SearchMode};

const RG_CHUNK_SIZE: usize = 512;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut repo: Option<PathBuf> = None;
    let mut index: Option<PathBuf> = None;
    let mut pattern: Option<String> = None;
    let mut case_insensitive = false;
    let mut skip_rg = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => repo = args.next().map(PathBuf::from),
            "--index" => index = args.next().map(PathBuf::from),
            "--pattern" => pattern = args.next(),
            "-i" | "--ignore-case" => case_insensitive = true,
            "--skip-rg" => skip_rg = true,
            other => {
                return Err(format!("unknown arg: {other}").into());
            }
        }
    }

    let repo = repo.ok_or("--repo is required")?;
    let index = index.unwrap_or_else(|| repo.join(".codgrep-index"));
    let pattern = pattern.ok_or("--pattern is required")?;
    let config = QueryConfig {
        regex_pattern: pattern.clone(),
        patterns: vec![pattern.clone()],
        case_insensitive,
        search_mode: SearchMode::CountOnly,
        ..QueryConfig::default()
    };

    let searcher = IndexSearcher::open(index.clone())?;

    let candidate_started = Instant::now();
    let candidate_paths = searcher.candidate_paths(&config)?;
    let candidate_secs = candidate_started.elapsed().as_secs_f64();

    let full_started = Instant::now();
    let full = searcher.search(&config)?;
    let full_secs = full_started.elapsed().as_secs_f64();

    let (rg_match_count, rg_secs) = if skip_rg {
        (None, None)
    } else {
        let rg_started = Instant::now();
        let rg_match_count = run_rg_count(&repo, &pattern, case_insensitive, &candidate_paths)?;
        let rg_secs = rg_started.elapsed().as_secs_f64();
        (Some(rg_match_count), Some(rg_secs))
    };

    println!("repo={}", repo.display());
    println!("index={}", index.display());
    println!("pattern={pattern}");
    println!("candidate_docs={}", candidate_paths.len());
    println!("codgrep_total_secs={full_secs:.6}");
    println!("candidate_secs={candidate_secs:.6}");
    println!(
        "codgrep_verify_estimated_secs={:.6}",
        (full_secs - candidate_secs).max(0.0)
    );
    if let Some(rg_secs) = rg_secs {
        println!("rg_verify_secs={rg_secs:.6}");
        println!(
            "projected_total_with_rg_verify_secs={:.6}",
            candidate_secs + rg_secs
        );
    }
    println!("codgrep_match_lines={}", full.matched_lines);
    if let Some(rg_match_count) = rg_match_count {
        println!("rg_match_lines={rg_match_count}");
    }

    if let Some(rg_match_count) = rg_match_count {
        if full.matched_lines != rg_match_count {
            return Err(format!(
                "count mismatch: codgrep={} rg={}",
                full.matched_lines, rg_match_count
            )
            .into());
        }
    }

    Ok(())
}

fn run_rg_count(
    repo: &Path,
    pattern: &str,
    case_insensitive: bool,
    candidate_paths: &[String],
) -> Result<usize, Box<dyn std::error::Error>> {
    if candidate_paths.is_empty() {
        return Ok(0);
    }

    let mut total = 0usize;
    for chunk in candidate_paths.chunks(RG_CHUNK_SIZE) {
        let mut command = Command::new("rg");
        command
            .current_dir(repo)
            .arg("--color")
            .arg("never")
            .arg("--no-heading")
            .arg("--with-filename")
            .arg("--count")
            .arg("--no-messages");
        if case_insensitive {
            command.arg("--ignore-case");
        }
        command.arg("-e").arg(pattern).arg("--");
        for path in chunk {
            command.arg(path);
        }

        let output = command.output()?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(format!(
                "rg failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        for line in String::from_utf8(output.stdout)?.lines() {
            let count = line
                .rsplit_once(':')
                .ok_or_else(|| format!("unexpected rg count output: {line}"))?
                .1
                .parse::<usize>()?;
            total += count;
        }
    }
    Ok(total)
}
