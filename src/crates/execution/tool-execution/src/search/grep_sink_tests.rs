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
