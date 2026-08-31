use super::*;

fn collect(
    pattern: &str,
    text: &str,
    multiline: bool,
    context: usize,
    budget: Option<usize>,
) -> (Vec<String>, usize) {
    let options = GrepOptions::new(pattern, "/repo/file").multiline(multiline);
    let matcher = build_grep_matcher(&options).unwrap();
    let sink = GrepSink::new(
        OutputMode::Content,
        true,
        context,
        context,
        None,
        PathBuf::from("/repo/file"),
        None,
    )
    .with_output_budget(budget);
    build_grep_searcher(context, context, multiline)
        .search_slice(&matcher, text.as_bytes(), sink.clone())
        .unwrap();
    let lines = sink
        .take_output_lines()
        .into_iter()
        .flat_map(|line| {
            line.lines()
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    (lines, sink.get_match_count())
}

#[test]
fn output_budget_keeps_exact_prefix_and_counts_with_context_and_multiline() {
    for (pattern, text, multiline, context) in [
        (
            "needle",
            "before\nneedle\nafter\nskip\nskip\nneedle\nafter\n",
            false,
            1,
        ),
        ("needle", "needle\nneedle\nneedle\n", false, 0),
        (
            r"start[\s\S]*?end",
            "start\n\nbody\nend\ngap\ngap\nstart\nend\n",
            true,
            1,
        ),
    ] {
        let (expected, count) = collect(pattern, text, multiline, context, None);
        for budget in 0..=expected.len() + 1 {
            let (actual, actual_count) = collect(pattern, text, multiline, context, Some(budget));
            assert_eq!(
                actual,
                expected.iter().take(budget).cloned().collect::<Vec<_>>()
            );
            assert_eq!(actual_count, count, "retention must not stop matching");
        }
    }
}

#[test]
fn small_output_budget_does_not_retain_all_matching_lines() {
    let text = "needle and content\n".repeat(100_000);
    let (lines, matches) = collect("needle", &text, false, 0, Some(2));
    assert_eq!(lines.len(), 2);
    assert_eq!(matches, 100_000);
}

fn result_fixture() -> Vec<GrepFileResult> {
    (0..8)
        .map(|index| GrepFileResult {
            path: PathBuf::from(format!("/repo/{index:02}.txt")),
            display_path: Some(format!("display-{:02}", 7 - index)),
            file_matches: index + 1,
            output_lines: vec![
                format!("{index}:first\n\n{index}:second"),
                format!("{index}:third"),
            ],
            modified_time: SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs((index % 3) as u64),
        })
        .collect()
}

#[test]
fn global_collector_preserves_all_modes_sorting_pagination_and_full_totals() {
    let files = result_fixture();
    for mode in [
        OutputMode::Content,
        OutputMode::Count,
        OutputMode::FilesWithMatches,
    ] {
        for offset in [0, 1, 7, 20, 50] {
            for limit in [None, Some(0), Some(1), Some(4)] {
                let mut options = GrepOptions::new("needle", "/repo")
                    .output_mode(mode)
                    .offset(offset);
                options.head_limit = limit;
                let expected = reduce_grep_results(&options, files.clone(), false).unwrap();
                for order in [
                    vec![0, 1, 2, 3, 4, 5, 6, 7],
                    vec![7, 6, 5, 4, 3, 2, 1, 0],
                    vec![3, 0, 6, 1, 7, 4, 2, 5],
                ] {
                    let mut collector = GrepResultCollector::new(&options);
                    for index in order {
                        collector.push(files[index].clone());
                        if let Some(budget) = grep_output_budget(&options) {
                            assert!(collector.retained_units <= budget);
                            assert!(collector.files.len() <= budget);
                        }
                    }
                    let actual = collector.finish(&options, false).unwrap();
                    assert_eq!(
                        actual.result_text, expected.result_text,
                        "{mode:?} offset={offset} limit={limit:?}"
                    );
                    assert_eq!(actual.applied_limit, expected.applied_limit);
                    assert_eq!(actual.applied_offset, expected.applied_offset);
                    assert_eq!(actual.file_count, 8);
                    assert_eq!(actual.total_matches, 36);
                    assert!(!actual.cancelled);
                }
            }
        }
    }
}

#[test]
fn global_content_retention_does_not_grow_with_matching_file_count() {
    let options = GrepOptions::new("needle", "/repo").offset(2).head_limit(4);
    let mut collector = GrepResultCollector::new(&options);
    for index in (0..2_000).rev() {
        collector.push(GrepFileResult {
            path: PathBuf::from(format!("/repo/{index:04}.txt")),
            display_path: None,
            file_matches: 40,
            output_lines: (0..40)
                .map(|line| format!("{index:04}:{line}:needle"))
                .collect(),
            modified_time: SystemTime::UNIX_EPOCH,
        });
        assert!(collector.retained_units <= 7);
        assert!(
            collector
                .files
                .values()
                .map(|file| file.output_lines.len())
                .sum::<usize>()
                <= 7
        );
    }
    let result = collector.finish(&options, false).unwrap();
    assert_eq!(result.file_count, 2_000);
    assert_eq!(result.total_matches, 80_000);
    assert_eq!(
        result.result_text,
        "0000:2:needle\n0000:3:needle\n0000:4:needle\n0000:5:needle"
    );
    assert_eq!(result.applied_limit, Some(4));
    assert_eq!(result.applied_offset, Some(2));
}

#[test]
fn rg_candidate_protocol_rejects_banner_truncation_and_status_mismatch() {
    let frame =
        |payload: &str| format!("BITFUN_RG_CANDIDATES_BEGIN\0{payload}BITFUN_RG_CANDIDATES_END\0");
    let path = "/repo/quote'\n\\file";
    assert_eq!(
        parse_rg_candidates(&frame(&format!("{path}\0")), 0, "/repo").unwrap(),
        HashSet::from([path.to_string()])
    );
    assert!(parse_rg_candidates(&frame(""), 1, "/repo")
        .unwrap()
        .is_empty());
    assert_eq!(
        parse_rg_candidates(&frame("./a.py\0"), 0, ".").unwrap(),
        HashSet::from(["a.py".to_string()])
    );
    for (output, status) in [
        (format!("Welcome\n{}", frame("/repo/file\0")), 0),
        (frame("/repo/file\0Welcome\n"), 0),
        (frame("/elsewhere/file\0"), 0),
        (frame("/repo/file\0"), 1),
        (frame(""), 0),
        ("BITFUN_RG_CANDIDATES_BEGIN\0/repo/file\0".to_string(), 0),
    ] {
        assert!(
            parse_rg_candidates(&output, status, "/repo").is_err(),
            "{output:?}"
        );
    }
}
