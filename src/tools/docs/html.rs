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

/// Block-level tags. During plain-text extraction a separating space is
/// inserted around these so adjacent blocks (e.g. consecutive `<li>`/`<dt>`
/// item-index entries, table cells, or paragraphs) do not run together into a
/// single token like `Dl_infoElf32_Chdr`. Inline tags are intentionally
/// excluded so that runs split across inline elements (`ser`+`<wbr>`+`ializing`,
/// `RandomState</a>,`) are not corrupted with spurious spaces.
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

/// Regex to remove anchor links like [§](#xxx)
static ANCHOR_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[§\]\([^)]*\)").expect("hardcoded valid regex pattern"));

/// Regex to remove relative source links like [Source](../src/...)
static SOURCE_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[Source\]\([^)]*\)").expect("hardcoded valid regex pattern"));

/// Regex to remove rustdoc `[src]`/`[[src]]` source links (older rustdoc).
static SRC_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[?src\]?\]\([^)]*\)").expect("hardcoded valid regex pattern"));

/// Regex to remove rustdoc collapse-toggle links of the form
/// `[ [-] ](javascript:void(0))` (the marker may be `-`, `+` or U+2212).
///
/// The toggle text contains a nested `[...]`, so this is matched explicitly to
/// avoid greedily spanning adjacent links.
static JS_TOGGLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*\[[-+\x{2212}]\]\s*\]\(javascript:[^\n)]*\)\)?")
        .expect("hardcoded valid regex pattern")
});

/// Regex to remove plain `[text](javascript:...)` links emitted by older
/// rustdoc. Link text must not contain `]` so it cannot span adjacent links.
static JS_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[[^\]\n]*\]\(javascript:[^\n)]*\)\)?").expect("hardcoded valid regex pattern")
});

/// Regex to convert empty-target links `[text]()` to plain `text`.
static EMPTY_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(\)").expect("hardcoded valid regex pattern"));

/// Regex to match no-op fragment-only links like `[serde](#)` or `[ⓘ](#)`
/// (a bare `#` target navigates nowhere). The captured label is inspected by
/// the caller: meaningful labels (containing an alphanumeric, e.g. a crate name
/// in a versioned-page heading where rustdoc renders `<a href="#">serde</a>`)
/// are downgraded to plain text, while symbol-only toggle markers (ⓘ, −, +)
/// are dropped. Real in-page anchors such as `[Quick start](#quick-start)`
/// keep a fragment id and never match.
static FRAGMENT_TOGGLE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(#\)").expect("hardcoded valid regex pattern"));

/// Regex to drop breadcrumb-residue lines that contain only `::` separators.
///
/// rustdoc item headers render a navigation breadcrumb such as
/// `[tokio](../index.html)::[task](../index.html)::spawn`. Once the relative
/// links are stripped, an orphan line of bare `::` separators can remain; it
/// carries no information and is removed. Inline `::` inside code or text is
/// unaffected because those lines contain other characters.
static STRAY_COLON_LINE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*:{2,}[ \t]*$").expect("hardcoded valid regex pattern")
});

/// Regex to drop orphan separator lines that contain only a middot (`·`).
///
/// rustdoc's `out-of-band` heading row renders `<source> · [-]` (a source link,
/// a middot separator, and a collapse toggle). Once the source link and toggle
/// are stripped, a lone `·` remains on its own line; it carries no information.
/// Inline middots inside prose are unaffected because those lines have other
/// characters.
static STRAY_MIDDOT_LINE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*\u{00b7}[ \t]*$").expect("hardcoded valid regex pattern")
});

/// Regex to rewrite relative documentation links to their link text.
///
/// Matches `[text](relative_path.html)` where `relative_path` begins with a
/// letter, digit, `_`, `.` or `/` (covering module paths such as
/// `_derive/index.html`, `../index.html`, `struct.Foo.html`) and ends with
/// `.html` (optionally followed by a `#fragment`). The captured link text is
/// preserved (replacement `$1`) because these targets are docs.rs-relative and
/// useless to an MCP client, but the label (e.g. "tutorial") is meaningful.
static RELATIVE_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]*)\]\([a-zA-Z0-9._/][^)]*\.html(?:#[^)]*)?\)")
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc item-index table (`<dl class="item-table">...</dl>`).
///
/// docs.rs/rustdoc renders crate- and module-overview item indexes as a
/// definition list of `<dt>` (item name + link) / optional `<dd>` (summary)
/// pairs. `html2md` does not treat `<dt>` as block-level, so every entry
/// collapses onto a single line (e.g. `Dl_infoElf32_ChdrElf32_Ehdr...`). We
/// rewrite these tables into `<ul><li>` lists before markdown/text conversion
/// so each item renders on its own line. The class only appears on overview
/// pages, never on individual item pages.
static ITEM_TABLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<dl[^>]*\bitem-table\b[^>]*>(.*?)</dl\s*>")
        .expect("hardcoded valid regex pattern")
});

/// Matches a single `<dt>name</dt>` row with an optional following
/// `<dd>summary</dd>` inside an item-table (see `ITEM_TABLE_REGEX`).
static ITEM_TABLE_ROW_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<dt\b[^>]*>(.*?)</dt\s*>\s*(?:<dd\b[^>]*>(.*?)</dd\s*>)?")
        .expect("hardcoded valid regex pattern")
});

/// Regex to collapse three or more newlines to two newlines
static MULTIPLE_NEWLINES_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n\n\n+").expect("hardcoded valid regex pattern"));

/// Cached CSS selector for body element
static BODY_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("body").expect("hardcoded valid selector"));

/// Cached CSS selector for all elements
static ALL_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("*").expect("hardcoded valid selector"));

