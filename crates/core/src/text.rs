const MAX_CHARS: usize = 4_000;
const FOOTNOTE_RULE: &str = "----------";

pub fn from_html(html: &str) -> Option<String> {
    let mut plain = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        plain.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            rest = "";
            break;
        };
        let tag = after[..end].trim().to_lowercase();
        if tag == "/p" {
            plain.push_str("\n\n");
        } else if tag.starts_with("br") {
            plain.push('\n');
        }
        rest = &after[end + 1..];
    }
    plain.push_str(rest);

    tidy(&decode_entities(&plain))
}

pub fn from_markdown(markdown: &str) -> Option<String> {
    let body: String = markdown
        .lines()
        .take_while(|line| !line.trim().starts_with(FOOTNOTE_RULE))
        .filter(|line| !is_footnote(line))
        .map(|line| line.trim_start_matches(['>', '#', ' ']))
        .collect::<Vec<_>>()
        .join("\n");
    let plain = without_links(&body).replace("**", "").replace('*', "");

    tidy(&plain.replace("([source])", "").replace("(source)", ""))
}

pub fn tidy(text: &str) -> Option<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim).chain([""]) {
        if line.is_empty() {
            if !lines.is_empty() {
                paragraphs.push(lines.join("\n"));
                lines.clear();
            }
        } else {
            lines.push(line);
        }
    }

    let tidied = paragraphs.join("\n\n");
    if tidied.is_empty() {
        return None;
    }
    Some(match tidied.char_indices().nth(MAX_CHARS) {
        Some((end, _)) => format!("{}…", tidied[..end].trim_end()),
        None => tidied,
    })
}

pub fn preview(text: &str, max_chars: usize) -> Option<String> {
    let (end, _) = text.char_indices().nth(max_chars)?;
    let cut = &text[..end];
    let whole = match cut.rfind(char::is_whitespace) {
        Some(space) if space > 0 => &cut[..space],
        _ => cut,
    };

    Some(format!(
        "{}…",
        whole.trim_end_matches(|letter: char| !letter.is_alphanumeric())
    ))
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&hellip;", "…")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—")
        .replace("&amp;", "&")
}

fn is_footnote(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('[') && line.contains("]:")
}

fn without_links(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        plain.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find(']') else {
            plain.push_str(&rest[start..]);
            return plain;
        };
        let label = &after[..end];
        let tail = &after[end + 1..];
        let close = match tail.chars().next() {
            Some('(') => tail.find(')'),
            Some('[') => tail.find(']'),
            _ => None,
        };
        match close {
            Some(close) => {
                plain.push_str(label);
                rest = &tail[close + 1..];
            }
            None => {
                plain.push('[');
                plain.push_str(label);
                plain.push(']');
                rest = tail;
            }
        }
    }
    plain.push_str(rest);
    plain
}

#[cfg(test)]
mod tests {
    use super::{MAX_CHARS, from_html, from_markdown, preview, tidy};

    #[test]
    fn a_preview_ends_on_a_whole_word() {
        assert_eq!(
            preview("at the center, of it", 12).as_deref(),
            Some("at the…")
        );
        assert_eq!(preview("one, two three", 6).as_deref(), Some("one…"));
        assert_eq!(preview("short", 5), None);
        assert_eq!(preview("ééééééé", 3).as_deref(), Some("ééé…"));
    }

    #[test]
    fn html_becomes_plain_paragraphs() {
        assert_eq!(
            from_html("<p><b>Dark</b> &amp; strange.</p><p>Second<br/>line</p>").as_deref(),
            Some("Dark & strange.\n\nSecond\nline")
        );
    }

    #[test]
    fn an_escaped_entity_is_decoded_only_once() {
        assert_eq!(from_html("a &amp;lt; b").as_deref(), Some("a &lt; b"));
    }

    #[test]
    fn unfinished_html_does_not_panic() {
        assert_eq!(from_html("Text <b").as_deref(), Some("Text"));
        assert_eq!(from_html("ñ <é> ü").as_deref(), Some("ñ  ü"));
    }

    #[test]
    fn empty_text_is_no_text() {
        assert_eq!(from_html("<p> </p>"), None);
        assert_eq!(from_markdown("\r\n \r\n"), None);
        assert_eq!(tidy(""), None);
    }

    #[test]
    fn markdown_loses_its_marks_links_and_footnotes() {
        let markdown = "**From** the author of *Circe* ([source][1]).\r\n\r\nSee [the site](https://a.test).\r\n\r\n[1]: https://b.test\r\n----------\r\nAlso in: [x](y)";

        assert_eq!(
            from_markdown(markdown).as_deref(),
            Some("From the author of Circe .\n\nSee the site.")
        );
    }

    #[test]
    fn quotes_and_headings_lose_their_markers() {
        assert_eq!(
            from_markdown(
                "## Review

> > One of my favorites
> ever"
            )
            .as_deref(),
            Some(
                "Review

One of my favorites
ever"
            )
        );
    }

    #[test]
    fn brackets_that_are_not_links_stay() {
        assert_eq!(from_markdown("a [note] b").as_deref(), Some("a [note] b"));
        assert_eq!(from_markdown("a [note").as_deref(), Some("a [note"));
    }

    #[test]
    fn very_long_text_is_cut_on_a_character() {
        let text = "é".repeat(MAX_CHARS + 10);
        let tidied = tidy(&text).unwrap();

        assert_eq!(tidied.chars().count(), MAX_CHARS + 1);
        assert!(tidied.ends_with('…'));
    }
}
