use std::{
    fmt::Write as _,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use crate::error::Result;

use super::{BenchReport, RunnerSummary};

pub(super) fn print_report(report: &BenchReport) {
    print!("{}", render_report(report));
}

pub(super) fn render_report(report: &BenchReport) -> String {
    let mut out = String::new();

    for build in &report.corpus_builds {
        let _ = writeln!(
            out,
            "build {} [{}]: {:.3}s, docs={}",
            build.corpus,
            build.tokenizer.as_str(),
            build.duration_secs,
            build.docs_indexed
        );
    }
    if !report.corpus_builds.is_empty() && !report.summaries.is_empty() {
        out.push('\n');
    }

    for (index, summary) in report.summaries.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let header = format!(
            "{} (target: {}, pattern: {})",
            summary.name, summary.target, summary.pattern
        );
        let _ = writeln!(out, "{header}");
        let _ = writeln!(out, "{}", "-".repeat(header.len()));
        let mut runners = summary.runners.iter().collect::<Vec<_>>();
        runners.sort_by_key(|runner| runner_sort_key(&runner.runner));
        for runner in runners {
            let _ = writeln!(
                out,
                "{}  mean={:.3}s stddev={:.3}s min={:.3}s samples={} candidates={} matches={}",
                runner_display_name(&runner.runner),
                runner.mean_secs,
                runner.stddev_secs,
                runner.min_secs,
                runner.sample_count,
                runner
                    .candidate_docs
                    .map_or_else(|| "-".to_string(), |count| count.to_string()),
                runner.match_count
            );
        }
    }

    out
}

pub(super) fn build_summary(
    runner: String,
    durations: Vec<f64>,
    candidate_docs: Option<usize>,
    match_count: usize,
) -> RunnerSummary {
    let mean_secs = mean(&durations);
    let stddev_secs = stddev(&durations, mean_secs);
    let min_secs = durations
        .iter()
        .copied()
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);

    RunnerSummary {
        runner,
        mean_secs,
        stddev_secs,
        min_secs,
        sample_count: durations.len(),
        candidate_docs,
        match_count,
    }
}

pub(super) fn mean_usize(total: usize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        total / count
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f64>()
        / (values.len() as f64 - 1.0);
    variance.sqrt()
}

pub(super) struct RawWriter {
    writer: BufWriter<File>,
}

impl RawWriter {
    pub(super) fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(
            writer,
            "benchmark,corpus,runner,runner_family,runner_mode,iteration,duration_secs,candidate_docs,match_count"
        )?;
        Ok(Self { writer })
    }

    pub(super) fn write_sample(&mut self, sample: RawSample<'_>) -> Result<()> {
        let runner_label = classify_runner(sample.runner);
        writeln!(
            self.writer,
            "{},{},{},{},{},{},{:.9},{},{}",
            sample.benchmark,
            sample.corpus,
            sample.runner,
            runner_label.family,
            runner_label.mode,
            sample.iteration,
            sample.duration_secs,
            sample
                .candidate_docs
                .map(|count| count.to_string())
                .unwrap_or_default(),
            sample.match_count
        )?;
        self.writer.flush()?;
        Ok(())
    }
}

pub(super) struct RawSample<'a> {
    pub(super) benchmark: &'a str,
    pub(super) corpus: &'a str,
    pub(super) runner: &'a str,
    pub(super) iteration: usize,
    pub(super) duration_secs: f64,
    pub(super) candidate_docs: Option<usize>,
    pub(super) match_count: usize,
}

fn runner_sort_key(runner: &str) -> usize {
    match runner {
        "codgrep" => 0,
        "rg" => 1,
        "codgrep_worktree_build" => 2,
        "codgrep_worktree" => 3,
        _ => 4,
    }
}

fn runner_display_name(runner: &str) -> &'static str {
    classify_runner(runner).display
}

fn classify_runner(runner: &str) -> RunnerLabel {
    match runner {
        "codgrep" => RunnerLabel {
            display: "codgrep [daemon-steady-state]",
            family: "codgrep",
            mode: "daemon_steady_state",
        },
        "codgrep_worktree_build" => RunnerLabel {
            display: "codgrep_worktree_build [dirty-first-query]",
            family: "codgrep",
            mode: "dirty_first_query",
        },
        "codgrep_worktree" => RunnerLabel {
            display: "codgrep_worktree [dirty-cached-query]",
            family: "codgrep",
            mode: "dirty_cached_query",
        },
        "rg" => RunnerLabel {
            display: "rg [scan]",
            family: "rg",
            mode: "scan",
        },
        _ => RunnerLabel {
            display: "unknown",
            family: "unknown",
            mode: "unknown",
        },
    }
}

struct RunnerLabel {
    display: &'static str,
    family: &'static str,
    mode: &'static str,
}
