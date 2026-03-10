use std::fs;
use std::path::{Path, PathBuf};

fn collect_rs_files(root: &Path, files: &mut Vec<PathBuf>) {
    if root.is_file() {
        if root.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(root.to_path_buf());
        }
        return;
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            collect_rs_files(&entry.path(), files);
        }
    }
}

#[test]
fn targeted_workflow_paths_do_not_use_raw_send_message_calls() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let targets = [
        manifest_dir.join("src/services/plan_mode"),
        manifest_dir.join("src/services/task_mode"),
        manifest_dir.join("src/services/strategy"),
        manifest_dir.join("src/services/quality_gates"),
        manifest_dir.join("src/services/spec_interview"),
        manifest_dir.join("src/commands/plan_mode"),
        manifest_dir.join("src/commands/task_mode"),
        manifest_dir.join("src/commands/spec_interview.rs"),
    ];

    let mut files = Vec::new();
    for target in targets {
        collect_rs_files(&target, &mut files);
    }

    let offenders: Vec<String> = files
        .into_iter()
        .filter(|path| !path.ends_with("src/services/analytics/tracked_llm.rs"))
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            if content.contains(".send_message(") {
                Some(
                    path.strip_prefix(&manifest_dir)
                        .unwrap_or(path.as_path())
                        .display()
                        .to_string(),
                )
            } else {
                None
            }
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "raw .send_message(...) calls are not allowed in tracked workflow paths: {:?}",
        offenders
    );
}