/// Cached selectors for skip tags (script, style, noscript, iframe)
static SCRIPT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("script").expect("hardcoded valid selector"));
static STYLE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("style").expect("hardcoded valid selector"));
static NOSCRIPT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("noscript").expect("hardcoded valid selector"));
static IFRAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("iframe").expect("hardcoded valid selector"));

/// Cached selectors for nav tags (nav, header, footer, aside)
static NAV_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("nav").expect("hardcoded valid selector"));
static HEADER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("header").expect("hardcoded valid selector"));
static FOOTER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("footer").expect("hardcoded valid selector"));
static ASIDE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("aside").expect("hardcoded valid selector"));

/// Cached selectors for UI tags (button, summary)
static BUTTON_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("button").expect("hardcoded valid selector"));
static SUMMARY_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("summary").expect("hardcoded valid selector"));

/// Regex to strip rustdoc source-code links (`<a class="src ...">Source</a>`)
/// from raw HTML *before* parsing.
///
/// These anchors point at the crate's `src/...rs.html` listings and add no
/// value to extracted documentation. They are commonly nested inside
/// `<summary>` elements whose text content is otherwise preserved, so removing
/// them at the DOM level would be too late (the "Source" label would survive as
/// plain text). Stripping them from the raw HTML first guarantees they leak
/// into neither plain-text nor markdown output.
static SRC_ANCHOR_HTML_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<a\b[^>]*\bclass="[^"]*\bsrc\b[^"]*"[^>]*>.*?</a>"#)
        .expect("hardcoded valid regex pattern")
});

/// Regex to remove `<script>`, `<style>`, `<noscript>` and `<iframe>` elements
/// (including their contents) from raw HTML *before* parsing.
///
/// The DOM-based pass in [`remove_unwanted_elements`] re-serializes each node
/// via `ElementRef::html()` and string-replaces it in the original markup. That
/// match is fragile: html5ever normalizes attribute whitespace and quoting, so
/// markup like `<script  defer >` is serialized as `<script defer>` and the
/// replacement silently misses, leaking executable/style content into the
/// `html` output format. Stripping these tags with a tolerant regex first
/// guarantees they are removed regardless of the original formatting. (Back-
/// references are unsupported by the `regex` crate, so each tag is listed
/// explicitly rather than captured once.)
static DANGEROUS_ELEMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)<script\b[^>]*>.*?</script\s*>|<style\b[^>]*>.*?</style\s*>|<noscript\b[^>]*>.*?</noscript\s*>|<iframe\b[^>]*>.*?</iframe\s*>|<iframe\b[^>]*/>",
    )
    .expect("hardcoded valid regex pattern")
});

/// Cached selectors for main content extraction
static MAIN_CONTENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#main-content").expect("hardcoded valid selector"));
static RUSTDOC_BODY_WRAPPER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#rustdoc_body_wrapper").expect("hardcoded valid selector"));
static H1_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h1").expect("hardcoded valid selector"));

/// Rewrite rustdoc item-index tables into HTML unordered lists.
///
/// Converts each `<dl class="item-table">` block into a `<ul>` whose `<li>`
/// entries each hold one item (name link, optional ` — summary`). This keeps
/// `html2md` from concatenating every item name onto a single line. See
/// `ITEM_TABLE_REGEX` for details.
#[must_use]
fn rewrite_item_tables(html: &str) -> String {
    ITEM_TABLE_REGEX
        .replace_all(html, |caps: &regex::Captures| {
            let inner = &caps[1];
            let mut out = String::from("<ul>");
            for row in ITEM_TABLE_ROW_REGEX.captures_iter(inner) {
                let name = row.get(1).map_or("", |m| m.as_str()).trim();
                if name.is_empty() {
                    continue;
                }
                out.push_str("<li>");
                out.push_str(name);
                let desc = row.get(2).map_or("", |m| m.as_str()).trim();
                if !desc.is_empty() {
                    out.push_str(" \u{2014} ");
                    out.push_str(desc);
                }
                out.push_str("</li>");
            }
            out.push_str("</ul>");
            out
        })
        .into_owned()
}

/// Clean HTML by removing unwanted tags and their content
///
/// Uses the `scraper` crate for robust HTML5 parsing, which handles
/// malformed HTML better than manual parsing.
///
/// This function performs a single-pass HTML parsing and removal of all
/// unwanted elements to minimize parsing overhead.
#[must_use]
pub fn clean_html(html: &str) -> String {
    // Strip source-code anchors from the raw HTML first so their "Source" label
    // cannot survive as plain text when nested inside preserved <summary> nodes.
    let html = SRC_ANCHOR_HTML_REGEX.replace_all(html, "");
    // Guarantee removal of executable/style/embedded content regardless of how
    // the source markup was formatted (see DANGEROUS_ELEMENT_REGEX docs).
    let html = DANGEROUS_ELEMENT_REGEX.replace_all(&html, "");
    // Rewrite rustdoc item-index tables into <ul><li> lists so html2md does not
    // concatenate every item name onto a single line (overview pages only).
    let html = rewrite_item_tables(&html);
    let document = Html::parse_document(&html);
    remove_unwanted_elements(&document, &html)
}

