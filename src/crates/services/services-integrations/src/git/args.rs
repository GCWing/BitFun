use super::types::{GitChangedFilesParams, GitDiffParams};

pub fn build_git_diff_args(params: &GitDiffParams) -> Vec<String> {
    let mut args = if params.review_safe.unwrap_or(false) {
        vec![
            "--literal-pathspecs".to_string(),
            "diff".to_string(),
            "--no-ext-diff".to_string(),
            "--no-textconv".to_string(),
            "--find-renames".to_string(),
        ]
    } else {
        vec!["diff".to_string()]
    };

    if params.staged.unwrap_or(false) {
        args.push("--cached".to_string());
    }

    match (&params.source, &params.target) {
        (Some(src), Some(tgt)) => {
            args.push(format!("{}..{}", src, tgt));
        }
        (Some(src), None) => {
            args.push(src.clone());
        }
        (None, None) => {}
        (None, Some(_)) => {}
    }

    if params.stat.unwrap_or(false) {
        args.push("--stat".to_string());
    }

    if let Some(files) = &params.files {
        args.push("--".to_string());
        args.extend(files.iter().cloned());
    }

    args
}

pub fn build_git_changed_files_args(params: &GitChangedFilesParams) -> Vec<String> {
    let mut args = if params.review_safe.unwrap_or(false) {
        vec![
            "--literal-pathspecs".to_string(),
            "diff".to_string(),
            "--no-ext-diff".to_string(),
            "--no-textconv".to_string(),
            "--find-renames".to_string(),
            "--name-status".to_string(),
            "-z".to_string(),
        ]
    } else {
        vec![
            "diff".to_string(),
            "--name-status".to_string(),
            "-z".to_string(),
        ]
    };

    if params.staged.unwrap_or(false) {
        args.push("--cached".to_string());
    }

    match (&params.source, &params.target) {
        (Some(src), Some(tgt)) => {
            args.push(format!("{}..{}", src, tgt));
        }
        (Some(src), None) => {
            args.push(src.clone());
        }
        (None, Some(tgt)) => {
            args.push(tgt.clone());
        }
        (None, None) => {}
    }

    args
}
