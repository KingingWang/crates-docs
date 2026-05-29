//! Cache key generation and validation for document cache

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Check if a byte is a valid crate name character
#[inline]
fn is_valid_crate_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Check if a byte is a valid item path character
#[inline]
fn is_valid_item_path_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':'
}

/// Check if crate name is valid (non-empty and all valid chars)
#[inline]
fn is_valid_crate_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_valid_crate_name_char)
}

/// Check if item path is valid (non-empty, valid chars, colons only as `::`)
///
/// Rust paths separate segments with `::` (a double colon). A lone `:` is not a
/// valid path separator, and allowing it would make the item cache key
/// ambiguous: `item:a:1.0:Serialize` could mean either path `"1.0:Serialize"`
/// (no version) or version `"1.0"` + path `"Serialize"`. Rejecting single
/// colons routes such inputs through the hashed-key branch, keeping keys
/// injective and matching the tool-level `validate_item_path` guard.
#[inline]
fn is_valid_item_path(path: &str) -> bool {
    !path.is_empty()
        && path.bytes().all(is_valid_item_path_char)
        && !path.replace("::", "").contains(':')
}

/// Reversibly escape `:` and `%` in a free-form cache-key segment.
///
/// Cache keys are colon-delimited. Free-form segments such as the search query
/// or sort string may themselves contain `:`, which would otherwise let two
/// distinct (query, sort, limit) inputs map to the same key (for example
/// `query="a", sort="b:c"` and `query="a:b", sort="c"` both yield
/// `search:a:b:c:{limit}`). Percent-encoding `%` first and then `:` keeps the
/// mapping injective while leaving colon-free inputs unchanged.
#[inline]
fn escape_key_segment(segment: &str) -> String {
    if segment.contains('%') || segment.contains(':') {
        segment.replace('%', "%25").replace(':', "%3a")
    } else {
        segment.to_string()
    }
}

