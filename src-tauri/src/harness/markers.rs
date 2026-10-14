//! Termo's section of a Markdown file: the lines between two HTML comments,
//! replaced where they are and never anything around them.

pub const START: &str = "<!-- termo:harness:start -->";
pub const END: &str = "<!-- termo:harness:end -->";

fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Where the section is, as byte offsets: from its first line's start to
/// just past its last line's line break.
fn find(text: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    let mut start = None;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if start.is_none() && trimmed == START {
            start = Some(offset);
        } else if trimmed == END {
            if let Some(start) = start {
                return Some((start, offset + line.len()));
            }
        }
        offset += line.len();
    }
    None
}

/// The file with its section set to `body`, or taken out when `body` is
/// `None`; `None` back when nothing at all would be left in it. The file
/// keeps its own line endings.
pub fn replace(text: Option<&str>, body: Option<&str>) -> Option<String> {
    let text = text.unwrap_or("");
    let eol = line_ending(text);
    let block = body.map(|body| {
        let body = body.replace("\r\n", "\n").replace('\n', eol);
        format!("{START}{eol}{body}{eol}{END}{eol}")
    });
    let out = match (find(text), block) {
        (Some((start, end)), Some(block)) => format!("{}{block}{}", &text[..start], &text[end..]),
        (Some((start, end)), None) => {
            // Also the blank line that set the section apart when it was added.
            let before = &text[..start];
            let before = before
                .strip_suffix(eol)
                .filter(|rest| rest.is_empty() || rest.ends_with('\n'))
                .unwrap_or(before);
            format!("{before}{}", &text[end..])
        }
        (None, Some(block)) if text.trim().is_empty() => block,
        (None, Some(block)) => {
            let separator = if text.ends_with('\n') { eol.to_string() } else { format!("{eol}{eol}") };
            format!("{text}{separator}{block}")
        }
        (None, None) => text.to_string(),
    };
    (!out.trim().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_then_removing_gives_the_file_back() {
        for original in ["# Notes\n\nKeep it short.\n", "# Notes\r\n\r\nKeep it short.\r\n"] {
            let added = replace(Some(original), Some("rule one\nrule two")).unwrap();
            assert!(added.starts_with(original));
            let removed = replace(Some(&added), None).unwrap();
            assert_eq!(removed, original);
        }
    }

    #[test]
    fn a_last_line_without_a_break_gets_one() {
        let added = replace(Some("last"), Some("x")).unwrap();
        assert!(added.starts_with("last\n\n"));
        assert_eq!(replace(Some(&added), None).unwrap(), "last\n");
    }

    #[test]
    fn a_section_is_replaced_where_it_is() {
        let text = format!("top\n\n{START}\nold\n{END}\nbottom\n");
        assert_eq!(replace(Some(&text), Some("new")).unwrap(), format!("top\n\n{START}\nnew\n{END}\nbottom\n"));
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let added = replace(Some("a\r\n"), Some("x\ny")).unwrap();
        assert_eq!(added, format!("a\r\n\r\n{START}\r\nx\r\ny\r\n{END}\r\n"));
    }

    #[test]
    fn a_file_with_only_the_section_goes_away() {
        let created = replace(None, Some("x")).unwrap();
        assert_eq!(created, format!("{START}\nx\n{END}\n"));
        assert_eq!(replace(Some(&created), None), None);
        assert_eq!(replace(None, None), None);
    }
}
