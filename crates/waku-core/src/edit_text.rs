//! Matching model-proposed file edits against the file on disk.

/// Whether `old`, a model's copy of a file it read, still names `current`.
/// Models routinely drop the final newline or write LF for a CRLF file;
/// neither means the file changed since it was read.
pub(crate) fn same_base(current: &str, old: &str) -> bool {
    fn core(text: &str) -> String {
        text.replace("\r\n", "\n").trim_end_matches('\n').to_owned()
    }
    current == old || core(current) == core(old)
}

/// `new` in the line-ending style and final newline of `current`, so an edit
/// written from a model's LF, newline-trimmed copy keeps the file's shape.
pub(crate) fn in_file_style(current: &str, new: &str) -> String {
    if current.is_empty() || new.is_empty() {
        return new.to_owned();
    }
    let mut text = new.replace("\r\n", "\n");
    if current.ends_with('\n') && !text.ends_with('\n') {
        text.push('\n');
    }
    let crlf = current.contains("\r\n") && !current.replace("\r\n", "").contains('\n');
    if crlf {
        text.replace('\n', "\r\n")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_final_newline_or_lf_copy_still_matches() {
        assert!(same_base("a\nb\n", "a\nb"));
        assert!(same_base("a\r\nb\r\n", "a\nb\n"));
        assert!(same_base("", ""));
        assert!(!same_base("a\nb\n", "a\nc"));
        assert!(!same_base("a\n\nb", "a\nb"));
    }

    #[test]
    fn edits_keep_the_files_line_endings_and_final_newline() {
        assert_eq!(in_file_style("a\n", "a\nb"), "a\nb\n");
        assert_eq!(in_file_style("a\r\n", "a\nb"), "a\r\nb\r\n");
        assert_eq!(in_file_style("a", "a\nb"), "a\nb");
        assert_eq!(in_file_style("", "new"), "new");
        assert_eq!(in_file_style("mixed\r\nlf\n", "x\ny"), "x\ny\n");
    }
}
