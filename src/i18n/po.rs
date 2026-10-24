//! A parser for gettext `.po` files.

/// One translatable message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub context: Option<String>,
    pub id: String,
    pub plural_id: Option<String>,
    /// One string, or one per plural form.
    pub translations: Vec<String>,
    pub fuzzy: bool,
}

impl Message {
    /// Untranslated, fuzzy or obsolete messages must not be used.
    pub fn is_usable(&self) -> bool {
        !self.fuzzy && self.translations.iter().any(|t| !t.is_empty())
    }
}

fn unescape(quoted: &str) -> Option<String> {
    let inner = quoted.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    Some(out)
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    None,
    Context,
    Id,
    PluralId,
    Translation(usize),
}

/// Parses a `.po` file. Malformed lines are skipped rather than failing the whole catalog.
pub fn parse_po(source: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut current = Message::default();
    let mut field = Field::None;
    let mut started = false;

    let mut finish = |current: &mut Message, started: &mut bool| {
        if *started {
            messages.push(std::mem::take(current));
        }
        *started = false;
    };

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#~") {
            // Obsolete entry.
            field = Field::None;
            continue;
        }
        if let Some(flags) = line.strip_prefix("#,") {
            if field != Field::None || started {
                finish(&mut current, &mut started);
                field = Field::None;
            }
            current.fuzzy |= flags.split(',').any(|f| f.trim() == "fuzzy");
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let (keyword, rest) = match line.find(|c: char| c.is_whitespace()) {
            Some(i) if !line.starts_with('"') => (&line[..i], &line[i..]),
            _ => ("", line),
        };
        let next = match keyword {
            "msgctxt" => Field::Context,
            "msgid" => Field::Id,
            "msgid_plural" => Field::PluralId,
            "msgstr" => Field::Translation(0),
            k if k.starts_with("msgstr[") => {
                match k.trim_start_matches("msgstr[").trim_end_matches(']').parse() {
                    Ok(n) => Field::Translation(n),
                    Err(_) => continue,
                }
            }
            "" => field,
            _ => continue,
        };
        // A new msgctxt, or a msgid not preceded by msgctxt, starts a new message.
        let starts_message = (next == Field::Context)
            || (next == Field::Id && keyword == "msgid" && field != Field::Context);
        if starts_message && started && !keyword.is_empty() {
            finish(&mut current, &mut started);
        }
        let Some(text) = unescape(rest) else { continue };
        started = true;
        field = next;
        match field {
            Field::Context => current.context.get_or_insert_with(String::new).push_str(&text),
            Field::Id => current.id.push_str(&text),
            Field::PluralId => current.plural_id.get_or_insert_with(String::new).push_str(&text),
            Field::Translation(n) => {
                if current.translations.len() <= n {
                    current.translations.resize(n + 1, String::new());
                }
                current.translations[n].push_str(&text);
            }
            Field::None => {}
        }
    }
    finish(&mut current, &mut started);
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    const PO: &str = r#"
# Translator comment
msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"

#: bar.slint:12
msgctxt "Bar"
msgid "Volume"
msgstr "Lautstärke"

msgctxt "Bar"
msgid "{n} window"
msgid_plural "{n} windows"
msgstr[0] "{n} Fenster"
msgstr[1] "{n} Fenster"

#, fuzzy
msgid "Maybe"
msgstr "Vielleicht"

msgid "Multi"
"line"
msgstr "Mehr"
"zeilig \"quoted\"\n"

#~ msgid "Old"
#~ msgstr "Alt"
"#;

    #[test]
    fn parses_contexts_plurals_and_continuations() {
        let messages = parse_po(PO);
        assert_eq!(messages.len(), 5, "{messages:#?}");
        assert!(messages[0].id.is_empty());
        assert!(messages[0].translations[0].contains("Plural-Forms"));

        assert_eq!(messages[1].context.as_deref(), Some("Bar"));
        assert_eq!(messages[1].translations, vec!["Lautstärke"]);

        assert_eq!(messages[2].plural_id.as_deref(), Some("{n} windows"));
        assert_eq!(messages[2].translations.len(), 2);

        assert!(messages[3].fuzzy);
        assert!(!messages[3].is_usable());

        assert_eq!(messages[4].id, "Multiline");
        assert_eq!(messages[4].translations[0], "Mehrzeilig \"quoted\"\n");
    }
}
