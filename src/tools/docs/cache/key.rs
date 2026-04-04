//! Cache key generation and validation for document cache

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Normalize crate name to lowercase
///
/// Crate names are case-insensitive on crates.io, so we normalize
/// to lowercase for consistent cache key generation.
#[must_use]
fn normalize_crate_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn normalize_version(version: Option<&str>) -> Option<String> {
    version.map(|v| v.trim().to_lowercase())
}

fn is_valid_crate_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_valid_item_path_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':'
}

fn is_valid_crate_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(is_valid_crate_name_char)
}

fn is_valid_item_path(path: &str) -> bool {
    !path.is_empty() && path.bytes().all(is_valid_item_path_char)
}

/// Cache key generator for document cache
pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// Build a raw crate HTML cache key with normalization.
    ///
    /// This key stores the fetched docs.rs HTML artifact shared across
    /// markdown, text, and html responses for the same crate lookup.
    ///
    /// Key format: `crate:{name}:html` or `crate:{name}:{version}:html`
    #[must_use]
    pub fn crate_html_cache_key(crate_name: &str, version: Option<&str>) -> String {
        let base_key = Self::crate_cache_key(crate_name, version);
        format!("{base_key}:html")
    }

    /// Build crate cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase (via `normalize_crate_name`)
    /// - `version`: lowercase, trimmed
    /// - Invalid characters in `crate_name` (non-alphanumeric, non-underscore, non-hyphen)
    ///   will result in a hashed key to prevent injection
    #[must_use]
    pub fn crate_cache_key(crate_name: &str, version: Option<&str>) -> String {
        let normalized_name = normalize_crate_name(crate_name);

        if !is_valid_crate_name(&normalized_name) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalize_version(version) {
                Some(normalized_ver) => format!("crate:hash:{hash}:{normalized_ver}"),
                None => format!("crate:hash:{hash}"),
            };
        }

        match normalize_version(version) {
            Some(normalized_ver) => format!("crate:{normalized_name}:{normalized_ver}"),
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
        let normalized_query = query.trim().to_lowercase();
        let normalized_sort = sort.unwrap_or("relevance").trim().to_lowercase();
        format!("search:{normalized_query}:{normalized_sort}:{limit}")
    }

    /// Build item cache key with normalization
    ///
    /// # Normalization rules
    ///
    /// - `crate_name`: lowercase (via `normalize_crate_name`)
    /// - `item_path`: trimmed but case-sensitive (Rust paths are case-sensitive)
    /// - `version`: lowercase, trimmed
    #[must_use]
    pub fn item_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        let normalized_name = normalize_crate_name(crate_name);
        let normalized_path = item_path.trim();

        if !is_valid_crate_name(&normalized_name) || !is_valid_item_path(normalized_path) {
            let mut hasher = DefaultHasher::new();
            normalized_name.hash(&mut hasher);
            normalized_path.hash(&mut hasher);
            let hash = hasher.finish();
            return match normalize_version(version) {
                Some(normalized_ver) => {
                    format!("item:{normalized_name}:{normalized_ver}:hash:{hash}")
                }
                None => format!("item:{normalized_name}:hash:{hash}"),
            };
        }

        match normalize_version(version) {
            Some(normalized_ver) => {
                format!("item:{normalized_name}:{normalized_ver}:{normalized_path}")
            }
            None => format!("item:{normalized_name}:{normalized_path}"),
        }
    }

    /// Build a raw item HTML cache key with normalization.
    ///
    /// This key stores the fetched docs.rs search-result HTML artifact shared
    /// across markdown, text, and html responses for the same item lookup.
    ///
    /// Key format: `item:{crate}:{path}:html` or `item:{crate}:{version}:{path}:html`
    #[must_use]
    pub fn item_html_cache_key(crate_name: &str, item_path: &str, version: Option<&str>) -> String {
        let base_key = Self::item_cache_key(crate_name, item_path, version);
        format!("{base_key}:html")
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
            "crate:serde:1.0:html"
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
            "item:serde:1.0:Serialize:html"
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

        let malicious_item_colon =
            CacheKeyGenerator::item_cache_key("serde", "Serialize:extra:colons", None);
        assert_eq!(malicious_item_colon, "item:serde:Serialize:extra:colons");

        let valid_item_path = CacheKeyGenerator::item_cache_key("serde", "serde::Serialize", None);
        assert_eq!(valid_item_path, "item:serde:serde::Serialize");

        let empty_item_key = CacheKeyGenerator::item_cache_key("serde", "", None);
        assert!(empty_item_key.contains("hash:"));

        let empty_item_crate = CacheKeyGenerator::item_cache_key("", "Crate", None);
        assert!(empty_item_crate.contains("hash:"));
    }
}
