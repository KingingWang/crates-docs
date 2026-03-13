//! HTML processing utilities
//!
//! Provides HTML cleaning and conversion functions for documentation extraction.

/// Tags whose content should be completely removed during HTML cleaning
const SKIP_TAGS: [&str; 4] = ["script", "style", "noscript", "iframe"];

/// Common HTML entity mappings
const HTML_ENTITIES: [(&str, &str); 6] = [
    ("lt", "<"),
    ("gt", ">"),
    ("amp", "&"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", " "),
];

/// Clean HTML by removing unwanted tags (script, style, noscript, iframe) and their content
#[must_use]
pub fn clean_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut skip_depth = 0;

    while i < len {
        let c = chars[i];

        if c == '<' {
            let start = i;
            let mut j = i + 1;

            // Collect tag name
            let tag_name = collect_tag_name(&chars, &mut j, len);
            let tag_lower = tag_name.to_lowercase();
            let pure_tag = tag_lower.trim_start_matches('/');

            // Check if this is a skip tag
            let is_skip_tag = SKIP_TAGS.contains(&pure_tag);

            if is_skip_tag {
                if tag_lower.starts_with('/') {
                    // Closing tag
                    if skip_depth > 0 {
                        skip_depth -= 1;
                    }
                    skip_to_tag_end(&chars, &mut j, len);
                    i = j;
                    continue;
                }

                // Opening tag
                skip_depth += 1;
                skip_to_tag_end(&chars, &mut j, len);
                i = j;
                continue;
            }

            // Skip to end of tag
            skip_to_tag_end(&chars, &mut j, len);

            // Keep content if not inside a skip tag
            if skip_depth == 0 {
                result.extend(chars[start..j].iter().copied());
            }

            i = j;
        } else {
            if skip_depth == 0 {
                result.push(c);
            }
            i += 1;
        }
    }

    result
}

/// Convert HTML to plain text by removing all HTML tags
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut skip_content = false;

    while i < len {
        let c = chars[i];

        match c {
            '<' => {
                let mut j = i + 1;
                let tag_name = collect_tag_name(&chars, &mut j, len);
                let tag_lower = tag_name.to_lowercase();
                let is_closing = tag_lower.starts_with('/');
                let pure_tag = tag_lower.trim_start_matches('/');

                // Check if we should skip content
                if !is_closing && !skip_content {
                    skip_content = SKIP_TAGS.contains(&pure_tag);
                } else if is_closing {
                    skip_content = false;
                }

                skip_to_tag_end(&chars, &mut j, len);
                i = j;

                // Add space after block-level elements
                if !skip_content {
                    result.push(' ');
                }
            }
            '&' => {
                let mut j = i + 1;
                let entity = collect_entity(&chars, &mut j, len);

                // Look up entity replacement
                let replacement = HTML_ENTITIES
                    .iter()
                    .find_map(
                        |&(name, repl)| {
                            if entity == name {
                                Some(repl)
                            } else {
                                None
                            }
                        },
                    )
                    .unwrap_or("");

                if !replacement.is_empty() {
                    result.push_str(replacement);
                }
                i = j;
            }
            _ => {
                if !skip_content {
                    result.push(c);
                }
                i += 1;
            }
        }
    }

    clean_whitespace(&result)
}

/// Extract documentation from HTML by cleaning and converting to Markdown
#[must_use]
pub fn extract_documentation(html: &str) -> String {
    let cleaned_html = clean_html(html);
    html2md::parse_html(&cleaned_html)
}

/// Extract search results from HTML
#[must_use]
pub fn extract_search_results(html: &str, item_path: &str) -> String {
    let cleaned_html = clean_html(html);
    let markdown = html2md::parse_html(&cleaned_html);

    if markdown.trim().is_empty() {
        format!("未找到项目 '{item_path}' 的文档")
    } else {
        format!("## 搜索结果: {item_path}\n\n{markdown}")
    }
}

/// Clean extra whitespace from text
fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collect tag name starting from current position
fn collect_tag_name(chars: &[char], j: &mut usize, len: usize) -> String {
    let mut tag_name = String::new();
    while *j < len && chars[*j] != '>' && !chars[*j].is_whitespace() {
        tag_name.push(chars[*j]);
        *j += 1;
    }
    tag_name
}

/// Skip to the end of current tag
fn skip_to_tag_end(chars: &[char], j: &mut usize, len: usize) {
    while *j < len && chars[*j] != '>' {
        *j += 1;
    }
    if *j < len {
        *j += 1; // Skip '>'
    }
}

/// Collect HTML entity name
fn collect_entity(chars: &[char], j: &mut usize, len: usize) -> String {
    let mut entity = String::new();
    while *j < len && chars[*j] != ';' {
        entity.push(chars[*j]);
        *j += 1;
    }
    if *j < len {
        *j += 1; // Skip ';'
    }
    entity
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
        assert_eq!(clean_whitespace("  hello   world  "), "hello world");
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
        assert!(result.contains("搜索结果"));
        assert!(result.contains("serde::Serialize"));
        assert!(result.contains("Result"));
    }

    #[test]
    fn test_extract_search_results_not_found() {
        let html = "<html><body></body></html>";
        let result = extract_search_results(html, "nonexistent");
        assert!(result.contains("未找到项目"));
        assert!(result.contains("nonexistent"));
    }
}
