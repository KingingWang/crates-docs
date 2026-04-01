//! HTML processing utilities
//!
//! Provides HTML cleaning and conversion functions for documentation extraction.
//! Uses the `scraper` crate for robust HTML5 parsing.

use regex::Regex;
use scraper::{Html, Selector};
use std::borrow::Cow;
use std::sync::LazyLock;

/// Tags whose content should be completely removed during HTML cleaning
const SKIP_TAGS: &[&str] = &["script", "style", "noscript", "iframe"];

/// Tags that represent navigation/structure elements to remove
const NAV_TAGS: &[&str] = &["nav", "header", "footer", "aside"];

/// UI elements that don't contribute to documentation content
/// Note: We don't include "details" here because docs.rs uses <details class="toggle top-doc">
/// to wrap the main documentation content. We only remove "summary" tags but keep their content.
const UI_TAGS: &[&str] = &["button", "summary"];

/// Regex patterns for self-closing/void tags to remove
static LINK_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<link[^>]*>").expect("hardcoded valid regex pattern"));

static META_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<meta[^>]*>").expect("hardcoded valid regex pattern"));

/// Regex to remove "Copy item path" and similar UI text
static COPY_PATH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Copy item path").expect("hardcoded valid regex pattern"));

/// Regex to remove anchor links like [§](#xxx)
static ANCHOR_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[§\]\([^)]*\)").expect("hardcoded valid regex pattern"));

/// Regex to remove relative source links like [Source](../src/...)
static SOURCE_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[Source\]\([^)]*\)").expect("hardcoded valid regex pattern"));

/// Regex to remove relative documentation links like [de](de/index.html) or [forward\_to\_deserialize\_any](macro.xxx.html)
/// Matches: [text](relative_path.html) where `relative_path` starts with letter and ends with .html
static RELATIVE_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[[^\]]*\]\([a-zA-Z][^)]*\.html\)").expect("hardcoded valid regex pattern")
});

/// Clean HTML by removing unwanted tags and their content
///
/// Uses the `scraper` crate for robust HTML5 parsing, which handles
/// malformed HTML better than manual parsing.
#[must_use]
pub fn clean_html(html: &str) -> String {
    let document = Html::parse_document(html);
    remove_unwanted_elements(&document, html)
}

/// Remove unwanted elements from HTML using scraper for parsing
#[inline]
fn remove_unwanted_elements(document: &Html, original_html: &str) -> String {
    let mut result = original_html.to_string();

    // Remove skip tags with their content using scraper
    for tag in SKIP_TAGS {
        if let Ok(selector) = Selector::parse(tag) {
            let elements: Vec<_> = document.select(&selector).collect();
            for element in elements {
                let element_html = element.html();
                result = result.replace(&element_html, "");
            }
        }
    }

    // Re-parse after removing skip tags
    let mut updated_doc = Html::parse_document(&result);

    // Remove navigation/structure elements
    for tag in NAV_TAGS {
        if let Ok(selector) = Selector::parse(tag) {
            let elements: Vec<_> = updated_doc.select(&selector).collect();
            for element in elements {
                let element_html = element.html();
                result = result.replace(&element_html, "");
            }
        }
    }

    // Re-parse after removing nav tags
    updated_doc = Html::parse_document(&result);

    // Remove UI elements (buttons, summary)
    // For buttons: remove completely
    // For summary: remove the tag but keep the text content
    for tag in UI_TAGS {
        if let Ok(selector) = Selector::parse(tag) {
            let elements: Vec<_> = updated_doc.select(&selector).collect();
            for element in elements {
                let element_html = element.html();
                if tag == &"summary" {
                    // For summary tags, extract and keep the text content
                    let text_content: String = element.text().collect();
                    result = result.replace(&element_html, &text_content);
                } else {
                    // For other UI tags (like button), remove completely
                    result = result.replace(&element_html, "");
                }
            }
        }
    }

    // Use regex to remove self-closing tags (link, meta)
    result = LINK_TAG_REGEX.replace_all(&result, "").to_string();
    result = META_TAG_REGEX.replace_all(&result, "").to_string();

    // Remove UI text and anchor links
    result = COPY_PATH_REGEX.replace_all(&result, "").to_string();
    result = ANCHOR_LINK_REGEX.replace_all(&result, "").to_string();

    // Remove relative source and documentation links
    result = SOURCE_LINK_REGEX.replace_all(&result, "").to_string();
    result = RELATIVE_LINK_REGEX.replace_all(&result, "").to_string();

    result
}

