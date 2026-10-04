use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const ELLIPSIS: &str = "…";

pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(crate) fn fit(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }

    let ellipsis_width = display_width(ELLIPSIS);
    if width < ellipsis_width {
        return String::new();
    }

    let content_width = width - ellipsis_width;
    let mut used = 0;
    let mut out = String::new();
    for grapheme in UnicodeSegmentation::graphemes(text, true) {
        let grapheme_width = display_width(grapheme);
        if used + grapheme_width > content_width {
            break;
        }
        out.push_str(grapheme);
        used += grapheme_width;
    }
    out.push_str(ELLIPSIS);
    out
}

pub(crate) fn pad_left(text: &str, width: usize) -> String {
    let visible = display_width(text);
    format!("{}{text}", " ".repeat(width.saturating_sub(visible)))
}

pub(crate) fn pad_right(text: &str, width: usize) -> String {
    let visible = display_width(text);
    format!("{text}{}", " ".repeat(width.saturating_sub(visible)))
}

/// Measures text decorated with the SGR sequences emitted by this crate.
///
/// This intentionally is not a general ANSI parser. Non-SGR terminal control
/// sequences are outside the TUI layout contract.
pub(crate) fn styled_width(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut width = 0;
    let mut visible_start = 0;
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\x1b'
            && bytes.get(index + 1) == Some(&b'[')
            && let Some(end) = sgr_end(bytes, index + 2)
        {
            width += display_width(&text[visible_start..index]);
            index = end;
            visible_start = end;
        } else {
            index += 1;
        }
    }

    width + display_width(&text[visible_start..])
}

pub(crate) fn pad_right_styled(text: &str, width: usize) -> String {
    let visible = styled_width(text);
    format!("{text}{}", " ".repeat(width.saturating_sub(visible)))
}

fn sgr_end(bytes: &[u8], mut index: usize) -> Option<usize> {
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'm' => return Some(index + 1),
            b'0'..=b'9' | b';' => index += 1,
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_uses_terminal_cells() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("进程"), 4);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn fit_uses_cell_budget_and_preserves_graphemes() {
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("你好世界", 5), "你好…");
        assert_eq!(fit("e\u{301}xyz", 3), "e\u{301}x…");
        assert_eq!(display_width(&fit("你好世界", 5)), 5);
    }

    #[test]
    fn fit_accounts_for_ellipsis_width() {
        let width = display_width(ELLIPSIS);
        assert_eq!(display_width(&fit("ab", width)), width);
    }

    #[test]
    fn padding_uses_terminal_cells() {
        assert_eq!(pad_right("进程", 6), "进程  ");
        assert_eq!(pad_left("进程", 6), "  进程");
    }

    #[test]
    fn styled_width_ignores_owned_sgr_sequences() {
        assert_eq!(styled_width("\x1b[1m进程\x1b[0m  5%"), 8);
        assert_eq!(
            pad_right_styled("\x1b[1m进程\x1b[0m", 6),
            "\x1b[1m进程\x1b[0m  "
        );
    }
}
