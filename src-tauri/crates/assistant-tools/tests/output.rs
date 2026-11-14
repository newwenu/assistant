use assistant_tools::os::output::{truncate_output, OutputLimits};

fn limits(max_bytes: usize, max_lines: usize, preview_bytes: usize) -> OutputLimits {
    OutputLimits::head(max_bytes, max_lines, preview_bytes)
}

#[test]
fn output_within_limits_is_returned_unchanged() {
    let text = "alpha\nbeta\ngamma\n";
    let output = truncate_output(text, &limits(1_000, 100, 0));

    assert_eq!(output.text, text);
    assert!(!output.truncated);
    assert_eq!(output.total_lines, 3);
    assert_eq!(output.total_bytes, text.len());
    assert_eq!(output.returned_lines, 3);
}

#[test]
fn line_limit_truncates_and_reports_actual_totals() {
    let text = "one\ntwo\nthree\nfour\n";
    let output = truncate_output(text, &limits(0, 2, 0));

    assert!(output.truncated);
    assert_eq!(output.total_lines, 4);
    assert_eq!(output.returned_lines, 2);
    assert!(output.text.starts_with("one\ntwo\n"));
    assert!(output.text.contains("showing 2 of 4 lines"));
    let expected_total = format!("of {} bytes", text.len());
    assert!(output.text.contains(&expected_total));
}

#[test]
fn byte_limit_truncates_and_reports_actual_totals() {
    let text = "one\ntwo\nthree\n";
    let output = truncate_output(text, &limits(8, 0, 0));

    assert!(output.truncated);
    assert_eq!(output.total_bytes, text.len());
    assert_eq!(output.returned_lines, 2);
    assert!(output.text.starts_with("one\ntwo\n"));
    assert!(output.text.contains("showing 2 of 3 lines"));
    let expected_total = format!("8 of {} bytes", text.len());
    assert!(output.text.contains(&expected_total));
}
