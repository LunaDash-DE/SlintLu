//! `file://` URIs, as used by MPRIS, portals and compositors for local files.

use std::path::PathBuf;

/// The local path of a `file://` URI (percent-decoded), or `None` for other strings.
pub fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // "file://host/path": only the local host is meaningful.
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(slash) if &rest[..slash] == "localhost" => &rest[slash..],
        _ => return None,
    };
    Some(PathBuf::from(percent_decode(path)))
}

/// A path from either a plain path or a `file://` URI.
pub fn path_from_path_or_uri(text: &str) -> PathBuf {
    file_uri_path(text).unwrap_or_else(|| PathBuf::from(text))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|d| d as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            file_uri_path("file:///home/me/My%20Pictures/%E6%9C%88.png"),
            Some(PathBuf::from("/home/me/My Pictures/月.png"))
        );
        assert_eq!(file_uri_path("file://localhost/tmp/a"), Some(PathBuf::from("/tmp/a")));
        assert_eq!(file_uri_path("file://elsewhere/tmp/a"), None);
        assert_eq!(file_uri_path("/tmp/a"), None);
        assert_eq!(file_uri_path("file:///bad%2"), Some(PathBuf::from("/bad%2")));
        assert_eq!(path_from_path_or_uri("/tmp/b"), PathBuf::from("/tmp/b"));
    }
}