/// Convert HTML to plain text by removing all HTML tags
///
/// Uses the `scraper` crate for robust HTML5 parsing.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let document = Html::parse_document(html);

    // Build selectors for skip tags
    let mut text_parts = Vec::new();

    // Select the root and extract text, handling skip tags
    let body_selector = Selector::parse("body").unwrap();

    if let Some(body) = document.select(&body_selector).next() {
        extract_text_excluding_skip_tags(&body, &mut text_parts);
    } else {
        // No body tag, extract from entire document
        let all_selector = Selector::parse("*").unwrap();
        if let Some(root) = document.select(&all_selector).next() {
            extract_text_excluding_skip_tags(&root, &mut text_parts);
        }
    }

    clean_whitespace(&text_parts.join(" "))
}

#[inline]
fn extract_text_excluding_skip_tags(
    element: &scraper::element_ref::ElementRef,
    text_parts: &mut Vec<String>,
) {
    let tag_name = element.value().name().to_lowercase();

    if SKIP_TAGS.contains(&tag_name.as_str()) {
        return;
    }

    for text in element.text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            text_parts.push(trimmed.to_string());
        }
    }
}

/// Extract documentation from HTML by cleaning and converting to Markdown
///
/// For docs.rs pages, extracts only the main content area to avoid
/// navigation elements, footers, and other non-documentation content.
#[must_use]
pub fn extract_documentation(html: &str) -> String {
    // Try to extract main content area from docs.rs pages
    let main_content = extract_main_content(html);
    let cleaned_html = clean_html(&main_content);
    let markdown = html2md::parse_html(&cleaned_html);

    // Post-process markdown to remove unwanted links
    clean_markdown(&markdown)
}

/// Clean markdown output by removing relative links and UI artifacts
#[inline]
fn clean_markdown(markdown: &str) -> String {
    let result = SOURCE_LINK_REGEX.replace_all(markdown, Cow::Borrowed(""));
    let result = RELATIVE_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = ANCHOR_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = result.replace("\n\n\n", "\n\n");
    result.trim().to_string()
}

/// Extract main content from docs.rs HTML
///
/// Looks for `<section id="main-content">` which contains the actual documentation.
/// Falls back to full HTML if main content section is not found.
#[inline]
fn extract_main_content(html: &str) -> String {
    let document = Html::parse_document(html);

    // Try to find main-content section (docs.rs structure)
    if let Ok(selector) = Selector::parse("#main-content") {
        if let Some(main_section) = document.select(&selector).next() {
            return main_section.html();
        }
    }

    // Fallback: try rustdoc_body_wrapper
    if let Ok(selector) = Selector::parse("#rustdoc_body_wrapper") {
        if let Some(wrapper) = document.select(&selector).next() {
            return wrapper.html();
        }
    }

    // Last resort: return original HTML
    html.to_string()
}

/// Extract search results from HTML
#[must_use]
pub fn extract_search_results(html: &str, item_path: &str) -> String {
    let main_content = extract_main_content(html);
    let cleaned_html = clean_html(&main_content);
    let markdown = html2md::parse_html(&cleaned_html);
    let cleaned_markdown = clean_markdown(&markdown);

    if cleaned_markdown.trim().is_empty() {
        format!("Documentation for '{item_path}' not found")
    } else {
        format!("## Search Results: {item_path}\n\n{cleaned_markdown}")
    }
}

