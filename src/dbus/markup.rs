//! Notification bodies may contain a small HTML subset; shells show plain text.

/// Removes tags and decodes the entities the notification spec allows.
pub fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_entities() {
        assert_eq!(strip("<b>Hi</b> &amp; <i>bye</i>"), "Hi & bye");
        assert_eq!(strip("a &lt;b&gt; c"), "a <b> c");
        assert_eq!(strip("<a href=\"x\">link</a>"), "link");
        assert_eq!(strip("plain"), "plain");
    }
}
