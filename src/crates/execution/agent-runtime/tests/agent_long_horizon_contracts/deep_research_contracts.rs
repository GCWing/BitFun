use bitfun_agent_runtime::deep_research::renumber_research_report;

#[test]
fn deep_research_citation_renumber_owner_preserves_report_and_display_map_contracts() {
    let report = "\
# Deep Research Report

> Summary mentioning cit_005 first.

## Findings

- Cited claim with cit_001 here.
- A pair: [cit_005, cit_001].
- Rejected reference cit_002 should be flagged.

## Citation Index

| ID | Claim | Source |
|----|-------|--------|
| cit_001 | claim a | u1 |
| cit_002 | claim b | u2 |
| cit_005 | claim c | u3 |
";
    let registry = "\
cit_001 | claim a | url=u1 | authority=high | status=ACCEPTED
cit_002 | claim b | url=u2 | authority=low | status=REJECTED | reason=contradicted
cit_005 | claim c | url=u3 | authority=medium
";

    let output = renumber_research_report(report, Some(registry));

    assert_eq!(output.stats.citations_renumbered, 2);
    assert_eq!(output.stats.rejected_refs_in_body, 1);
    assert_eq!(output.stats.rejected_index_rows_dropped, 1);

    assert!(output.report.contains("mentioning [1] first"));
    assert!(output.report.contains("claim with [2] here"));
    assert!(output.report.contains("A pair: [1, 2]"));
    assert!(output.report.contains("cit_002 (rejected)"));

    let index_section = output
        .report
        .split("## Citation Index")
        .nth(1)
        .expect("citation index");
    assert!(index_section.contains("[1] cit_005"));
    assert!(index_section.contains("[2] cit_001"));
    assert!(!index_section.contains("cit_002"));

    let entries = output
        .display_map
        .iter()
        .map(|entry| (entry.display.as_str(), entry.internal.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![("[1]", "cit_005"), ("[2]", "cit_001")]);
}

#[test]
fn deep_research_citation_renumber_owner_is_idempotent_without_citations() {
    let report = "# Report\n\nNo internal citation ids here.\n";
    let output = renumber_research_report(report, None);

    assert_eq!(output.report, report);
    assert_eq!(output.stats.citations_renumbered, 0);
    assert!(output.display_map.is_empty());
}
