//! Writing gettext `.mo` files.

use super::po::Message;

const MAGIC: u32 = 0x9504_12de;
const HEADER_SIZE: usize = 28;

fn key(message: &Message) -> Vec<u8> {
    let mut key = Vec::new();
    if let Some(context) = &message.context {
        key.extend_from_slice(context.as_bytes());
        key.push(0x04);
    }
    key.extend_from_slice(message.id.as_bytes());
    if let Some(plural) = &message.plural_id {
        key.push(0);
        key.extend_from_slice(plural.as_bytes());
    }
    key
}

fn value(message: &Message) -> Vec<u8> {
    message.translations.join("\0").into_bytes()
}

/// Encodes usable messages (plus the header entry) as a little-endian `.mo` file.
pub fn write_mo(messages: &[Message]) -> Vec<u8> {
    let mut entries: Vec<(Vec<u8>, Vec<u8>)> =
        messages.iter().filter(|m| m.is_usable()).map(|m| (key(m), value(m))).collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|a, b| a.0 == b.0);

    let count = entries.len();
    let originals_offset = HEADER_SIZE;
    let translations_offset = originals_offset + count * 8;
    let mut data_offset = translations_offset + count * 8;

    let mut out = Vec::new();
    for word in [MAGIC, 0, count as u32, originals_offset as u32, translations_offset as u32, 0, 0]
    {
        out.extend_from_slice(&word.to_le_bytes());
    }
    let mut data = Vec::new();
    let mut table = |bytes: &[u8], out: &mut Vec<u8>| {
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data_offset as u32).to_le_bytes());
        data.extend_from_slice(bytes);
        data.push(0);
        data_offset += bytes.len() + 1;
    };
    for (k, _) in &entries {
        table(k, &mut out);
    }
    for (_, v) in &entries {
        table(v, &mut out);
    }
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn string(bytes: &[u8], table: usize, index: usize) -> &[u8] {
        let len = read_u32(bytes, table + index * 8) as usize;
        let offset = read_u32(bytes, table + index * 8 + 4) as usize;
        &bytes[offset..offset + len]
    }

    #[test]
    fn writes_sorted_tables_with_contexts() {
        let messages = vec![
            Message { id: "b".into(), translations: vec!["B".into()], ..Default::default() },
            Message {
                context: Some("Bar".into()),
                id: "a".into(),
                translations: vec!["A".into()],
                ..Default::default()
            },
            Message { id: "skip".into(), translations: vec![String::new()], ..Default::default() },
        ];
        let mo = write_mo(&messages);
        assert_eq!(read_u32(&mo, 0), MAGIC);
        assert_eq!(read_u32(&mo, 8), 2);
        let (originals, translations) = (read_u32(&mo, 12) as usize, read_u32(&mo, 16) as usize);
        assert_eq!(string(&mo, originals, 0), b"Bar\x04a");
        assert_eq!(string(&mo, translations, 0), b"A");
        assert_eq!(string(&mo, originals, 1), b"b");
        assert_eq!(string(&mo, translations, 1), b"B");
    }

    #[test]
    fn plural_forms_are_nul_separated() {
        let message = Message {
            id: "file".into(),
            plural_id: Some("files".into()),
            translations: vec!["Datei".into(), "Dateien".into()],
            ..Default::default()
        };
        assert_eq!(key(&message), b"file\0files");
        assert_eq!(value(&message), b"Datei\0Dateien");
    }
}
