pub static KNOWN_TAPS: &[(&str, &str)] = &[
    (
        "reliquaint-core",
        "https://github.com/syraenix/reliquaint-core",
    ),
];

/// Resolve a short name to its canonical URL.
///
/// HTTPS/HTTP URLs and paths starting with `/` or `.` pass through unchanged.
/// Unknown short names are returned unchanged (git will fail with a clear error).
pub fn resolve_tap_source(input: &str) -> &str {
    if input.starts_with("https://")
        || input.starts_with("http://")
        || input.starts_with('/')
        || input.starts_with('.')
    {
        return input;
    }
    KNOWN_TAPS
        .iter()
        .find(|(name, _)| *name == input)
        .map(|(_, url)| *url)
        .unwrap_or(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_short_name() {
        assert_eq!(
            resolve_tap_source("reliquaint-core"),
            "https://github.com/syraenix/reliquaint-core"
        );
    }

    #[test]
    fn passes_https_url_through() {
        let url = "https://example.com/my-tap";
        assert_eq!(resolve_tap_source(url), url);
    }

    #[test]
    fn passes_local_path_through() {
        assert_eq!(resolve_tap_source("./local-tap"), "./local-tap");
        assert_eq!(resolve_tap_source("/abs/path"), "/abs/path");
    }

    #[test]
    fn unknown_short_name_returned_as_is() {
        assert_eq!(resolve_tap_source("unknown-tap"), "unknown-tap");
    }
}