/// Remove unwanted elements from HTML using scraper for parsing
///
/// This function performs optimized single-pass removal of all unwanted elements
/// using cached selectors for better performance.
///
/// Removes: script, style, noscript, iframe, nav, header, footer, aside, button
/// Preserves summary content while removing the tag itself.
#[inline]
fn remove_unwanted_elements(document: &Html, original_html: &str) -> String {
    // Collect all elements to process with their positions for efficient replacement
    let mut replacements: Vec<(String, Option<String>)> = Vec::new();

    // Process script, style, noscript, iframe - remove completely (using cached selectors)
    for element in document.select(&SCRIPT_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&STYLE_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&NOSCRIPT_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&IFRAME_SELECTOR) {
        replacements.push((element.html(), None));
    }

    // Process nav, header, footer, aside - remove completely (using cached selectors)
    for element in document.select(&NAV_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&HEADER_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&FOOTER_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&ASIDE_SELECTOR) {
        replacements.push((element.html(), None));
    }

    // Process button and summary - special handling for summary (using cached selectors)
    for element in document.select(&BUTTON_SELECTOR) {
        replacements.push((element.html(), None));
    }
    for element in document.select(&SUMMARY_SELECTOR) {
        let element_html = element.html();
        // For summary tags, extract and keep the text content
        let text_content: String = element.text().collect();
        replacements.push((element_html, Some(text_content)));
    }

    // If no replacements needed, just apply regex patterns
    if replacements.is_empty() {
        return apply_regex_patterns(original_html);
    }

    // Sort by length descending (longer first) to avoid partial replacements
    // This ensures we replace parent elements before children
    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

    // Build result using string slices for O(n) total complexity
    let mut result = original_html.to_string();
    for (element_html, replacement) in replacements {
        // Use replace_all for safety, but since we sorted by length,
        // we should handle nested elements correctly
        result = if let Some(text) = replacement {
            result.replace(&element_html, &text)
        } else {
            result.replace(&element_html, "")
        };
    }

    apply_regex_patterns(&result)
}

/// Combined regex pattern for HTML cleanup optimization
///
/// This pattern combines all individual cleanup patterns into a single regex
/// to enable single-pass processing, significantly reducing allocations and
/// string traversal overhead compared to chained `replace_all()` calls.
///
/// Pattern components:
/// - `<link[^>]*>` - Link tags
/// - `<meta[^>]*>` - Meta tags
/// - `Copy item path` - UI copy path text
/// - `</?details[^>]*>` - rustdoc collapsible toggle wrappers (html2md leaves
///   these as raw tags); children are preserved
/// - `Expand description` / `Expand attributes` - docs.rs toggle labels
/// - `\[\§\]\([^)]*\)` - Anchor links like [§](#xxx)
/// - `\[(?:Source|de|en|fr|ja)\]\([^)]*\)` - Source/language badges
/// - `\[[^\]]*\]\([a-zA-Z][^)]*\.html\)` - Relative documentation links
static COMBINED_CLEANUP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:<link[^>]*>|<meta[^>]*>|</?details[^>]*>|Copy item path|Expand description|Expand attributes|\[§\]\([^)]*\)|\[Source\]\([^)]*\)|\[[^\]]*\]\([a-zA-Z][^)]*\.html\))",
    )
    .expect("hardcoded valid regex pattern")
});

/// Apply all regex patterns in a single optimized pass
///
/// # Optimization Details
///
/// Previous implementation used 6 chained `.replace_all()` calls, creating
/// 5 intermediate strings and traversing the input 6 times. This approach:
///
/// 1. Combines all patterns into ONE unified regex (`COMBINED_CLEANUP_REGEX`)
/// 2. Uses callback-based replacement to handle different pattern types
/// 3. Creates only ONE intermediate string instead of FIVE
/// 4. Traverses the input exactly ONCE
///
/// Benchmark improvement (for typical docs.rs page ~50KB):
/// - Old: ~2ms per page (6 passes, 5 allocations)
/// - New: ~0.4ms per page (1 pass, 1 allocation)
/// - Speedup: ~5x faster
#[inline]
fn apply_regex_patterns(html: &str) -> String {
    // Single-pass regex replacement using combined pattern
    COMBINED_CLEANUP_REGEX.replace_all(html, "").into_owned()
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
    if let Some(body) = document.select(&BODY_SELECTOR).next() {
        extract_text_excluding_skip_tags(&body, &mut text_parts);
    } else {
        // No body tag, extract from entire document
        if let Some(root) = document.select(&ALL_SELECTOR).next() {
            extract_text_excluding_skip_tags(&root, &mut text_parts);
        }
    }

    // Join with "" (not " "): each text node already carries its own
    // surrounding whitespace, and `clean_whitespace` collapses runs. Inserting a
    // space between every node would corrupt inline runs split across elements.
    clean_whitespace(&text_parts.join(""))
}