/// Cache key generator for document cache
pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// Build a raw crate HTML cache key with normalization.
    ///
    /// This key stores the fetched docs.rs HTML artifact shared across
    /// markdown, text, and html responses for the same crate lookup.
    ///
    /// Key format: `htmlraw:crate:{name}` or `htmlraw:crate:{name}:{version}`
    ///
    /// The `htmlraw:` namespace prefix keeps raw HTML artifacts in a separate
    /// keyspace from rendered documentation keys (`crate:...`). Without it, a
    /// rendered lookup for version literal `"html"` (e.g.
    /// `crate_cache_key("serde", Some("html"))` => `crate:serde:html`) would
    /// collide with the HTML artifact key for `crate_html_cache_key("serde",
    /// None)`, cross-contaminating rendered text and raw HTML.
    #[must_use]
    pub fn crate_html_cache_key(crate_name: &str, version: Option<&str>) -> String {
        let base_key = Self::crate_cache_key(crate_name, version);
        format!("htmlraw:{base_key}")
    }

    /// Build crate cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase, trimmed
    ///   (crate names are case-insensitive on crates.io)
    /// - `version`: lowercase, trimmed
    /// - Invalid characters in `crate_name` (non-alphanumeric, non-underscore, non-hyphen)
    ///   will result in a hashed key to prevent injection
    #[must_use]
    pub fn crate_cache_key(crate_name: &str, version: Option<&str>) -> String {
        // Inline normalization to avoid intermediate allocations
        let normalized_name = crate_name.trim().to_lowercase();
        let normalized_ver = version.map(|v| v.trim().to_lowercase());

        if !is_valid_crate_name(&normalized_name) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalized_ver {
                Some(ver) => format!("crate:hash:{hash}:{ver}"),
                None => format!("crate:hash:{hash}"),
            };
        }

        match normalized_ver {
            Some(ver) => format!("crate:{normalized_name}:{ver}"),
            None => format!("crate:{normalized_name}"),
        }
    }

    /// Build search cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - query: lowercase, trimmed (search is case-insensitive)
    /// - sort: lowercase, trimmed
    #[must_use]
    pub fn search_cache_key(query: &str, limit: u32, sort: Option<&str>) -> String {
        let normalized_query = escape_key_segment(&query.trim().to_lowercase());
        let normalized_sort =
            escape_key_segment(&sort.unwrap_or("relevance").trim().to_lowercase());
        format!("search:{normalized_query}:{normalized_sort}:{limit}")
    }

    /// Build item cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase, trimmed
    ///   (crate names are case-insensitive on crates.io)
    /// - `item_path`: trimmed but case-sensitive (Rust paths are case-sensitive)
    /// - `version`: lowercase, trimmed
    #[must_use]
    pub fn item_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        let normalized_name = crate_name.trim().to_lowercase();
        let normalized_path = item_path.trim();
        let normalized_ver = version.map(|v| v.trim().to_lowercase());

        if !is_valid_crate_name(&normalized_name) || !is_valid_item_path(normalized_path) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            normalized_path.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalized_ver {
                Some(ver) => {
                    format!("item:{normalized_name}:{ver}:hash:{hash}")
                }
                None => format!("item:{normalized_name}:hash:{hash}"),
            };
        }

        match normalized_ver {
            Some(ver) => {
                format!("item:{normalized_name}:{ver}:{normalized_path}")
            }
            None => format!("item:{normalized_name}:{normalized_path}"),
        }
    }

    /// Build a raw item HTML cache key with normalization.
    ///
    /// This key stores the fetched docs.rs search-result HTML artifact shared
    /// across markdown, text, and html responses for the same item lookup.
    ///
    /// Key format: `htmlraw:item:{crate}:{path}` (see [`Self::crate_html_cache_key`]
    /// for why the `htmlraw:` namespace is used to avoid collisions).
    #[must_use]
    pub fn item_html_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        let base_key = Self::item_cache_key(crate_name, item_path, version);
        format!("htmlraw:{base_key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("serde", None),
            "crate:serde"
        );
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("serde", Some("1.0")),
            "crate:serde:1.0"
        );
        assert_eq!(
            CacheKeyGenerator::crate_html_cache_key("serde", Some("1.0")),
            "htmlraw:crate:serde:1.0"
        );

        assert_eq!(
            CacheKeyGenerator::search_cache_key("web framework", 10, None),
            "search:web framework:relevance:10"
        );
        assert_eq!(
            CacheKeyGenerator::search_cache_key("web framework", 10, Some("downloads")),
            "search:web framework:downloads:10"
        );

        assert_eq!(
            CacheKeyGenerator::item_cache_key("serde", "Serialize", None),
            "item:serde:Serialize"
        );
        assert_eq!(
            CacheKeyGenerator::item_cache_key("serde", "Serialize", Some("1.0")),
            "item:serde:1.0:Serialize"
        );
        assert_eq!(
            CacheKeyGenerator::item_html_cache_key("serde", "Serialize", Some("1.0")),
            "htmlraw:item:serde:1.0:Serialize"
        );
    }

    #[test]
    fn test_cache_key_normalization_case_insensitivity() {
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("Serde", None),
            CacheKeyGenerator::crate_cache_key("serde", None)
        );
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("SERDE", None),
            CacheKeyGenerator::crate_cache_key("serde", None)
        );

        assert_eq!(
            CacheKeyGenerator::crate_cache_key("Tokio", Some("1.0")),
            CacheKeyGenerator::crate_cache_key("tokio", Some("1.0"))
        );

        assert_eq!(
            CacheKeyGenerator::search_cache_key("Web Framework", 10, Some("Relevance")),
            CacheKeyGenerator::search_cache_key("web framework", 10, Some("relevance"))
        );

        assert_eq!(
            CacheKeyGenerator::item_cache_key("Serde", "Serialize", None),
            CacheKeyGenerator::item_cache_key("serde", "Serialize", None)
        );
    }

    #[test]
    fn test_cache_key_normalization_whitespace() {
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("serde", Some(" 1.0 ")),
            "crate:serde:1.0"
        );

        assert_eq!(
            CacheKeyGenerator::search_cache_key("  web framework  ", 10, Some(" downloads ")),
            "search:web framework:downloads:10"
        );

        assert_eq!(
            CacheKeyGenerator::item_cache_key("serde", "  Serialize  ", None),
            "item:serde:Serialize"
        );
    }

    #[test]
    fn test_html_artifact_keys_do_not_collide_with_rendered_keys() {
        // A rendered lookup for the (pathological) version literal "html" must
        // not collide with the raw HTML artifact keyspace.
        let rendered = CacheKeyGenerator::crate_cache_key("serde", Some("html"));
        let artifact = CacheKeyGenerator::crate_html_cache_key("serde", None);
        assert_ne!(rendered, artifact);
        assert_eq!(rendered, "crate:serde:html");
        assert_eq!(artifact, "htmlraw:crate:serde");

        let rendered_item = CacheKeyGenerator::item_cache_key("serde", "Serialize", Some("html"));
        let artifact_item = CacheKeyGenerator::item_html_cache_key("serde", "Serialize", None);
        assert_ne!(rendered_item, artifact_item);
    }

    #[test]
    fn test_cache_key_normalization_version_case() {
        assert_eq!(
            CacheKeyGenerator::crate_cache_key("serde", Some("1.0-RC1")),
            "crate:serde:1.0-rc1"
        );
        assert_eq!(
            CacheKeyGenerator::item_cache_key("serde", "Serialize", Some("V1.0")),
            "item:serde:v1.0:Serialize"
        );
    }

    #[test]
    fn test_cache_key_injection_prevention() {
        let malicious_key = CacheKeyGenerator::crate_cache_key("serde:malicious", None);
        assert!(malicious_key.starts_with("crate:hash:"));
        assert!(!malicious_key.contains("serde:malicious"));

        let malicious_key_with_version =
            CacheKeyGenerator::crate_cache_key("crate:evil", Some("1.0"));
        assert!(malicious_key_with_version.starts_with("crate:hash:"));
        assert!(!malicious_key_with_version.contains("crate:evil"));

        let valid_key = CacheKeyGenerator::crate_cache_key("serde-json", None);
        assert_eq!(valid_key, "crate:serde-json");

        let valid_key_underscore = CacheKeyGenerator::crate_cache_key("my_crate", None);
        assert_eq!(valid_key_underscore, "crate:my_crate");
    }

    #[test]
    fn test_search_cache_key_no_colon_collision() {
        // Two distinct (query, sort) inputs that previously collapsed to the
        // same colon-delimited key must now produce distinct keys.
        let a = CacheKeyGenerator::search_cache_key("a", 5, Some("b:c"));
        let b = CacheKeyGenerator::search_cache_key("a:b", 5, Some("c"));
        assert_ne!(a, b);

        // Colon-free inputs stay human-readable and unescaped.
        assert_eq!(
            CacheKeyGenerator::search_cache_key("web framework", 10, Some("downloads")),
            "search:web framework:downloads:10"
        );

        // A literal percent in the query must not be confused with an escape.
        let pct = CacheKeyGenerator::search_cache_key("100%", 10, None);
        let escaped = CacheKeyGenerator::search_cache_key("100%3a", 10, None);
        assert_ne!(pct, escaped);
    }

    #[test]
    fn test_item_cache_key_no_version_path_collision() {
        // path "1.0:Serialize" (no version) must NOT collide with
        // version "1.0" + path "Serialize".
        let a = CacheKeyGenerator::item_cache_key("serde", "1.0:Serialize", None);
        let b = CacheKeyGenerator::item_cache_key("serde", "Serialize", Some("1.0"));
        assert_ne!(a, b);
        // Legitimate `::` paths stay readable/unhashed.
        assert_eq!(
            CacheKeyGenerator::item_cache_key("serde", "de::Deserialize", None),
            "item:serde:de::Deserialize"
        );
    }

    #[test]
    fn test_item_path_case_sensitivity() {
        assert_ne!(
            CacheKeyGenerator::item_cache_key("serde", "Serialize", None),
            CacheKeyGenerator::item_cache_key("serde", "serialize", None)
        );
    }

    #[test]
    fn test_cache_key_edge_cases() {
        let empty_key = CacheKeyGenerator::crate_cache_key("", None);
        assert!(empty_key.starts_with("crate:hash:"));

        let whitespace_key = CacheKeyGenerator::crate_cache_key("   ", None);
        assert!(whitespace_key.starts_with("crate:hash:"));

        assert_eq!(
            CacheKeyGenerator::crate_cache_key("serde", Some("")),
            "crate:serde:"
        );

        let unicode_key = CacheKeyGenerator::crate_cache_key("serde测试", None);
        assert!(unicode_key.starts_with("crate:hash:"));
        assert!(!unicode_key.contains("测试"));

        let malicious_item_path =
            CacheKeyGenerator::item_cache_key("serde", "Serialize\nmalicious", None);
        assert!(malicious_item_path.contains("hash:"));
        assert!(!malicious_item_path.contains('\n'));

        // Single (non-`::`) colons are ambiguous separators and must be hashed,
        // not embedded verbatim, to avoid version/path key collisions.
        let malicious_item_colon =
            CacheKeyGenerator::item_cache_key("serde", "Serialize:extra:colons", None);
        assert!(malicious_item_colon.starts_with("item:serde:hash:"));
        assert!(!malicious_item_colon.contains("Serialize:extra:colons"));

        let valid_item_path = CacheKeyGenerator::item_cache_key("serde", "serde::Serialize", None);
        assert_eq!(valid_item_path, "item:serde:serde::Serialize");

        let empty_item_key = CacheKeyGenerator::item_cache_key("serde", "", None);
        assert!(empty_item_key.contains("hash:"));

        let empty_item_crate = CacheKeyGenerator::item_cache_key("", "Crate", None);
        assert!(empty_item_crate.contains("hash:"));
    }
}
