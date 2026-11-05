use assistant_tools::storage::{
    persisted_reference, select_persist_targets, FileToolResultStorage, PersistToolResult,
    ToolResultStorage, PER_ROUND_LIMIT_CHARS, PER_TOOL_LIMIT_CHARS, PREVIEW_CHARS,
};

#[test]
fn small_results_stay_inline() {
    let targets = select_persist_targets(&[100, 200, 300]);
    assert_eq!(targets, vec![false, false, false]);
}

#[test]
fn oversized_result_persists_even_below_round_budget() {
    let targets = select_persist_targets(&[PER_TOOL_LIMIT_CHARS + 1]);
    assert_eq!(
        targets,
        vec![true],
        "per-tool limit is independent of the round budget"
    );
}

#[test]
fn round_budget_evicts_the_minimum_number_of_results() {
    let sizes = [40_000, 45_000, 44_000, 43_000, 42_000];
    let targets = select_persist_targets(&sizes);

    assert_eq!(
        targets.iter().filter(|flag| **flag).count(),
        1,
        "only one result needs to leave the inline budget"
    );
    assert_eq!(
        targets,
        vec![true, false, false, false, false],
        "descending accumulation evicts the smallest result last"
    );
}

#[test]
fn equal_sizes_evict_in_stable_order() {
    let sizes = [45_000; 5];
    let targets = select_persist_targets(&sizes);

    assert_eq!(targets.iter().filter(|flag| **flag).count(), 1);
    assert_eq!(
        targets,
        vec![false, false, false, false, true],
        "stable sort keeps original order for ties, so the tail is evicted"
    );
}

const _: () = assert!(45_000 * 5 > PER_ROUND_LIMIT_CHARS);

#[test]
fn preview_is_capped_at_preview_chars() {
    let content = "y".repeat(5_000);
    let rendered = persisted_reference("out.txt", &content);

    let preview = rendered.split('\n').nth(1).expect("preview line");
    assert_eq!(preview.chars().count(), PREVIEW_CHARS);
    assert!(rendered.starts_with("<persisted-output path=\"out.txt\">\n"));
}

#[tokio::test]
async fn file_storage_writes_the_full_content() {
    let dir = tempfile::tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());
    let content = "z".repeat(1_000);

    let reference = storage
        .persist(PersistToolResult {
            tool_call_id: "call/with:bad*chars".to_string(),
            tool_name: "probe".to_string(),
            content: content.clone(),
        })
        .await
        .unwrap();

    let written = std::fs::read_to_string(&reference.path).unwrap();
    assert_eq!(written, content);
    assert_eq!(reference.bytes, content.len());
    assert!(reference.path.ends_with(".txt"));
}