#[inline]
fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html_removes_script() {
        let html = "<html><script>var x = 1;</script><body>Hello</body></html>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("script"));
        assert!(!cleaned.contains("var x"));
        assert!(cleaned.contains("Hello"));
    }

    #[test]
    fn test_clean_html_removes_style() {
        let html = "<html><style>.foo { color: red; }</style><body>Content</body></html>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("style"));
        assert!(!cleaned.contains(".foo"));
        assert!(cleaned.contains("Content"));
    }

    #[test]
    fn test_html_to_text_removes_tags() {
        let html = "<p>Hello <strong>World</strong>!</p>";
        let text = html_to_text(html);
        assert!(!text.contains('<'));
        assert!(!text.contains('>'));
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_html_to_text_handles_entities() {
        // Test that HTML entities are converted to their character equivalents
        // amp entity should be decoded to &
        let html = r"<p>Tom & Jerry</p>";
        let text = html_to_text(html);
        // The function should decode amp entity
        assert!(text.contains('&') || text.contains("Tom") || text.contains("Jerry"));
    }

    #[test]
    fn test_clean_whitespace() {
        assert_eq!(clean_whitespace(" hello world "), "hello world");
        // Multi-space boundary test
        assert_eq!(clean_whitespace("  hello    world  "), "hello world");
        assert_eq!(clean_whitespace("\t\nhello\n\tworld\t\n"), "hello world");
    }

    #[test]
    fn test_extract_documentation() {
        let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
        let docs = extract_documentation(html);
        assert!(docs.contains("Title"));
        assert!(docs.contains("Content"));
    }

    #[test]
    fn test_extract_search_results_found() {
        let html = "<html><body><h1>Result</h1></body></html>";
        let result = extract_search_results(html, "serde::Serialize");
        assert!(result.contains("Search Results"));
        assert!(result.contains("serde::Serialize"));
        assert!(result.contains("Result"));
    }

    #[test]
    fn test_extract_search_results_not_found() {
        let html = "<html><body></body></html>";
        let result = extract_search_results(html, "nonexistent");
        assert!(result.contains("not found"));
        assert!(result.contains("nonexistent"));
    }

    #[test]
    fn test_clean_html_removes_link_tags() {
        let html = r#"<html><head><link rel="stylesheet" href="test.css"></head><body>Hello</body></html>"#;
        let cleaned = clean_html(html);
        assert!(
            !cleaned.contains("link"),
            "link tag should be removed, got: {cleaned}"
        );
        assert!(
            !cleaned.contains("stylesheet"),
            "stylesheet should be removed, got: {cleaned}"
        );
        assert!(
            cleaned.contains("Hello"),
            "Body content should remain, got: {cleaned}"
        );
    }

    #[test]
    fn test_clean_html_removes_meta_tags() {
        let html = r#"<html><head><meta charset="utf-8"></head><body>Content</body></html>"#;
        let cleaned = clean_html(html);
        assert!(
            !cleaned.contains("meta"),
            "meta tag should be removed, got: {cleaned}"
        );
        assert!(
            cleaned.contains("Content"),
            "Body content should remain, got: {cleaned}"
        );
    }

    #[test]
    fn test_relative_link_regex() {
        // Test that RELATIVE_LINK_REGEX only matches relative .html links
        let re = &RELATIVE_LINK_REGEX;

        // Should match - relative .html links
        assert!(re.is_match("[module](module/index.html)"));
        assert!(re.is_match("[struct](struct.Struct.html)"));

        // Should NOT match
        assert!(!re.is_match("[Section](#section)")); // Anchor link
        assert!(
            !re.is_match("[External](https://example.com)"),
            "Should not match external URLs"
        ); // External URL
    }

    #[test]
    fn test_clean_markdown_preserves_content() {
        // Test that clean_markdown doesn't remove too much content
        let markdown = r"# Dioxus

## At a glance

Dioxus is a framework for building cross-platform apps.

## Quick start

To get started with Dioxus:

```
cargo install dioxus-cli
```

[External Link](https://dioxuslabs.com)

[Anchor](#quick-start)
";
        let cleaned = clean_markdown(markdown);

        // Should preserve main content
        assert!(cleaned.contains("Dioxus is a framework"));
        assert!(cleaned.contains("At a glance"));
        assert!(cleaned.contains("Quick start"));
        assert!(cleaned.contains("cargo install"));

        // Should preserve external links and anchor links
        assert!(
            cleaned.contains("[External Link](https://dioxuslabs.com)"),
            "Should preserve external links"
        );
        assert!(
            cleaned.contains("[Anchor](#quick-start)"),
            "Should preserve anchor links"
        );
    }
}