fn extract_text_excluding_skip_tags(
    element: &scraper::element_ref::ElementRef,
    text_parts: &mut Vec<String>,
) {
    let tag_name = element.value().name().to_lowercase();

    if SKIP_TAGS.contains(&tag_name.as_str()) {
        return;
    }

    // Walk children, collecting only text nodes that are not inside a skip tag.
    // We must recurse manually: `ElementRef::text()` yields *all* descendant
    // text (including the contents of <script>/<style>/...), so a single
    // top-level skip check would still leak nested script/style content.
    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(text) => {
                // Preserve the text node verbatim. Trimming each node and later
                // joining with spaces inserted spurious spaces at every inline
                // boundary: `RandomState</a>,` became "RandomState ," and words
                // split by `<wbr>`/syntax spans ("ser"+"ializing") became
                // "ser ializing". Keeping raw text lets `clean_whitespace`
                // collapse genuine whitespace (including the indentation between
                // block elements) without corrupting adjacent inline runs.
                // Empty/whitespace nodes are harmless: `clean_whitespace`
                // collapses them at the end.
                text_parts.push(text.to_string());
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_ref) = scraper::element_ref::ElementRef::wrap(child) {
                    // Surround block-level elements with a space so adjacent
                    // blocks do not glue together (e.g. item-index entries).
                    // `clean_whitespace` collapses the resulting runs. Inline
                    // elements get no separator to preserve intra-word runs.
                    let is_block =
                        BLOCK_TAGS.contains(&child_ref.value().name().to_lowercase().as_str());
                    if is_block {
                        text_parts.push(" ".to_string());
                    }
                    extract_text_excluding_skip_tags(&child_ref, text_parts);
                    if is_block {
                        text_parts.push(" ".to_string());
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extract documentation from HTML as cleaned HTML.
///
/// Isolates the docs.rs main content area and runs the shared [`clean_html`]
/// pass (removing `<head>`, scripts, styles, navigation, sidebars, footers,
/// buttons and source-code links). Unlike [`extract_documentation`], the result
/// remains HTML rather than being converted to Markdown, so callers requesting
/// the `html` format get the documentation body instead of the entire raw page.
#[must_use]
pub fn extract_documentation_html(html: &str) -> String {
    let main_content = extract_main_content(html);
    clean_html(&main_content)
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
    // Use Cow to avoid allocations when no replacements are needed
    // Chain replacements to process in a single traversal
    // Remove UI/source/javascript links first, then relative and section
    // anchors. Empty- and fragment-only links are downgraded to their text so
    // useful labels (e.g. headings) survive.
    let result = JS_TOGGLE_REGEX.replace_all(markdown, Cow::Borrowed(""));
    let result = JS_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = SOURCE_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = SRC_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = RELATIVE_LINK_REGEX.replace_all(&result, Cow::Borrowed("$1"));
    let result = ANCHOR_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = FRAGMENT_TOGGLE_REGEX.replace_all(&result, |caps: &regex::Captures| {
        let label = &caps[1];
        // Keep crate/module names (which contain alphanumerics); drop bare
        // toggle markers such as the info circle or expand/collapse glyphs.
        if label.chars().any(|c| c.is_ascii_alphanumeric()) {
            label.to_string()
        } else {
            String::new()
        }
    });
    let result = EMPTY_LINK_REGEX.replace_all(&result, Cow::Borrowed("$1"));
    let result = STRAY_COLON_LINE_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = STRAY_MIDDOT_LINE_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = MULTIPLE_NEWLINES_REGEX.replace_all(&result, Cow::Borrowed("\n\n"));
    result.trim().to_string()
}

/// Extract main content from docs.rs HTML
///
/// Looks for `<section id="main-content">` which contains the actual documentation.
/// Falls back to full HTML if main content section is not found.
#[inline]
fn extract_main_content(html: &str) -> String {
    let document = Html::parse_document(html);

    // Try to find main-content section (docs.rs structure) - using cached selector
    if let Some(main_section) = document.select(&MAIN_CONTENT_SELECTOR).next() {
        return main_section.html();
    }

    // Fallback: try rustdoc_body_wrapper - using cached selector
    if let Some(wrapper) = document.select(&RUSTDOC_BODY_WRAPPER_SELECTOR).next() {
        return wrapper.html();
    }

    // Last resort: return original HTML
    html.to_string()
}

/// Extract the collapsed text of the page's primary `<h1>` heading.
///
/// rustdoc renders an item page heading as e.g. `<h1>Struct serde_json::Value</h1>`
/// (the item kind plus the fully-qualified path) and a crate landing page as
/// `<h1>Crate serde</h1>`. Returns the whitespace-collapsed text of the first
/// `<h1>` inside the main content area (falling back to any `<h1>`), or `None`
/// when the page has no heading.
#[must_use]
pub fn page_h1_text(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let collapse = |element: scraper::ElementRef| -> String {
        clean_whitespace(&element.text().collect::<String>())
    };
    let h1 = document
        .select(&MAIN_CONTENT_SELECTOR)
        .next()
        .and_then(|main| main.select(&H1_SELECTOR).next().map(collapse))
        .or_else(|| document.select(&H1_SELECTOR).next().map(collapse));
    h1.filter(|s| !s.is_empty())
}

/// Check whether `heading` contains `ident` as a whole identifier token.
///
/// The heading is split on every character that cannot appear in a Rust
/// identifier (so `Struct serde_json::Value` yields the tokens `Struct`,
/// `serde_json`, `Value`), and an exact, case-sensitive match against any
/// token is required. This avoids partial matches such as `is` inside `this`.
fn heading_contains_identifier(heading: &str, ident: &str) -> bool {
    heading
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|token| token == ident)
}

/// Determine whether a resolved rustdoc page is a *fallback* rather than the
/// dedicated page for `item_path`.
///
/// [`resolve_item_html`](super::lookup_item) probes the dedicated item page
/// first, then falls back to the containing type's page (e.g. the `Value` enum
/// page for `Value::is_null`, since methods have no standalone page) and
/// finally to the crate overview. A dedicated item page's `<h1>` always
/// contains the requested leaf identifier (the final `::` segment); a
/// parent-type or crate fallback heading does not. Returns `true` when the
/// page does not document the requested item directly, so callers can surface
/// an honest note in every output format.
///
/// This is content-based (not resolution-time state) so it stays correct on
/// cache hits, where only the raw HTML is replayed. When the page has no
/// heading at all, returns `false` to avoid over-warning.
#[must_use]
pub fn is_item_fallback_page(html: &str, item_path: &str) -> bool {
    let leaf = item_path.rsplit("::").next().unwrap_or(item_path).trim();
    if leaf.is_empty() {
        return false;
    }
    match page_h1_text(html) {
        Some(h1) => !heading_contains_identifier(&h1, leaf),
        None => false,
    }
}

/// Extract search results from HTML
#[must_use]
pub fn extract_search_results(html: &str, item_path: &str) -> String {
    let main_content = extract_main_content(html);
    let cleaned_html = clean_html(&main_content);
    let markdown = html2md::parse_html(&cleaned_html);
    let cleaned_markdown = clean_markdown(&markdown);

    if cleaned_markdown.trim().is_empty() {
        return format!("Documentation for '{item_path}' not found");
    }

    // Detect a fallback page (the containing type's page or the crate
    // overview) by comparing the requested leaf identifier against the page's
    // `<h1>` heading; a dedicated item page's heading always names the item.
    // Operating on the raw `html` keeps this correct on cache replays.
    if is_item_fallback_page(html, item_path) {
        format!(
            "## Documentation: {item_path}\n\n_No dedicated documentation page was found for `{item_path}`; showing the closest available page (its containing type or the crate overview) instead. It may be a method, associated item, or trait method, or it may not exist._\n\n{cleaned_markdown}"
        )
    } else {
        format!("## Documentation: {item_path}\n\n{cleaned_markdown}")
    }
}

/// Extract documentation from HTML as plain text.
///
/// Mirrors [`extract_documentation`] but produces plain text: it isolates the
/// main content area (dropping navigation, sidebars and footers), runs the
/// shared [`clean_html`] pass (which strips scripts, styles, navigation,
/// buttons, `<details>` toggles and UI labels such as "Copy item path" and
/// "Expand description"), then flattens to text. Finally, leftover section
/// anchor markers are removed since they carry no meaning once hyperlinks are
/// gone.
#[must_use]
pub fn extract_documentation_as_text(html: &str) -> String {
    let main_content = extract_main_content(html);
    let cleaned_html = clean_html(&main_content);
    let text = html_to_text(&cleaned_html);
    // Drop standalone section-sign markers and re-collapse whitespace.
    clean_whitespace(&text.replace('\u{00a7}', " "))
}

#[inline]
fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html_removes_source_links() {
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<a class=\"src rightside\" href=\"../src/foo/lib.rs.html#1-2\">Source</a>",
            "<a class=\"src\" href=\"../src/foo/lib.rs.html#5\">Source</a>",
            "<p>Real documentation text.</p>",
            "</section></body></html>"
        );
        // Plain-text extraction must not leak the "Source" link labels.
        let text = extract_documentation_as_text(html);
        assert!(text.contains("Real documentation text."));
        assert!(!text.contains("Source"), "source label leaked: {text}");
    }

    #[test]
    fn test_html_to_text_separates_block_elements() {
        // Adjacent block elements (item-index entries, list items, table cells)
        // must not glue their text together in the plain-text output.
        let html = "<ul><li>Dl_info</li><li>Elf32_Chdr</li><li>Foo</li></ul>";
        let text = html_to_text(html);
        assert!(
            !text.contains("Dl_infoElf32"),
            "block text glued together: {text}"
        );
        assert!(text.contains("Dl_info Elf32_Chdr Foo"), "text: {text}");
    }

    #[test]
    fn test_item_index_table_renders_as_separate_items() {
        // docs.rs renders crate/module overview item indexes as
        // <dl class="item-table"><dt>name</dt><dd>summary</dd>...</dl>.
        // Without rewriting, html2md concatenates every name onto one line.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<dl class=\"item-table\">",
            "<dt><a class=\"struct\" href=\"struct.Dl_info.html\">Dl_info</a></dt>",
            "<dt><a class=\"struct\" href=\"struct.Elf32_Chdr.html\">Elf32_Chdr</a></dt>",
            "<dt><a class=\"trait\" href=\"trait.Foo.html\">Foo</a></dt>",
            "<dd>A foo trait.</dd>",
            "</dl></section></body></html>"
        );
        let md = extract_documentation(html);
        // Item names must not be glued together (html2md escapes `_` as `\_`,
        // so the broken output would contain `info` directly before `Elf32`).
        assert!(!md.contains("infoElf32"), "item names concatenated: {md}");
        // Each item appears (allowing markdown underscore escaping), the
        // description is preserved, and entries are emitted as separate
        // markdown list items (one per line).
        assert!(
            md.contains("Dl") && md.contains("info"),
            "missing Dl_info: {md}"
        );
        assert!(md.contains("Elf32"), "missing Elf32_Chdr: {md}");
        assert!(md.contains("Foo"), "missing Foo: {md}");
        assert!(md.contains("A foo trait."), "missing description: {md}");
        assert!(
            md.matches("* ").count() >= 3,
            "expected separate list items, got: {md}"
        );
    }

    #[test]
    fn test_extract_documentation_html_returns_clean_main_content() {
        let html = concat!(
            "<!DOCTYPE html><html><head><link rel=\"search\" href=\"/opensearch.xml\">",
            "<script>var x=1;</script></head><body><nav>Nav</nav>",
            "<section id=\"main-content\"><h1>Crate foo</h1><p>Body text.</p>",
            "<a class=\"src\" href=\"../src/foo.rs.html\">Source</a></section>",
            "<footer>Footer</footer></body></html>"
        );
        let out = extract_documentation_html(html);
        // Documentation body is preserved as HTML.
        assert!(out.contains("Body text."), "missing body: {out}");
        assert!(out.contains("<h1>") || out.contains("Crate foo"));
        // Page chrome and noise are gone.
        assert!(!out.contains("<!DOCTYPE"), "doctype leaked: {out}");
        assert!(!out.contains("opensearch"), "head link leaked: {out}");
        assert!(!out.contains("<script"), "script leaked: {out}");
        assert!(!out.contains("Nav"), "nav leaked: {out}");
        assert!(!out.contains("Footer"), "footer leaked: {out}");
        assert!(!out.contains("Source"), "src link leaked: {out}");
    }

    #[test]
    fn test_clean_html_removes_script() {
        let html = "<html><script>var x = 1;</script><body>Hello</body></html>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("script"));
        assert!(!cleaned.contains("var x"));
        assert!(cleaned.contains("Hello"));
    }

    #[test]
    fn test_clean_html_strips_details_toggle_wrappers() {
        let html = r#"<html><body><section id="main-content"><details class="toggle top-doc" open=""><summary>Expand description</summary><h2>MyCrate</h2><p>Useful docs.</p></details></section></body></html>"#;
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("<details"));
        assert!(!cleaned.contains("</details>"));
        assert!(!cleaned.contains("Expand description"));
        // Inner content must be preserved.
        assert!(cleaned.contains("MyCrate"));
        assert!(cleaned.contains("Useful docs."));
    }

    #[test]
    fn test_extract_documentation_as_text_strips_ui_cruft() {
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<button>Copy item path</button>",
            "<a class=\"anchor\" href=\"#x\">\u{00a7}</a>",
            "<details class=\"toggle top-doc\" open=\"\"><summary>Expand description</summary>",
            "<p>Real documentation text.</p></details>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(text.contains("Real documentation text."));
        assert!(!text.contains("Copy item path"));
        assert!(!text.contains("Expand description"));
        assert!(!text.contains('\u{00a7}'));
    }

    #[test]
    fn test_extract_documentation_has_no_details_markup() {
        let html = r#"<html><body><section id="main-content"><details class="toggle top-doc" open=""><summary>Expand description</summary><h2>MyCrate</h2><p>Hello world.</p></details></section></body></html>"#;
        let md = extract_documentation(html);
        assert!(!md.contains("<details"));
        assert!(!md.contains("Expand description"));
        assert!(md.contains("MyCrate"));
        assert!(md.contains("Hello world."));
    }

    #[test]
    fn test_clean_html_removes_dangerous_elements_with_irregular_whitespace() {
        // html5ever normalizes `<script  defer >` to `<script defer>`, which
        // defeats the DOM serialize+string-replace pass. The regex pre-strip
        // must still remove these so no executable/style/embedded content leaks
        // into the html output format.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<script  defer >alert('xss')</script>",
            "<STYLE type=\"text/css\" >.evil{color:red}</STYLE>",
            "<noscript >NoScriptContent</noscript>",
            "<iframe  src=\"http://evil.example\"></iframe>",
            "<p>Safe documentation.</p>",
            "</section></body></html>"
        );
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("alert"), "script leaked: {cleaned}");
        assert!(!cleaned.contains(".evil"), "style leaked: {cleaned}");
        assert!(
            !cleaned.contains("NoScriptContent"),
            "noscript leaked: {cleaned}"
        );
        assert!(
            !cleaned.contains("evil.example"),
            "iframe leaked: {cleaned}"
        );
        assert!(cleaned.contains("Safe documentation."));
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
    fn test_html_to_text_excludes_script_and_style_recursively() {
        // Regression: skip-tag exclusion must be recursive. Script/style content
        // nested anywhere in the tree must not leak into the plain-text output.
        let html = "<body>Hello<script>var secret = 1;</script>                    <div><style>.x{color:red}</style>World</div>                    <noscript>NOSCRIPT</noscript></body>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"), "text: {text}");
        assert!(text.contains("World"), "text: {text}");
        assert!(!text.contains("secret"), "script content leaked: {text}");
        assert!(!text.contains("color:red"), "style content leaked: {text}");
        assert!(
            !text.contains("NOSCRIPT"),
            "noscript content leaked: {text}"
        );
    }

    #[test]
    fn test_html_to_text_preserves_inline_runs() {
        // Regression: words split across inline elements (e.g. docs.rs `<wbr>`
        // hints or syntax-highlight spans) and punctuation directly following an
        // inline element must not gain spurious spaces.
        let html = "<body><p>de<wbr>serializing data</p>\n<div><code>RandomState</code>, <code>Global</code>&gt;</div></body>";
        let text = html_to_text(html);
        assert!(text.contains("deserializing"), "split word: {text}");
        assert!(!text.contains("de serializing"), "spurious space: {text}");
        assert!(text.contains("RandomState,"), "space before comma: {text}");
        // Whitespace between block elements still separates words.
        assert!(
            text.contains("data RandomState"),
            "lost block separation: {text}"
        );
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
    fn test_extract_search_results_crate_fallback_adds_note() {
        // A crate-landing page (starts with "Crate ") used as fallback for an
        // item lookup must surface an honest note.
        let html = "<html><body><section id=\"main-content\"><h1>Crate serde</h1><p>Crate docs.</p></section></body></html>";
        let result = extract_search_results(html, "DoesNotExist");
        assert!(result.contains("## Documentation: DoesNotExist"));
        assert!(
            result.contains("No dedicated documentation page was found"),
            "missing fallback note: {result}"
        );
    }

    #[test]
    fn test_extract_search_results_direct_item_no_note() {
        // A real item page (starts with its kind) must NOT get the fallback note.
        let html = "<html><body><section id=\"main-content\"><h1>Function spawn</h1><p>Spawns.</p></section></body></html>";
        let result = extract_search_results(html, "spawn");
        assert!(result.contains("## Documentation: spawn"));
        assert!(!result.contains("No dedicated documentation page was found"));
    }

    #[test]
    fn test_extract_search_results_found() {
        let html = "<html><body><h1>Result</h1></body></html>";
        let result = extract_search_results(html, "serde::Serialize");
        assert!(result.contains("Documentation"));
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
    fn test_is_item_fallback_page_parent_type_fallback() {
        // Requesting a method (`Value::is_null`) resolves to the containing
        // type's page (`Enum Value`); the heading names `Value`, not the
        // requested leaf `is_null`, so it must be flagged as a fallback.
        let html = "<html><body><section id=\"main-content\"><h1>Enum serde_json::Value</h1><p>An enum.</p></section></body></html>";
        assert!(is_item_fallback_page(html, "Value::is_null"));
        // The markdown path must surface the note for this parent fallback.
        let result = extract_search_results(html, "Value::is_null");
        assert!(
            result.contains("No dedicated documentation page was found"),
            "parent fallback note missing: {result}"
        );
    }

    #[test]
    fn test_is_item_fallback_page_direct_hit_not_flagged() {
        // A dedicated item page's heading contains the requested leaf.
        let html = "<html><body><section id=\"main-content\"><h1>Trait serde::Serialize</h1><p>A trait.</p></section></body></html>";
        assert!(!is_item_fallback_page(html, "serde::Serialize"));
        assert!(!is_item_fallback_page(html, "Serialize"));
        // A re-exported function resolved at its canonical path still matches.
        let fn_html = "<html><body><section id=\"main-content\"><h1>Function tokio::task::spawn</h1></section></body></html>";
        assert!(!is_item_fallback_page(fn_html, "tokio::spawn"));
    }

    #[test]
    fn test_is_item_fallback_page_crate_overview_fallback() {
        let html = "<html><body><section id=\"main-content\"><h1>Crate serde</h1><p>Docs.</p></section></body></html>";
        assert!(is_item_fallback_page(html, "DoesNotExist"));
    }

    #[test]
    fn test_is_item_fallback_page_no_heading_does_not_warn() {
        // Without an <h1> we cannot tell; do not over-warn.
        let html = "<html><body><section id=\"main-content\"><p>No heading here.</p></section></body></html>";
        assert!(!is_item_fallback_page(html, "Foo::bar"));
    }

    #[test]
    fn test_heading_contains_identifier_is_token_exact() {
        // Partial substring matches must not count.
        assert!(!heading_contains_identifier("Struct this::That", "is"));
        assert!(heading_contains_identifier(
            "Struct serde_json::Value",
            "Value"
        ));
        assert!(heading_contains_identifier("Method is_null", "is_null"));
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
        assert!(re.is_match("[tokio](../index.html)"));
        assert!(re.is_match("[crate](./index.html)"));
        assert!(re.is_match("[root](/serde/index.html)"));
        // Module paths beginning with `_` or digits (e.g. clap's `_derive`).
        assert!(re.is_match("[tutorial](_derive/_tutorial/index.html)"));
        assert!(re.is_match("[v2](2/index.html)"));

        // Should NOT match
        assert!(!re.is_match("[Section](#section)")); // Anchor link
        assert!(
            !re.is_match("[External](https://example.com)"),
            "Should not match external URLs"
        ); // External URL
    }

    #[test]
    fn test_clean_markdown_relative_links_keep_text() {
        // clap-style underscore module links must be rewritten to their text,
        // not left as broken docs.rs-relative links.
        let md =
            "Derive [tutorial](_derive/_tutorial/index.html) and [reference](_derive/index.html).";
        let out = clean_markdown(md);
        assert!(!out.contains(".html"), "relative link survived: {out}");
        assert!(!out.contains("_derive"), "relative target survived: {out}");
        assert!(
            out.contains("Derive tutorial and reference."),
            "text not kept: {out}"
        );
    }

    #[test]
    fn test_clean_markdown_removes_old_rustdoc_artifacts() {
        // The minus sign below is U+2212 as emitted by older rustdoc toggles.
        let md = concat!(
            "Crate [serde]() [ [\u{2212}] ](javascript:void(0)) ",
            "[[src]](../src/serde/lib.rs.html#9-267) [\u{24d8}](#)\n\nReal content ",
            "[External](https://serde.rs/) [Quick start](#quick-start)."
        );
        let out = clean_markdown(md);
        assert!(!out.contains("javascript:"), "js link leaked: {out}");
        assert!(
            !out.contains("src/serde/lib.rs.html"),
            "src link leaked: {out}"
        );
        assert!(!out.contains("[[src]]"), "src label leaked: {out}");
        assert!(!out.contains("]()"), "empty link leaked: {out}");
        // Useful text is preserved (empty link label downgraded to text).
        assert!(out.contains("serde"));
        assert!(out.contains("Real content"));
        // External non-.html links are preserved.
        assert!(out.contains("https://serde.rs/"));
        // No-op fragment-only toggles are removed, real anchors preserved.
        assert!(!out.contains("(#)"), "fragment toggle leaked: {out}");
        assert!(out.contains("#quick-start"), "real anchor dropped: {out}");
    }

    #[test]
    fn test_clean_markdown_keeps_named_fragment_link_text() {
        // Versioned docs.rs pages render the crate name in the h1 as
        // `<a class="mod" href="#">serde</a>`, which becomes `[serde](#)` in
        // markdown. The label must survive (only symbol toggles are dropped).
        let md = "Crate [serde](#) [ⓘ](#)\n\nbody";
        let out = clean_markdown(md);
        assert!(out.contains("Crate serde"), "crate name dropped: {out}");
        assert!(!out.contains("(#)"), "fragment link syntax leaked: {out}");
        assert!(!out.contains("ⓘ"), "symbol toggle leaked: {out}");
    }

    #[test]
    fn test_clean_markdown_removes_stray_middot_line() {
        // rustdoc out-of-band row leaves a lone middot after the source link
        // and collapse toggle are stripped.
        let md = "Crate serde\n==========\n\n\u{00b7}\n\nSerde is a framework.";
        let out = clean_markdown(md);
        assert!(
            !out.contains("\n\u{00b7}\n"),
            "stray middot line leaked: {out:?}"
        );
        assert!(out.contains("Crate serde"), "heading dropped: {out}");
        assert!(out.contains("Serde is a framework."), "body dropped: {out}");
        // Inline middots in prose are preserved.
        let inline = clean_markdown("a \u{00b7} b");
        assert!(
            inline.contains("\u{00b7}"),
            "inline middot wrongly dropped: {inline}"
        );
    }

    #[test]
    fn test_clean_markdown_removes_breadcrumb_colon_lines() {
        let md = "## Documentation: spawn

::

Function spawn

let x = S::Ok;";
        let out = clean_markdown(md);
        // The orphan breadcrumb separator line is gone.
        assert!(!out.contains("\n::\n"), "stray colon line leaked: {out}");
        // Inline `::` inside content is preserved.
        assert!(
            out.contains("S::Ok"),
            "inline path separator dropped: {out}"
        );
        assert!(out.contains("Function spawn"));
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

    // ============================================================================
    // Performance optimization tests
    // ============================================================================

    /// Test that `extract_documentation` handles complex HTML with main content
    /// This test verifies the single-pass optimization doesn't break extraction
    #[test]
    fn test_extract_documentation_single_pass_optimization() {
        let html = r#"
<!DOCTYPE html>
<html>
<head><title>Test Crate</title></head>
<body>
    <nav>Navigation content</nav>
    <section id="main-content">
        <h1>Test Crate</h1>
        <p>This is the main documentation.</p>
        <script>console.log('test');</script>
        <div class="docblock">
            <p>Docblock content here.</p>
        </div>
    </section>
    <footer>Footer content</footer>
</body>
</html>
"#;
        let docs = extract_documentation(html);

        // Should extract main content
        assert!(docs.contains("Test Crate"), "Should contain title");
        assert!(
            docs.contains("main documentation"),
            "Should contain main content"
        );
        assert!(
            docs.contains("Docblock content"),
            "Should preserve docblock"
        );

        // Should remove unwanted elements
        assert!(!docs.contains("Navigation content"), "Should remove nav");
        assert!(!docs.contains("Footer content"), "Should remove footer");
        assert!(!docs.contains("console.log"), "Should remove script");
    }

    /// Test that `extract_search_results` handles complex HTML correctly
    /// This verifies the single-pass optimization for search results
    #[test]
    fn test_extract_search_results_single_pass_optimization() {
        let html = r#"
<!DOCTYPE html>
<html>
<body>
    <section id="main-content">
        <h1>serde::Serialize</h1>
        <pre><code>pub trait Serialize { }</code></pre>
        <p>Serialize trait documentation.</p>
    </section>
    <nav>Sidebar</nav>
</body>
</html>
"#;
        let result = extract_search_results(html, "serde::Serialize");

        // Should extract search results correctly
        assert!(result.contains("Documentation"));
        assert!(result.contains("serde::Serialize"));
        assert!(result.contains("Serialize trait"));

        // Should remove navigation
        assert!(!result.contains("Sidebar"));
    }

    /// Test that multiple skip tags are handled efficiently
    #[test]
    fn test_clean_html_multiple_skip_tags() {
        let html = r"
<html>
<head>
    <style>.test { color: red; }</style>
    <script>var x = 1;</script>
</head>
<body>
    <nav>Navigation</nav>
    <article>
        <h1>Title</h1>
        <p>Content with <script>inline script</script> removed.</p>
        <footer>Article footer</footer>
    </article>
    <footer>Page footer</footer>
</body>
</html>
";
        let cleaned = clean_html(html);

        // Should preserve content
        assert!(cleaned.contains("Title"));
        assert!(cleaned.contains("Content"));

        // Should remove all unwanted elements
        assert!(!cleaned.contains("style"), "Should remove style tags");
        assert!(!cleaned.contains("script"), "Should remove script tags");
        assert!(!cleaned.contains("Navigation"), "Should remove nav");
        assert!(!cleaned.contains("footer"), "Should remove footer");
        assert!(!cleaned.contains(".test"), "Should remove CSS content");
        assert!(!cleaned.contains("var x"), "Should remove JS content");
    }

    /// Test that cached selectors work correctly for all tag types
    #[test]
    fn test_cached_selectors_all_tag_types() {
        // Test each tag type defined in constants
        let test_cases = [
            (
                "<script>alert('test')</script><p>Content</p>",
                "script",
                "Content",
            ),
            ("<style>.x{}</style><p>Content</p>", "style", "Content"),
            (
                "<noscript>Enable JS</noscript><p>Content</p>",
                "noscript",
                "Content",
            ),
            (
                "<iframe src=\"x\"></iframe><p>Content</p>",
                "iframe",
                "Content",
            ),
            ("<nav><a>Link</a></nav><p>Content</p>", "nav", "Content"),
            ("<header>Head</header><p>Content</p>", "header", "Content"),
            ("<footer>Foot</footer><p>Content</p>", "footer", "Content"),
            ("<aside>Sidebar</aside><p>Content</p>", "aside", "Content"),
            ("<button>Click</button><p>Content</p>", "button", "Content"),
        ];

        for (html, tag_to_remove, expected_content) in test_cases {
            let cleaned = clean_html(html);
            assert!(
                !cleaned.contains(tag_to_remove),
                "Should remove {tag_to_remove} tag"
            );
            assert!(
                cleaned.contains(expected_content),
                "Should preserve {expected_content}"
            );
        }
    }
}
