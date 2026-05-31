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

/// Block-level tags. During plain-text extraction a [`BLOCK_SEP`] marker is
/// inserted around these so adjacent blocks (e.g. consecutive `<li>`/`<dt>`
/// item-index entries, table cells, or paragraphs) do not run together into a
/// single token like `Dl_infoElf32_Chdr`, and so each block can be emitted on
/// its own line. Inline tags are intentionally excluded so that runs split
/// across inline elements (`ser`+`<wbr>`+`ializing`, `RandomState</a>,`) are not
/// corrupted with spurious spaces.
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
    "tfoot",
    "thead",
    "tr",
    "ul",
];

/// Sentinel marker inserted around block-level elements during plain-text
/// extraction (see [`BLOCK_TAGS`]). It is deliberately distinct from any
/// whitespace so genuine block boundaries can be turned into newlines without
/// being confused with the incidental whitespace inside text nodes (including
/// source-indentation newlines), which is collapsed to single spaces. A NUL
/// byte never appears in rendered documentation text: the HTML parser replaces
/// any literal NUL in the input with U+FFFD.
const BLOCK_SEP: &str = "\u{0}";

/// Sentinel marker inserted around table cells (`<td>`/`<th>`) during plain-text
/// extraction. Unlike [`BLOCK_SEP`] (which becomes a newline), `CELL_SEP` keeps
/// a table row's cells on a single line, joined by ` | `, so the row's
/// columns stay associated (e.g. `%C | 20 | The proleptic Gregorian year ...`).
/// U+0001 never appears in rendered documentation text, so it is a safe
/// sentinel (cf. [`BLOCK_SEP`]).
const CELL_SEP: &str = "\u{1}";

/// Sentinel characters used to preserve the verbatim whitespace of `<pre>`
/// code blocks through the whitespace-collapsing passes. They are control
/// characters that Rust does not classify as whitespace, so they survive both
/// `str::split_whitespace` and `str::lines`. [`decode_pre`] restores the
/// original characters once all collapsing is complete.
const PRE_SPACE: char = '\u{2}';
const PRE_NEWLINE: char = '\u{3}';
const PRE_TAB: char = '\u{4}';

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

/// Regex to downgrade rustdoc *item-anchor* links to their plain-text label.
///
/// rustdoc cross-references items with fragment-only links whose id carries a
/// type-specific prefix (`#method.foo`, `#tymethod.foo`, `#variant.Foo`,
/// `#structfield.foo`, `#associatedtype.Error`, `#associatedconstant.MAX`,
/// `#reexport.foo`) or the impl-block form (`#impl-Trait-for-Type`). These
/// anchors only exist inside the rustdoc page; the rendered markdown has no
/// matching heading id, so the links are dead. Group 1 captures the label
/// (the item name) so it can be kept as text. Genuine in-page section anchors
/// (e.g. `[Quick start](#quick-start)`) lack these prefixes and are untouched.
static RUSTDOC_ITEM_ANCHOR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[([^\]]*)\]\(#(?:(?:method|tymethod|variant|structfield|associatedtype|associatedconstant|reexport)\.|impl-)[^)]*\)",
    )
    .expect("hardcoded valid regex pattern")
});

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

/// Regex to strip an orphaned trailing middot separator from a line.
///
/// rustdoc joins out-of-band metadata with ` \u{00b7} ` separators, e.g.
/// `1.0.0 \u{00b7} <source link>`. Once the trailing source/toggle link is
/// removed, the line keeps a dangling ` \u{00b7}` that carries no meaning
/// (e.g. the stability line becomes `1.0.0 \u{00b7}`). Drop the trailing
/// middot together with the whitespace (including non-breaking spaces) that
/// precedes it. Middots embedded in prose are unaffected because they are
/// followed by more text.
static TRAILING_MIDDOT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)[ \t\u{00a0}]*\u{00b7}[ \t\u{00a0}]*$").expect("hardcoded valid regex pattern")
});

/// Regex to trim trailing horizontal whitespace, including non-breaking spaces.
///
/// rustdoc headings and metadata rows frequently end with a stray space or
/// non-breaking space (`\u{00a0}`) that html2md preserves, leaving artifacts
/// like `Struct HashMap\u{00a0}` above a setext underline. Stripping trailing
/// whitespace per line removes the noise without affecting content.
static TRAILING_WS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)[ \t\u{00a0}]+$").expect("hardcoded valid regex pattern"));

/// Regex to strip the redundant closing hashes html2md appends to ATX
/// headings.
///
/// html2md 0.2.15 renders `<h3>`-`<h6>` as ATX headings with a trailing run of
/// closing hashes (e.g. `### Examples ###`, `#### pub fn get() ####`). Those
/// closing hashes are optional in `CommonMark` and read as noise, so we drop the
/// trailing ` #+` while keeping the leading marker. Group 1 captures the
/// heading text.
static HEADING_TRAILING_HASH_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(#{1,6}[ \t].*?)[ \t]+#+[ \t]*$").expect("hardcoded valid regex pattern")
});

/// Matches an HTML superscript element (`<sup>...</sup>`) left verbatim in the
/// markdown output.
///
/// `html2md` 0.2.15 has no handler for `<sup>`/`<sub>`, so rustdoc footnote
/// references and exponents (e.g. `<sup id="fnref1"><a href="#fn1">1</a></sup>`)
/// survive as literal HTML in the markdown. Group 1 captures the inner markup;
/// [`clean_markdown`] strips any nested tags and re-emits it as `^(...)`.
static SUPERSCRIPT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<sup\b[^>]*>(.*?)</sup\s*>").expect("hardcoded valid regex pattern")
});

/// Matches an HTML subscript element (`<sub>...</sub>`) left verbatim in the
/// markdown output. Counterpart to [`SUPERSCRIPT_REGEX`]; re-emitted as `_(...)`.
static SUBSCRIPT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<sub\b[^>]*>(.*?)</sub\s*>").expect("hardcoded valid regex pattern")
});

/// Matches a single HTML tag, used to strip residual inline markup (e.g. a
/// nested `<a>`) from the inner content of a super/subscript before re-emitting
/// it as plain text. See [`clean_markdown`].
static INLINE_TAG_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<[^>]+>").expect("hardcoded valid regex pattern"));

/// Matches a negative auto-trait impl heading whose linkified trait name is
/// glued to the leading `!`, e.g. `### impl<T> !Freeze for Mutex<T>`.
///
/// rustdoc emits the negative-impl marker as a text `!` immediately before the
/// trait link (`!<a class="trait" ...>Freeze</a>`). html2md fuses these into
/// `![Freeze](url)`, which is markdown image syntax and renders as a broken
/// embedded image instead of the text `!Freeze`. Group 1 captures the heading
/// prefix up to (and including) the `!`-glued bracket's `!`; [`clean_markdown`]
/// re-emits it with the `!` backslash-escaped so it stays literal. Scoped to
/// `impl` headings so genuine doc-body images are never touched.
static NEGATIVE_IMPL_TRAIT_IMAGE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(#{1,6} +impl\b[^\n]*?)!\[").expect("hardcoded valid regex pattern")
});

/// Regex to rewrite relative documentation links to their link text.
///
/// Matches `[text](path.html)` where `path` begins with a letter, digit, `_`,
/// `.` or `/` (covering module paths such as `_derive/index.html`,
/// `../index.html`, `struct.Foo.html`) and ends with `.html` (optionally
/// followed by a `#fragment`). Group 1 captures the link text and group 2 the
/// URL. The link text may contain one level of nested brackets (e.g. an
/// attribute label `#[tokio::main]` or a slice type `[u8]`).
/// Docs.rs-relative targets are useless to an MCP client, so they are
/// downgraded to their (meaningful) label; absolute external URLs containing a
/// scheme (`://`) are kept intact since they are still reachable.
static RELATIVE_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[((?:[^\[\]]|\[[^\]]*\])*)\]\(([a-zA-Z0-9._/][^)]*\.html(?:#[^)]*)?)\)")
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc "Read more" see-also affordance link (`[Read more](url)`).
///
/// rustdoc appends a `<a href="...">Read more</a>` link to the one-line summary
/// of every inherited/trait method (e.g. derived `Clone`/`Debug`/`Hash`). When
/// the target is a docs.rs-relative `.html` path it cannot be resolved by an
/// MCP client, and downgrading it to its label leaves a meaningless dangling
/// "Read more" at the end of the sentence. Group 1 captures any leading inline
/// whitespace and group 2 the URL, so a relative affordance can be dropped
/// entirely while an absolute (`scheme://`) one is preserved. See
/// [`clean_markdown`].
static READ_MORE_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"([ \t]*)\[Read more\]\(([^)]*)\)").expect("hardcoded valid regex pattern")
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

/// Matches a `<pre>...</pre>` block (verbatim code) so callers can leave its
/// significant whitespace untouched while transforming the surrounding markup.
static PRE_BLOCK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<pre\b.*?</pre\s*>").expect("hardcoded valid regex pattern")
});

/// Matches a whitespace run that contains a newline/tab/CR immediately before
/// an inline element's opening tag.
///
/// `html2md` 0.2.15 drops such leading whitespace before inline elements like
/// `<a>`, `<em>` and `<strong>`, gluing the element onto the preceding word
/// (e.g. a word, a newline, then an `<a>` link wraps an inline-code span and
/// renders glued to the word after relative-link downgrading). A *single*
/// literal space is preserved correctly by `html2md`, so these runs are
/// collapsed to one space. The pattern only matches runs containing a
/// newline/tab/CR, so genuine single spaces and deliberately glued cases such
/// as a hyphen directly followed by `<code>` (no whitespace at all) are left
/// untouched.
static INLINE_LEADING_WS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)[ \t\r\n]*[\r\n\t][ \t\r\n]*(<(?:a|code|em|strong|b|i|span|sup|sub|abbr|kbd|var|cite|q|mark|small|u)\b)",
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches a whitespace run containing a newline/tab/CR immediately *after* an
/// inline element's closing tag, when followed by word-like content.
///
/// Symmetric to [`INLINE_LEADING_WS_REGEX`]: `html2md` 0.2.15 also drops such
/// trailing whitespace, gluing the next word onto the element (e.g.
/// `</a>` followed by a newline and `crate` renders as `[..](..)crate`, which
/// becomes `..crate` after relative-link downgrading). The trailing lookahead
/// restricts the fix to alphanumeric/backtick/bracket/open-paren starts so a line wrapped
/// before trailing punctuation (`</a>\n.`) is left untouched. A single literal
/// space is already preserved by `html2md`, so only newline-bearing runs match.
static INLINE_TRAILING_WS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(</(?:a|code|em|strong|b|i|span|sup|sub|abbr|kbd|var|cite|q|mark|small|u)>)[ \t\r\n]*[\r\n\t][ \t\r\n]*(?P<n>[A-Za-z0-9`\[(])",
    )
    .expect("hardcoded valid regex pattern")
});

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
    // Match both modern (`class="src"`, double-quoted) and older rustdoc
    // (`class='srclink'`, single-quoted) source-code anchors so their `[src]`
    // label never leaks into the plain-text output (which, unlike the markdown
    // path, has no later link-stripping pass).
    Regex::new(r#"(?s)<a\b[^>]*\bclass\s*=\s*['"][^'"]*\bsrc(?:link)?\b[^'"]*['"][^>]*>.*?</a>"#)
        .expect("hardcoded valid regex pattern")
});

/// Regex to fix the orphan `\u{00b7}` separator left between a stability
/// "since" badge and its now-removed source link.
///
/// rustdoc emits `<span class="since">1.0.0</span> \u{00b7} <a class="src">Source</a>`
/// inside an item's right-side metadata. [`SRC_ANCHOR_HTML_REGEX`] deletes the
/// source anchor, leaving ` \u{00b7} </span>`. When the enclosing `<summary>` is
/// later flattened to text the dangling middot glues onto the following
/// signature (`1.0.0 \u{00b7} fn next(...)`). Collapse the separator (and its
/// surrounding whitespace) to a single space while preserving the closing tag,
/// so the version stays cleanly separated from the signature.
static ORPHAN_SINCE_MIDDOT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)[ \t\u{00a0}]*\u{00b7}[ \t\u{00a0}]*(</span\s*>)")
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc stability "since" version badge
/// (`<span class="since ...">1.0.0</span>`) that is immediately followed by
/// another tag with no separating whitespace.
///
/// In a flattened `<summary>` (provided trait methods on FFI structs, e.g.
/// libc) the badge abuts the method code-header, so plain-text extraction
/// fuses them (`1.0.0fn clone_from`). Group 1 captures the whole badge; the
/// trailing `<` (re-emitted by the replacement) ensures a space is inserted
/// only when the badge is glued, never doubling an existing space. The version
/// text holds no nested tags, so `[^<]*` captures it safely. See [`clean_html`].
static SINCE_BADGE_GLUED_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)(<span\b[^>]*\bclass\s*=\s*["'][^"']*\bsince\b[^"']*["'][^>]*>[^<]*</span\s*>)<"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Regex to remove rustdoc UI anchor links that carry no documentation value.
///
/// rustdoc decorates headings, item declarations and code examples with
/// navigation affordances rendered as `<a>` elements:
/// - section/anchor links `<a class="anchor">\u{00a7}</a>` (a section-sign that
///   jumps to the heading),
/// - notable-trait markers `<a class="tooltip" data-notable-ty="...">\u{24d8}</a>`
///   (a circled-i tooltip toggle), and
/// - "Run code" buttons `<a class="test-arrow" href="https://play.rust-lang.org/...">`
///   with empty link text (the playground launcher for a doc example), and
/// - scraped-example help links `<a class="scrape-help" href="...">?</a>` (the
///   `?` affordance beside an "Examples found in repository" heading).
///
/// The glyph anchors commonly sit inside a `<summary>` whose text is otherwise
/// preserved, so removing them at the DOM level is too late (the glyph would
/// survive as plain text and glue onto the following declaration, e.g.
/// `\u{00a7}impl<...>` or `Keys<'_, K, V> \u{24d8}`). The run buttons otherwise
/// render as an empty-text markdown link wrapping a very long playground URL
/// (`[](https://play.rust-lang.org/?code=...)`). Stripping all three from the
/// raw HTML keeps them out of both the markdown and plain-text output.
static UI_ANCHOR_HTML_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)<a\b[^>]*\bclass\s*=\s*['"][^'"]*\b(?:anchor|tooltip|test-arrow|scrape-help)\b[^'"]*['"][^>]*>.*?</a>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Regex to remove rustdoc UI anchors whose target is a `javascript:` URL
/// (collapse/expand toggles such as `#toggle-all-docs`, which render as a
/// bracketed minus/plus marker).
///
/// These are pure UI affordances; documentation never legitimately links to a
/// `javascript:` URL. Their visible marker text would otherwise leak into the
/// plain-text output, since the `javascript:`-link cleanup only runs on the
/// markdown path.
static JS_ANCHOR_HTML_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*['"]\s*javascript:[^>]*>.*?</a>"#)
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

/// Regex to remove rustdoc UI web-components from raw HTML before parsing.
///
/// Modern rustdoc emits custom elements for its chrome: `<rustdoc-toolbar>`
/// (the settings/options toolbar, rendered empty in static HTML) and
/// `<rustdoc-topbar>` (a duplicate breadcrumb such as
/// `<h2><a href="#">Iterator</a></h2>`). The toolbar sits inside
/// `#main-content`, so it leaks into the `html` output as a stray empty tag;
/// the topbar can leak a redundant heading. Neither carries documentation
/// value, so both are stripped (paired and self-closing forms).
static RUSTDOC_UI_ELEMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)<rustdoc-(?:toolbar|topbar)\b[^>]*>.*?</rustdoc-(?:toolbar|topbar)\s*>|<rustdoc-(?:toolbar|topbar)\b[^>]*/>",
    )
    .expect("hardcoded valid regex pattern")
});

/// Regex to remove the rustdoc navigation breadcrumb element.
///
/// rustdoc renders a breadcrumb above each item title, e.g.
/// `<div class="rustdoc-breadcrumbs"><a href="../index.html">std</a>::<wbr>`
/// `<a href="index.html">vec</a></div>`. Its links are page-relative, so they
/// are downgraded to bare text and leave a dangling line such as `std::vec`
/// (or a lone `std` on macro pages) directly under our own
/// `## Documentation: <path>` title. The breadcrumb is pure navigation chrome
/// that duplicates the title, so the whole element is removed before parsing.
/// It contains only anchors and separators (no nested `<div>`), so the
/// non-greedy match terminates at the first `</div>`.
static RUSTDOC_BREADCRUMBS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<div\b[^>]*\brustdoc-breadcrumbs\b[^>]*>.*?</div\s*>")
        .expect("hardcoded valid regex pattern")
});

/// Regex matching a rustdoc prose admonition rendered as a styled `<pre>`.
///
/// rustdoc/mdBook authors create "Warning"/"Note" callout boxes with the idiom
/// `<pre class="compile_fail" style="white-space:normal;font:inherit;">` (or
/// with `class="ignore"`) wrapping ordinary prose HTML such as a paragraph with
/// a bold "Warning" lead-in. The `white-space:normal;font:inherit` style makes
/// rustdoc
/// render it as flowing prose rather than monospaced code. Without special
/// handling our pipeline treats the `<pre>` as a code block and wraps the prose
/// in a bare fenced code block (mislabeling prose as code and flattening its
/// inline links and code). Genuine code examples keep the default `white-space: pre`, so
/// matching on `white-space:normal` reliably selects only these prose boxes.
/// They are rewritten to a `<blockquote>` so the inner prose renders normally
/// as a callout in every output format. The box holds no nested `<pre>`, so the
/// non-greedy body terminates at the first `</pre>`.
static PROSE_PRE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<pre\b[^>]*\bstyle\s*=\s*["'][^"']*white-space\s*:\s*normal[^"']*["'][^>]*>(.*?)</pre\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Regex matching rustdoc's "unsafe function" marker superscript.
///
/// In module item lists rustdoc appends `<sup title="unsafe function">WARN</sup>`
/// (the `WARN` glyph is a warning emoji) after each unsafe function's name. Our
/// superscript handling would otherwise turn it into a `^(...)` token glued onto
/// the name (e.g. `copy^(...)`). The marker conveys a useful fact, so it is
/// replaced with a readable ` (unsafe)` annotation in every output format before
/// parsing.
static UNSAFE_FN_MARKER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<sup\b[^>]*\btitle\s*=\s*["']unsafe function["'][^>]*>.*?</sup\s*>"#)
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc collapse-toggle `<summary class="hideme">` element.
///
/// rustdoc places interactive "Show N methods"/"Show N associated items" and
/// "Expand description" toggles inside `<summary class="hideme">` nodes, and the
/// "Show N methods" one sits *inside* the item-declaration `<pre>` block. Its
/// label text therefore leaks into the rendered code (e.g.
/// `Show 76 methods    // Required method` inside a trait signature) in every
/// output format. The element is pure UI chrome, so it is removed wholesale
/// (the surrounding `<details>` content is preserved). See [`clean_html`].
static HIDEME_SUMMARY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<summary\b[^>]*\bclass\s*=\s*["'][^"']*\bhideme\b[^"']*["'][^>]*>.*?</summary\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc impl-block documentation `<div class="docblock">` that is
/// the final child of an impl `<section>` nested inside a `<summary>`.
///
/// rustdoc renders an impl block's own documentation (e.g. a "Basic API"
/// heading) as `<div class="docblock">...</div>` *inside* the `<summary>` that
/// also holds the `impl ...` declaration. Because [`remove_unwanted_elements`]
/// flattens `<summary>` nodes to their decoded text, that docblock glues onto
/// the declaration (e.g. `impl ArgBasic API`). Group 1 captures the docblock
/// contents so the wrapper can be relocated *after* the `</summary>`, where it
/// renders as ordinary content. The trailing `</div></section></summary>`
/// boundary only occurs for impl-block docs (method/field docblocks sit after
/// their `</summary>`), so this does not disturb other documentation. See
/// [`clean_html`].
static IMPL_DOCBLOCK_IN_SUMMARY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)</h3>\s*<div class="docblock">(.*?)</div>\s*</section>\s*</summary>"#)
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc portability/feature-availability badge that carries a
/// human-readable `title` attribute.
///
/// rustdoc renders availability pills as
/// `<span class="stab portability" title="Available on crate feature `fs` only">`
/// `<code>fs</code></span>` immediately after an item link, with no separating
/// whitespace. Group 1 captures the title text, which is the clearest rendering
/// (it also covers platform/cfg badges such as "Available on `docsrs` and Unix
/// only"). See [`rewrite_portability_badges`].
static STAB_PORTABILITY_TITLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<span\b[^>]*\bclass\s*=\s*["'][^"']*\bportability\b[^"']*["'][^>]*\btitle\s*=\s*"([^"]*)"[^>]*>.*?</span\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc portability badge that lacks a usable `title` attribute.
/// Group 1 captures the inner markup (the feature name(s)). Fallback for the
/// title-based [`STAB_PORTABILITY_TITLE_REGEX`].
static STAB_PORTABILITY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<span\b[^>]*\bclass\s*=\s*["'][^"']*\bportability\b[^"']*["'][^>]*>(.*?)</span\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches an inline rustdoc stability badge span (e.g.
/// `<span class="stab unstable" title="">Experimental</span>` or a
/// `<span class="stab deprecated">Deprecated</span>` pill) that rustdoc renders
/// immediately after an item name with no separating whitespace, gluing the
/// badge label onto the name (e.g. `TryReserveErrorKindExperimental`).
///
/// Group 1 captures the inner label. Portability badges (`class="stab
/// portability"`) are handled earlier by [`rewrite_portability_badges`] and so
/// are already consumed before this runs; only the remaining stab pills match.
/// The pattern is span-scoped, so block-level stability banners
/// (`<div class="stab unstable">...</div>`) on item-detail pages are untouched.
/// See [`rewrite_stab_badges`].
static STAB_BADGE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<span\b[^>]*\bclass\s*=\s*["'][^"']*\bstab\b[^"']*["'][^>]*>(.*?)</span\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches the opening tag of a rustdoc item-info wrapper
/// (`<span class="item-info">`), which holds the stability/deprecation badges
/// that rustdoc renders immediately after an item signature.
///
/// rustdoc emits the wrapper with no separating whitespace after the preceding
/// `</section>` (e.g. `...&str</h4></section><span class="item-info"><div
/// class="stab deprecated"><span class="emoji">\u{1f44e}</span>...`). When the
/// enclosing collapsed `<summary>` is flattened to text, the badge glues onto
/// the signature (`-> &str\u{1f44e} Deprecated since ...`). Group 1 captures the
/// opening tag so [`clean_html`] can re-emit it preceded by a single space.
static ITEM_INFO_OPEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)(<span\b[^>]*\bclass\s*=\s*["'][^"']*\bitem-info\b[^"']*["'][^>]*>)"#)
        .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc decorative emoji badge such as the nightly-API flask.
///
/// rustdoc renders unstable/experimental markers as
/// `<span class="emoji">\u{1f52c}</span><span>This is a nightly-only ...</span>`
/// with no separating whitespace, so html2md glues the emoji onto the following
/// text (`\u{1f52c}This is a nightly-only experimental API.`). Group 1 captures
/// the whole badge; [`rewrite_emoji_badges`] re-emits it followed by a single
/// space so the emoji reads as a separate visual cue.
static EMOJI_SPAN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)(<span\b[^>]*\bclass\s*=\s*["'][^"']*\bemoji\b[^"']*["'][^>]*>.*?</span\s*>)"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc struct-field declaration span
/// (`<span class="structfield section-header">field: Type</span>`).
///
/// rustdoc emits one such span per field with no separating whitespace and
/// relies on CSS to render each as its own block. Without intervention the
/// adjacent spans glue together: markdown yields back-to-back inline code
/// spans, and the plain-text path fuses a field type onto the next field
/// name into a corrupt token. The captured inner content is re-wrapped in a
/// block element so each field renders on its own line. Group 1 is the field
/// declaration. See [`clean_html`].
static STRUCTFIELD_SPAN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<span\b[^>]*\bclass\s*=\s*["'][^"']*\bstructfield\b[^"']*["'][^>]*>(.*?)</span\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches a rustdoc `where`-clause block (`<div class="where">where ...</div>`)
/// embedded in item declarations and signatures.
///
/// rustdoc relies on CSS to render this block on its own line(s); the markup
/// itself carries no line break before the block or after it, so both html2md
/// and the plain-text extractor glue it onto the surrounding tokens (e.g.
/// `Vec<T, A = Global>where` and `Allocator,{`). Group `w` captures the inner
/// content. See [`rewrite_where_clauses`].
static WHERE_DIV_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div\b[^>]*\bclass\s*=\s*["'][^"']*\bwhere\b[^"']*["'][^>]*>(?P<w>.*?)</div\s*>"#,
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

/// Matches a rustdoc `<div class="code-attribute">` element. rustdoc wraps each
/// attribute (e.g. `#[repr(i8)]`, `#[non_exhaustive]`) shown above an item
/// declaration in this block-level `<div>`, which CSS renders on its own line.
/// Group 1 captures the inner attribute markup. See
/// [`rewrite_code_attributes`].
static CODE_ATTRIBUTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div\b[^>]*\bclass\s*=\s*["'][^"']*\bcode-attribute\b[^"']*["'][^>]*>(.*?)</div\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Put each item-declaration attribute on its own line.
///
/// rustdoc renders declaration attributes inside `<div class="code-attribute">`
/// blocks within the `<pre class="item-decl">` signature. Because the `<div>`
/// only breaks the line via CSS, extracting the `<pre>` text glues the
/// attribute onto the following declaration (e.g. `#[repr(i8)]pub enum
/// Ordering`) in every format. Replace each such `<div>` with its inner content
/// followed by a newline so the attribute keeps its own line; the result
/// renders identically to rustdoc in all three output formats.
#[must_use]
fn rewrite_code_attributes(html: &str) -> String {
    CODE_ATTRIBUTE_REGEX
        .replace_all(html, "${1}\n")
        .into_owned()
}

/// Matches a rustdoc code-header element (`<h3>`/`<h4 class="code-header">`),
/// which holds an item/impl/method signature. Group 1 is the heading level
/// digit (matched again at the close tag) and group 2 the inner markup. See
/// [`rewrite_code_headers`].
static CODE_HEADER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<h([34])\b[^>]*\bclass\s*=\s*["'][^"']*\bcode-header\b[^"']*["'][^>]*>(.*?)</h[34]\s*>"#,
    )
    .expect("hardcoded valid regex pattern")
});

/// Matches `(` followed by a newline and indentation (rustdoc's wrapped-argument
/// list opener). See [`rewrite_code_headers`].
static SIG_OPEN_PAREN_WRAP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(\s*\n\s*").expect("hardcoded valid regex pattern"));

/// Matches an optional trailing comma plus a newline before the closing `)` of a
/// wrapped argument list. See [`rewrite_code_headers`].
static SIG_CLOSE_PAREN_WRAP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r",?\s*\n\s*\)").expect("hardcoded valid regex pattern"));

/// Matches any remaining newline-with-whitespace run inside a signature. See
/// [`rewrite_code_headers`].
static SIG_NEWLINE_RUN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\n\s*").expect("hardcoded valid regex pattern"));

/// Collapse multi-line rustdoc signatures in code-header elements onto a single
/// line.
///
/// rustdoc wraps long `fn`/method signatures across several lines using literal
/// newlines and indentation inside the (non-`<pre>`) `<h4 class="code-header">`
/// element, e.g. `try_lock_owned(\n    self: Arc<Self>,\n) -> ...`. html2md
/// renders such a header as an ATX heading, so the embedded newlines split it
/// into a broken two-line heading; the plain-text path collapses them but keeps
/// stray spaces (`( self: Arc<Self>, )`). Normalise the wrapped argument list
/// back to a single clean line (`(self: Arc<Self>) -> ...`) before parsing.
/// Only code-header elements are touched, so `<pre>` code examples (which may
/// legitimately contain `(\n    `) are unaffected.
fn rewrite_code_headers(html: &str) -> String {
    CODE_HEADER_REGEX
        .replace_all(html, |caps: &regex::Captures| {
            let level = &caps[1];
            let inner = &caps[2];
            // Impl headers (`<h3>`) stay headings, but item-signature headers
            // (`<h4>`: methods, associated consts/types) render as plain text.
            // rustdoc only wraps *documented* items in `<details><summary>`
            // (whose `<h4>` is flattened to text); an *undocumented* sibling is
            // a bare `<section>` whose `<h4>` would otherwise survive as a
            // spurious `####` heading, inconsistent with its documented peers.
            // See test_undocumented_assoc_item_not_rendered_as_heading.
            let (open, close) = if level == "4" {
                (r#"<p class="code-header">"#.to_string(), "</p>".to_string())
            } else {
                (
                    format!("<h{level} class=\"code-header\">"),
                    format!("</h{level}>"),
                )
            };
            if !inner.contains('\n') {
                return format!("{open}{inner}{close}");
            }
            let inner = SIG_OPEN_PAREN_WRAP_REGEX.replace_all(inner, "(");
            let inner = SIG_CLOSE_PAREN_WRAP_REGEX.replace_all(&inner, ")");
            let inner = SIG_NEWLINE_RUN_REGEX.replace_all(&inner, " ");
            format!("{open}{inner}{close}")
        })
        .into_owned()
}

/// Detach rustdoc `where`-clause blocks from the surrounding declaration.
///
/// rustdoc emits `<div class="where">` with no literal line breaks around it
/// (the layout is CSS-only), so item declarations render glued, e.g.
/// `Vec<T, A = Global>where` and `Allocator,{ /* private fields */ }`. Inside
/// `<pre>` declarations the clause is wrapped in newlines to reproduce the
/// multi-line rustdoc layout; elsewhere (single-line code-header signatures) it
/// is collapsed to a single space-padded clause so the heading stays on one
/// line. `<pre>` boundaries are detected with [`PRE_BLOCK_REGEX`].
fn rewrite_where_clauses(html: &str) -> String {
    let collapse = |caps: &regex::Captures| -> String {
        let inner = caps.name("w").map_or("", |m| m.as_str());
        format!(
            " {} ",
            inner.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    };
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for m in PRE_BLOCK_REGEX.find_iter(html) {
        // Outside <pre>: collapse the clause onto one space-padded line.
        out.push_str(&WHERE_DIV_REGEX.replace_all(&html[last..m.start()], &collapse));
        // Inside <pre>: keep the clause verbatim but break it onto its own lines.
        out.push_str(&WHERE_DIV_REGEX.replace_all(m.as_str(), "\n${w}\n"));
        last = m.end();
    }
    out.push_str(&WHERE_DIV_REGEX.replace_all(&html[last..], &collapse));
    out
}

/// Rewrite rustdoc portability/feature badges so they are not glued onto the
/// preceding item name.
///
/// Each `<span class="stab portability">` is replaced by a space-separated
/// parenthetical: the badge's human-readable `title` when present (e.g. the
/// "Available on crate feature ... only" string), otherwise its inner content.
/// This stops html2md from gluing the feature pill onto the item name, so it
/// reads naturally in both markdown and plain-text formats.
fn rewrite_portability_badges(html: &str) -> String {
    let with_titles = STAB_PORTABILITY_TITLE_REGEX.replace_all(html, |caps: &regex::Captures| {
        format!(" ({})", badge_title_to_html(&caps[1]))
    });
    STAB_PORTABILITY_REGEX
        .replace_all(&with_titles, " (${1})")
        .into_owned()
}

/// Convert a badge `title` string into HTML, turning backtick-delimited
/// segments into genuine `<code>` elements.
///
/// rustdoc availability titles embed the feature name in literal backticks
/// (e.g. ``Available on crate feature `thread_rng` only``). Splicing that text
/// in verbatim makes html2md treat the backticks as plain characters: it then
/// escapes any markdown metacharacter inside them (e.g. the underscore in
/// `thread_rng`), leaking a stray backslash inside what looks like a code span
/// (`` `thread\_rng` ``). Emitting a real `<code>` element instead yields a
/// proper code span in markdown (no escaping) and correct markup in the html
/// output. Backticks are only treated as delimiters when balanced; an odd
/// count leaves the title untouched.
#[must_use]
fn badge_title_to_html(title: &str) -> String {
    let parts: Vec<&str> = title.split('`').collect();
    // An even number of segments means an odd number of backticks (unbalanced);
    // leave the title as-is rather than emit a dangling `<code>`.
    if parts.len().is_multiple_of(2) {
        return title.to_string();
    }
    let mut out = String::with_capacity(title.len() + 13);
    for (i, part) in parts.iter().enumerate() {
        if i % 2 == 1 {
            out.push_str("<code>");
            out.push_str(part);
            out.push_str("</code>");
        } else {
            out.push_str(part);
        }
    }
    out
}

/// Rewrite remaining inline rustdoc stability badges so their label is not
/// glued onto the preceding item name.
///
/// Each leftover `<span class="stab ...">` pill (e.g. the `Experimental` or
/// `Deprecated` marker that follows an item link in a module index table) is
/// replaced by a space-separated parenthetical built from its label text. Run
/// *after* [`rewrite_portability_badges`] so feature/availability pills have
/// already been consumed and only stability markers remain.
#[must_use]
fn rewrite_stab_badges(html: &str) -> String {
    STAB_BADGE_REGEX.replace_all(html, " (${1})").into_owned()
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
    // After the source link is gone, collapse the orphan `\u{00b7}` separator
    // that rustdoc left between the "since" badge and that link (see
    // ORPHAN_SINCE_MIDDOT_REGEX) so it cannot glue onto the next signature.
    let html = ORPHAN_SINCE_MIDDOT_REGEX.replace_all(&html, " ${1}");
    // Drop `javascript:` UI toggles (e.g. the bracketed collapse-all control)
    // so their marker text does not survive plain-text extraction.
    let html = JS_ANCHOR_HTML_REGEX.replace_all(&html, "");
    // Strip rustdoc UI anchors (section-sign/notable-trait glyphs and the
    // playground "Run code" buttons) before parsing so they do not survive as
    // plain text or as empty-text links (see UI_ANCHOR_HTML_REGEX).
    let html = UI_ANCHOR_HTML_REGEX.replace_all(&html, "");
    // Separate a "since" version badge from a directly-following element so a
    // flattened <summary> does not fuse it onto the next signature
    // (`1.0.0fn clone_from`). See SINCE_BADGE_GLUED_REGEX.
    let html = SINCE_BADGE_GLUED_REGEX.replace_all(&html, "${1} <");
    // Guarantee removal of executable/style/embedded content regardless of how
    // the source markup was formatted (see DANGEROUS_ELEMENT_REGEX docs).
    let html = DANGEROUS_ELEMENT_REGEX.replace_all(&html, "");
    // Strip rustdoc UI web-components (toolbar/topbar chrome) so they do not
    // leak into the html output or as a redundant heading (see
    // RUSTDOC_UI_ELEMENT_REGEX).
    let html = RUSTDOC_UI_ELEMENT_REGEX.replace_all(&html, "");
    // Remove the rustdoc navigation breadcrumb above the item title; its
    // page-relative links would otherwise be downgraded to a dangling bare
    // line (e.g. `std::vec`, or a lone `std` on macro pages) that merely
    // duplicates our own title (see RUSTDOC_BREADCRUMBS_REGEX).
    let html = RUSTDOC_BREADCRUMBS_REGEX.replace_all(&html, "");
    // Rewrite rustdoc prose admonitions ("Warning"/"Note" callouts authored as
    // `<pre style="white-space:normal;...">`) into blockquotes so their prose
    // renders normally instead of being mislabeled as a bare ``` code block
    // (see PROSE_PRE_REGEX). Genuine code examples are untouched.
    let html = PROSE_PRE_REGEX.replace_all(&html, "<blockquote>${1}</blockquote>");
    // Replace rustdoc's "unsafe function" marker superscript with a readable
    // ` (unsafe)` annotation; otherwise it leaks as `^(...)` glued onto the
    // function name in module item lists (see UNSAFE_FN_MARKER_REGEX).
    let html = UNSAFE_FN_MARKER_REGEX.replace_all(&html, " (unsafe)");
    // Remove rustdoc "Show N methods"/"Expand description" collapse
    // toggles (`<summary class="hideme">`); the "Show N methods" toggle
    // sits inside the item-declaration <pre>, so its label otherwise
    // leaks into the rendered signature (see HIDEME_SUMMARY_REGEX).
    let html = HIDEME_SUMMARY_REGEX.replace_all(&html, "");
    // Detach `where` clauses (CSS-only line breaks) so declarations do not
    // render glued (e.g. `Vec<T, A = Global>where`).
    let html = rewrite_where_clauses(&html);
    // Collapse multi-line wrapped signatures in code-header elements onto a
    // single clean line so html2md does not emit a broken two-line heading
    // (see rewrite_code_headers).
    let html = rewrite_code_headers(&html);
    // Put each item-declaration attribute (e.g. `#[repr(i8)]`) on its own line
    // so it is not glued onto the following declaration (see
    // rewrite_code_attributes).
    let html = rewrite_code_attributes(&html);
    // Separate feature/portability badges from the preceding item name so they
    // do not render glued (e.g. `fs`fs``); replace each with a readable
    // parenthetical built from the badge's title (or inner) text.
    let html = rewrite_portability_badges(&html);
    // Separate remaining inline stability pills (e.g. `Experimental`/`Deprecated`
    // markers in module index tables) from the preceding item name so they do
    // not render glued (see rewrite_stab_badges). Runs after the portability
    // rewrite so feature/availability pills are already consumed.
    let html = rewrite_stab_badges(&html);
    // Append a space after decorative emoji badges (e.g. the nightly-API flask)
    // so the emoji does not glue onto the following text (see EMOJI_SPAN_REGEX).
    let html = EMOJI_SPAN_REGEX.replace_all(&html, "${1} ");
    // Separate the item-info badge wrapper (stability/deprecation pills) from a
    // preceding signature so a flattened `<summary>` does not glue the badge
    // onto the declaration (e.g. `-> &str\u{1f44e} Deprecated`). See
    // ITEM_INFO_OPEN_REGEX.
    let html = ITEM_INFO_OPEN_REGEX.replace_all(&html, " ${1}");
    // Rewrite rustdoc item-index tables into <ul><li> lists so html2md does not
    // concatenate every item name onto a single line (overview pages only).
    let html = rewrite_item_tables(&html);
    // Put each struct-field declaration on its own block so adjacent fields
    // do not glue together (`a: A``b: B` in markdown, `A_tb` token fusion in
    // text). See STRUCTFIELD_SPAN_REGEX.
    let html = STRUCTFIELD_SPAN_REGEX.replace_all(&html, "<p>${1}</p>");
    // Relocate an impl block's own documentation out of the flattened
    // `<summary>` so its heading/text does not glue onto the `impl ...`
    // declaration (e.g. `impl ArgBasic API`). See
    // IMPL_DOCBLOCK_IN_SUMMARY_REGEX.
    let html = IMPL_DOCBLOCK_IN_SUMMARY_REGEX.replace_all(
        &html,
        r#"</h3></section></summary><div class="docblock">${1}</div>"#,
    );
    let document = Html::parse_document(&html);
    remove_unwanted_elements(&document, &html)
}

/// HTML-escape the special characters `&`, `<`, and `>` in plain text.
///
/// Used when decoded text (from `ElementRef::text()`) is spliced back into an
/// HTML string that will be parsed again downstream (e.g. by `html2md`). Without
/// re-escaping, fragments such as `Option<usize>` would be misread as tags and
/// silently dropped. `&` is escaped first so the replacement is idempotent for a
/// single pass.
#[must_use]
fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
        // For summary tags, extract and keep the text content. `text()` returns
        // *decoded* text, so generic markup such as `Option&lt;usize&gt;`
        // becomes literal `Option<usize>`. This string is later re-parsed by
        // `html2md`/`scraper`, which would treat `<usize>` as an unknown tag and
        // drop it; re-escape the markup so it survives the second parse.
        let text_content: String = element.text().collect();
        replacements.push((element_html, Some(escape_html_text(&text_content))));
    }

    // If no replacements needed, just apply regex patterns
    if replacements.is_empty() {
        return apply_regex_patterns(original_html);
    }

    // Sort by length descending (longer first) to avoid partial replacements
    // This ensures we replace parent elements before children
    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

    // Build result using string slices for O(n) total complexity.
    //
    // Use the parsed document's own serialized form (the body's inner HTML) as
    // the replacement base rather than `original_html`. Each `element.html()`
    // is produced by the same html5ever serializer, so it is guaranteed to be a
    // substring here. Matching against the raw `original_html` instead would
    // miss elements whose source formatting differs from the serialized form
    // (e.g. extra whitespace inside a tag like `<nav  class=...>` or differing
    // attribute quoting), silently leaking navigation, headers, footers and
    // asides into the cleaned output. The body's inner HTML keeps the prior
    // fragment shape (no synthetic `<html>`/`<head>` wrappers).
    let mut result = document
        .select(&BODY_SELECTOR)
        .next()
        .map_or_else(|| document.root_element().html(), |body| body.inner_html());
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
    decode_pre(&html_to_text_raw(html))
}

/// Like [`html_to_text`] but leaves `<pre>` content encoded with the
/// [`PRE_SPACE`]/[`PRE_NEWLINE`]/[`PRE_TAB`] sentinels. Callers that run
/// additional whitespace-normalisation passes (e.g.
/// [`extract_documentation_as_text`]) use this and call [`decode_pre`]
/// themselves once all collapsing is done.
fn html_to_text_raw(html: &str) -> String {
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
    // surrounding whitespace, and `collapse_block_whitespace` collapses runs.
    // Inserting a space between every node would corrupt inline runs split
    // across elements. `BLOCK_SEP` markers added around block elements become
    // newlines so the output keeps document structure.
    collapse_block_whitespace(&text_parts.join(""))
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
                    let name = child_ref.value().name().to_lowercase();
                    // Preserve the verbatim formatting of `<pre>` code blocks.
                    // Their newlines and indentation would otherwise be flattened
                    // by the whitespace-collapsing passes, rendering multi-line
                    // code examples as a single unreadable line. Encode the
                    // significant whitespace as control sentinels that survive
                    // collapsing; `decode_pre` restores it at the very end.
                    if name == "pre" {
                        let raw = child_ref.text().collect::<String>();
                        text_parts.push(BLOCK_SEP.to_string());
                        text_parts.push(encode_pre(raw.trim_matches('\n')));
                        text_parts.push(BLOCK_SEP.to_string());
                        continue;
                    }
                    // Render superscript/subscript (e.g. footnote references) as
                    // plain-text `^(...)`/`_(...)` notation so a bare `1` is not
                    // mistaken for body text. Matches the markdown path's handling.
                    if name == "sup" || name == "sub" {
                        let mut inner_parts = Vec::new();
                        extract_text_excluding_skip_tags(&child_ref, &mut inner_parts);
                        let inner = inner_parts
                            .join("")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !inner.is_empty() {
                            let (open, close) = if name == "sup" {
                                ("^(", ")")
                            } else {
                                ("_(", ")")
                            };
                            text_parts.push(format!("{open}{inner}{close}"));
                        }
                        continue;
                    }
                    // Surround block-level elements with a `BLOCK_SEP`
                    // marker so adjacent blocks do not glue together (e.g.
                    // item-index entries) and each renders on its own line.
                    // `collapse_block_whitespace` turns the markers into
                    // newlines. Inline elements get no separator to preserve
                    // intra-word runs.
                    // Table cells use a CELL_SEP marker (rendered as ` | `) so a
                    // row's columns stay on one line; every other block element
                    // uses BLOCK_SEP (rendered as a newline).
                    let is_cell = name == "td" || name == "th";
                    let is_block = !is_cell && BLOCK_TAGS.contains(&name.as_str());
                    let sep = if is_cell { CELL_SEP } else { BLOCK_SEP };
                    if is_cell || is_block {
                        text_parts.push(sep.to_string());
                    }
                    extract_text_excluding_skip_tags(&child_ref, text_parts);
                    // A cell pushes only a *leading* CELL_SEP delimiter; a block
                    // is wrapped on both sides. This keeps a single separator
                    // between adjacent cells so empty cells can be preserved
                    // (see collapse_block_whitespace) and columns stay aligned.
                    if is_block {
                        text_parts.push(sep.to_string());
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

/// Matches an inline `<code>...</code>` element (non-greedy). Used by
/// [`flatten_links_in_inline_code`] to drop anchor wrappers that markdown
/// cannot render inside a code span.
static INLINE_CODE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<code\b[^>]*>.*?</code\s*>").expect("valid regex"));

/// Matches an opening or closing `<a>` anchor tag. Used to strip link wrappers
/// while keeping their text. See [`flatten_links_in_inline_code`].
static ANCHOR_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)</?a\b[^>]*>").expect("valid regex"));

/// Flatten `<a>` links nested inside an inline `<code>` element to their text
/// (markdown path only).
///
/// rustdoc renders re-exports as `<code>pub use <a href=...>name</a>;</code>`.
/// html2md turns the inner anchor into a markdown link *inside* the backtick
/// code span (`` `pub use [name](url);` ``), which renders as literal text
/// because markdown does not support links inside inline code. Removing the
/// anchor wrapper (keeping its text) yields a clean `` `pub use name;` `` code
/// span. `<pre>` blocks are skipped so code-example formatting/links are left
/// untouched; the html output format never calls this, so its links survive.
#[must_use]
fn flatten_links_in_inline_code(html: &str) -> String {
    let strip = |segment: &str| -> String {
        INLINE_CODE_REGEX
            .replace_all(segment, |caps: &regex::Captures| {
                ANCHOR_TAG_REGEX.replace_all(&caps[0], "").into_owned()
            })
            .into_owned()
    };
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for m in PRE_BLOCK_REGEX.find_iter(html) {
        out.push_str(&strip(&html[last..m.start()]));
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&strip(&html[last..]));
    out
}

/// Matches a `<pre ...>` opening tag (group 1 = its attributes) plus an
/// optional immediately-following `<code ...>` open tag. Used by
/// [`inject_code_fence_language`] to attach the detected language to the code
/// block's opening fence.
static PRE_LANG_OPEN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<pre\b([^>]*)>(\s*<code\b[^>]*>)?").expect("valid regex"));

/// Matches a `class="..."` attribute value (group 1). See
/// [`detect_pre_language`].
static PRE_CLASS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)class\s*=\s*["']([^"']*)["']"#).expect("valid regex"));

/// Sentinel wrapping a code-fence language hint while it travels through
/// `html2md` inside the code block (STX bytes are never present in docs text).
const CODE_FENCE_SENTINEL: char = '\u{2}';

/// Determine the syntax-highlighting language for a rustdoc `<pre>` block from
/// its class attribute. rustdoc marks Rust examples with the `rust` class
/// (`rust rust-example-rendered`) and other fenced languages with
/// `language-<name>` (e.g. `language-toml`). Returns `None` when no language can
/// be determined so the fence stays bare.
#[must_use]
fn detect_pre_language(pre_attrs: &str) -> Option<String> {
    let class = PRE_CLASS_REGEX.captures(pre_attrs)?.get(1)?.as_str();
    for tok in class.split_whitespace() {
        if let Some(lang) = tok.strip_prefix("language-") {
            if !lang.is_empty() {
                return Some(lang.to_string());
            }
        }
    }
    if class.split_whitespace().any(|t| t == "rust") {
        return Some("rust".to_string());
    }
    None
}

/// Attach the detected language to each `<pre>` code block (markdown path only).
///
/// `html2md` 0.2.15 drops all `<pre>`/`<code>` class information and always
/// emits a bare ```` ``` ```` fence, losing rustdoc's language annotation
/// (`rust`, `toml`, ...). To preserve it, prepend a sentinel-wrapped language
/// token as the first line of the block's content; it survives `html2md`
/// verbatim and is converted into a fence info string by
/// [`restore_code_fence_language`]. Blocks without a detectable language are
/// left untouched.
#[must_use]
fn inject_code_fence_language(html: &str) -> String {
    PRE_LANG_OPEN_REGEX
        .replace_all(html, |caps: &regex::Captures| {
            let whole = &caps[0];
            match detect_pre_language(&caps[1]) {
                Some(lang) => {
                    format!("{whole}{CODE_FENCE_SENTINEL}{lang}{CODE_FENCE_SENTINEL}\n")
                }
                None => whole.to_string(),
            }
        })
        .into_owned()
}

/// Collapse newline-containing whitespace on either side of inline elements to
/// a single space, leaving `<pre>` blocks untouched.
///
/// Works around an `html2md` 0.2.15 quirk where whitespace adjacent to an
/// inline element (e.g. `the\n<a>...` or `...</a>\ncrate`) is dropped, gluing
/// the element onto the neighbouring word. `<pre>` code blocks are skipped so
/// their significant indentation and line breaks (which often wrap highlighted
/// `<a>`/`<span>` tokens) are preserved verbatim. See [`INLINE_LEADING_WS_REGEX`]
/// and [`INLINE_TRAILING_WS_REGEX`].
fn normalize_inline_leading_whitespace(html: &str) -> String {
    // Collapse a newline-bearing whitespace run on either side of an inline
    // element to a single space (html2md drops both). Applied only outside
    // <pre> blocks so code indentation/line breaks are preserved.
    let fix = |segment: &str| -> String {
        let leading = INLINE_LEADING_WS_REGEX.replace_all(segment, " $1");
        INLINE_TRAILING_WS_REGEX
            .replace_all(&leading, "$1 $n")
            .into_owned()
    };
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for m in PRE_BLOCK_REGEX.find_iter(html) {
        // Transform the segment before this <pre> block.
        out.push_str(&fix(&html[last..m.start()]));
        // Emit the <pre> block verbatim.
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&fix(&html[last..]));
    out
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
    // Flatten links nested inside inline <code> (e.g. re-exports) so they do
    // not become unrenderable markdown links inside a backtick span.
    let cleaned_html = flatten_links_in_inline_code(&cleaned_html);
    // Preserve rustdoc code-block language hints (html2md drops class info);
    // see inject_code_fence_language / restore_code_fence_language.
    let cleaned_html = inject_code_fence_language(&cleaned_html);
    // Restore whitespace html2md would otherwise drop before inline elements.
    let cleaned_html = normalize_inline_leading_whitespace(&cleaned_html);
    let markdown = html2md::parse_html(&cleaned_html);

    // Post-process markdown to remove unwanted links
    clean_markdown(&markdown)
}

/// Reverse the backslash escaping that html2md applies to ordinary text.
///
/// html2md 0.2.15 escapes the markdown metacharacters ``< > * _ ~ \`` in every
/// non-code text node. Because this output is consumed as documentation rather
/// than re-rendered as markdown, those escapes are pure noise (e.g.
/// `serde\_json`, `Vec\<u8\>`, `-\>`). This pass removes the escaping outside of
/// code, while leaving fenced code blocks and inline code spans untouched
/// (html2md never escapes code, so any backslash there is genuine).
fn unescape_markdown(markdown: &str) -> String {
    const ESCAPED: [char; 6] = ['<', '>', '*', '_', '~', '\\'];
    let mut out = String::with_capacity(markdown.len());
    let mut in_fence = false;
    for line in markdown.split_inclusive('\n') {
        // Fenced code blocks are delimited by a line whose first non-whitespace
        // characters are three backticks; emit them verbatim and skip unescaping
        // their contents.
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }

        // Inline pass: toggle in/out of code on each maximal backtick run so
        // single- and multi-backtick spans are both preserved verbatim.
        let chars: Vec<char> = line.chars().collect();
        let mut in_code = false;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                let start = i;
                while i < chars.len() && chars[i] == '`' {
                    i += 1;
                }
                for _ in start..i {
                    out.push('`');
                }
                in_code = !in_code;
                continue;
            }
            if c == '\\' && !in_code && i + 1 < chars.len() && ESCAPED.contains(&chars[i + 1]) {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Matches an opening code fence followed by a sentinel-wrapped language line
/// (see [`inject_code_fence_language`]). Group 1 is the fence (with any
/// indentation), group 2 the language token. See [`restore_code_fence_language`].
static CODE_FENCE_SENTINEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^([ \t]*`{3,})[ \t]*\r?\n[ \t]*\x02([^\x02\r\n]*)\x02[ \t]*\r?\n")
        .expect("valid regex")
});

/// Matches any leftover language sentinel (a code block whose fence was not
/// matched, e.g. an empty block). See [`restore_code_fence_language`].
static ORPHAN_FENCE_SENTINEL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x02[^\x02\n]*\x02\n?").expect("valid regex"));

/// Convert the language sentinel emitted by [`inject_code_fence_language`] into
/// a markdown fence info string (e.g. ```` ```rust ````), then strip any
/// orphaned sentinels. Runs in the markdown post-processing pass.
#[must_use]
fn restore_code_fence_language(markdown: &str) -> String {
    let with_lang = CODE_FENCE_SENTINEL_REGEX.replace_all(markdown, "${1}${2}\n");
    ORPHAN_FENCE_SENTINEL_REGEX
        .replace_all(&with_lang, "")
        .into_owned()
}

/// Clean markdown output by removing relative links and UI artifacts
#[inline]
fn clean_markdown(markdown: &str) -> String {
    // Use Cow to avoid allocations when no replacements are needed
    // Chain replacements to process in a single traversal
    // Restore code-fence language hints carried through html2md as sentinels
    // (see restore_code_fence_language) before any other processing.
    let markdown = restore_code_fence_language(markdown);
    // First strip html2md's backslash escaping from non-code text so escaped
    // identifiers/generics (`serde\_json`, `Vec\<u8\>`) read naturally.
    let unescaped = unescape_markdown(&markdown);
    // html2md leaves `<sup>`/`<sub>` as raw HTML (e.g. footnote references in
    // tables). Convert them to plain-text `^(...)`/`_(...)` notation, stripping
    // any nested tags (such as a footnote `<a>` link) from the inner content.
    let unescaped = SUPERSCRIPT_REGEX.replace_all(&unescaped, |caps: &regex::Captures| {
        let inner = INLINE_TAG_STRIP_REGEX.replace_all(&caps[1], "");
        let inner = inner.trim();
        if inner.is_empty() {
            String::new()
        } else {
            format!("^({inner})")
        }
    });
    let unescaped = SUBSCRIPT_REGEX.replace_all(&unescaped, |caps: &regex::Captures| {
        let inner = INLINE_TAG_STRIP_REGEX.replace_all(&caps[1], "");
        let inner = inner.trim();
        if inner.is_empty() {
            String::new()
        } else {
            format!("_({inner})")
        }
    });
    // Escape the negative-impl marker `!` that html2md fused onto a linkified
    // trait name (`![Freeze](url)`) so it renders as literal `!Freeze` text and
    // not a broken markdown image. See NEGATIVE_IMPL_TRAIT_IMAGE_REGEX.
    let unescaped = NEGATIVE_IMPL_TRAIT_IMAGE_REGEX.replace_all(&unescaped, r"${1}\![");
    // Remove UI/source/javascript links first, then relative and section
    // anchors. Empty- and fragment-only links are downgraded to their text so
    // useful labels (e.g. headings) survive.
    let result = JS_TOGGLE_REGEX.replace_all(&unescaped, Cow::Borrowed(""));
    let result = JS_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = SOURCE_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = SRC_LINK_REGEX.replace_all(&result, Cow::Borrowed(""));
    // Drop a "Read more" see-also affordance whose target is a docs.rs-relative
    // `.html` path (it would otherwise be downgraded below to a meaningless
    // dangling "Read more"); keep absolute (`scheme://`) ones, which remain
    // reachable. See READ_MORE_LINK_REGEX.
    let result = READ_MORE_LINK_REGEX.replace_all(&result, |caps: &regex::Captures| {
        let ws = &caps[1];
        let url = &caps[2];
        if url.contains("://") {
            format!("{ws}[Read more]({url})")
        } else {
            String::new()
        }
    });
    let result = RELATIVE_LINK_REGEX.replace_all(&result, |caps: &regex::Captures| {
        let text = &caps[1];
        let url = &caps[2];
        // Keep absolute external links (those carrying a `scheme://`); only
        // docs.rs-relative `.html` targets are downgraded to their label.
        if url.contains("://") {
            format!("[{text}]({url})")
        } else {
            text.to_string()
        }
    });
    // Downgrade dead rustdoc item-anchor links (`#method.X`,
    // `#associatedtype.X`, `#impl-...`) to their label; the rendered
    // markdown has no matching heading id, so the links go nowhere.
    let result = RUSTDOC_ITEM_ANCHOR_REGEX.replace_all(&result, Cow::Borrowed("$1"));
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
    let result = TRAILING_MIDDOT_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = TRAILING_WS_REGEX.replace_all(&result, Cow::Borrowed(""));
    let result = HEADING_TRAILING_HASH_REGEX.replace_all(&result, Cow::Borrowed("$1"));
    // html2md pads blockquotes with empty `>` lines (e.g. a clap note renders
    // as `>\n>\n> text\n>\n>`); drop the noisy boundary/duplicate marker lines.
    let result = tidy_blockquotes(&result);
    let result = MULTIPLE_NEWLINES_REGEX.replace_all(&result, Cow::Borrowed("\n\n"));
    result.trim().to_string()
}

/// Remove the empty `>` marker lines `html2md` emits around blockquote content.
///
/// `html2md` 0.2.15 renders `<blockquote><p>x</p></blockquote>` as
/// `>\n>\n> x\n>\n>` (leading/trailing empty quote lines plus duplicates).
/// Within each maximal run of consecutive blockquote lines (those whose first
/// non-space character is `>`), leading and trailing empty quote lines are
/// dropped and internal runs of empty quote lines are collapsed to a single one
/// (preserving genuine paragraph breaks inside a multi-paragraph quote).
/// A quote line is "empty" when it contains only `>` and whitespace characters.
#[must_use]
fn tidy_blockquotes(markdown: &str) -> String {
    let is_quote = |l: &str| l.trim_start().starts_with('>');
    let is_empty_quote = |l: &str| is_quote(l) && l.chars().all(|c| c == '>' || c.is_whitespace());

    let lines: Vec<&str> = markdown.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !is_quote(lines[i]) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        // Gather a maximal run of consecutive blockquote lines.
        let start = i;
        while i < lines.len() && is_quote(lines[i]) {
            i += 1;
        }
        let block = &lines[start..i];
        // Find the first and last non-empty quote line in the block.
        let first = block.iter().position(|l| !is_empty_quote(l));
        let last = block.iter().rposition(|l| !is_empty_quote(l));
        if let (Some(first), Some(last)) = (first, last) {
            let mut prev_empty = false;
            for line in &block[first..=last] {
                let empty = is_empty_quote(line);
                if empty && prev_empty {
                    continue; // collapse consecutive internal empty quote lines
                }
                out.push((*line).to_string());
                prev_empty = empty;
            }
        }
        // A block of only empty quote lines is dropped entirely.
    }
    out.join("\n")
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
    // Flatten links nested inside inline <code> (e.g. re-exports) so they do
    // not become unrenderable markdown links inside a backtick span.
    let cleaned_html = flatten_links_in_inline_code(&cleaned_html);
    // Preserve rustdoc code-block language hints (html2md drops class info);
    // see inject_code_fence_language / restore_code_fence_language.
    let cleaned_html = inject_code_fence_language(&cleaned_html);
    // Restore whitespace html2md would otherwise drop before inline elements.
    let cleaned_html = normalize_inline_leading_whitespace(&cleaned_html);
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
    // Use the raw extraction so `<pre>` content stays encoded through the
    // line-normalisation pass; decode it back to real whitespace at the end.
    let text = html_to_text_raw(&cleaned_html);
    // Drop standalone section-sign markers, then re-collapse each line so the
    // newline-delimited block structure from `html_to_text_raw` is preserved.
    let normalized = normalize_lines(&text.replace('\u{00a7}', " "));
    // Strip the dangling middot separator left on out-of-band rows (e.g. the
    // stability line `1.0.0 \u{00b7}`) once the trailing source link is gone.
    let normalized = TRAILING_MIDDOT_REGEX.replace_all(&normalized, "");
    strip_trailing_line_whitespace(&decode_pre(&normalized))
}

/// Collapse whitespace within each block segment and join blocks with newlines.
///
/// [`BLOCK_SEP`] markers delimit block-level boundaries. Within each segment all
/// whitespace runs (spaces, tabs, and incidental source newlines) collapse to a
/// single space, which preserves inline runs split across elements. Empty
/// segments are dropped so adjacent markers do not emit blank lines.
#[inline]
fn collapse_block_whitespace(text: &str) -> String {
    text.split(BLOCK_SEP)
        .map(|seg| {
            // Within a block segment, table cells are separated by CELL_SEP.
            // Each cell carries a single *leading* CELL_SEP delimiter, so the
            // fragment before the first delimiter is empty and is dropped; the
            // remaining cells (including genuinely empty ones, e.g. a blank
            // row-label header) are kept so columns stay aligned. Segments
            // without a CELL_SEP (the common case) collapse unchanged.
            if seg.contains(CELL_SEP) {
                let mut cells: Vec<String> = seg
                    .split(CELL_SEP)
                    .map(|cell| cell.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect();
                if cells.first().is_some_and(String::is_empty) {
                    cells.remove(0);
                }
                // Drop pure visual-spacer rows (every cell empty) so they do
                // not render as content-free `| |` noise between data rows.
                // Rows with any content keep their (possibly empty) cells so
                // columns stay aligned.
                if cells.iter().all(String::is_empty) {
                    String::new()
                } else {
                    cells.join(" | ")
                }
            } else {
                seg.split_whitespace().collect::<Vec<_>>().join(" ")
            }
        })
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse intra-line whitespace and drop blank lines while preserving the
/// newline-delimited block structure produced by [`html_to_text`].
#[inline]
fn normalize_lines(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip trailing whitespace from every line of finalised text output.
///
/// Signatures rendered inside an encoded `<pre>` can carry a trailing space
/// (e.g. `-> StepBy<Self> ` immediately before a wrapped `where` clause): the
/// space is held as a [`PRE_SPACE`] sentinel, so it survives [`normalize_lines`]
/// and is only restored to a real space by [`decode_pre`]. A final per-line
/// `trim_end` removes such dangling whitespace without touching indentation.
#[inline]
fn strip_trailing_line_whitespace(text: &str) -> String {
    text.split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[inline]
fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Encode the significant whitespace of `<pre>` content as control sentinels
/// ([`PRE_SPACE`], [`PRE_NEWLINE`], [`PRE_TAB`]) so it survives the
/// whitespace-collapsing passes. Carriage returns are dropped.
fn encode_pre(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            ' ' => out.push(PRE_SPACE),
            '\n' => out.push(PRE_NEWLINE),
            '\t' => out.push(PRE_TAB),
            '\r' => {}
            other => out.push(other),
        }
    }
    out
}

/// Reverse of [`encode_pre`]: restore the original whitespace characters from
/// the [`PRE_SPACE`]/[`PRE_NEWLINE`]/[`PRE_TAB`] sentinels.
fn decode_pre(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            PRE_SPACE => out.push(' '),
            PRE_NEWLINE => out.push('\n'),
            PRE_TAB => out.push('\t'),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_strips_old_rustdoc_src_and_toggle_anchors() {
        // Older rustdoc heading markup: a `javascript:` collapse-all toggle and a
        // single-quoted `srclink` source anchor. Neither must leak its bracketed
        // marker into the plain-text output.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<h1>Crate serde",
            "<a id=\"toggle-all-docs\" href=\"javascript:void(0)\" title=\"collapse all docs\">",
            "[<span class='inner'>TOGGLEMARK</span>]</a>",
            "<a class='srclink' href='../src/serde/lib.rs.html#9-267' title='goto source code'>[src]</a>",
            "</h1><p>Real doc.</p>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(!text.contains("[src]"), "src link leaked: {text:?}");
        assert!(!text.contains("TOGGLEMARK"), "toggle leaked: {text:?}");
        assert!(text.contains("Crate serde"), "heading dropped: {text:?}");
        assert!(text.contains("Real doc."), "content dropped: {text:?}");
    }

    #[test]
    fn test_markdown_strips_trailing_heading_hashes() {
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<h3>Examples</h3>",
            "<h4>pub fn get(&amp;self)</h4>",
            "<p>Body text.</p>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(md.contains("### Examples"), "h3 missing: {md:?}");
        assert!(!md.contains("Examples ###"), "trailing hashes left: {md:?}");
        assert!(md.contains("#### pub fn get(&self)"), "h4 missing: {md:?}");
        assert!(!md.contains(") ####"), "trailing hashes left: {md:?}");
    }

    #[test]
    fn test_markdown_restores_space_before_inline_link() {
        // html2md drops newline whitespace before inline <a>, gluing the link
        // onto the preceding word. The HashMap docs trigger this with
        // "using the\n<a>...<code>default</code>...".
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<p>replaced on a per-<code>HashMap</code> basis using the\n",
            "<a href=\"trait.Default.html#tymethod.default\"><code>default</code></a>, ",
            "<a href=\"struct.HashMap.html#method.with_hasher\"><code>with_hasher</code></a> methods.</p>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // The space before the (downgraded) link is restored.
        assert!(
            md.contains("using the `default`"),
            "missing space before inline link: {md:?}"
        );
        // The deliberately glued `per-<code>HashMap</code>` (no source
        // whitespace) stays glued.
        assert!(
            md.contains("per-`HashMap`"),
            "spurious space inserted into hyphenated code: {md:?}"
        );
    }

    #[test]
    fn test_where_clause_detached_from_declaration() {
        // rustdoc's <div class="where"> has no literal line breaks, so the
        // declaration renders glued ("Global>where", "Allocator,{").
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre class=\"rust item-decl\"><code>pub struct Vec&lt;T, A = ",
            "<a class=\"struct\" href=\"struct.Global.html\">Global</a>&gt;",
            "<div class=\"where\">where\n    A: <a class=\"trait\" href=\"trait.Allocator.html\">Allocator</a>,</div>",
            "{ <span class=\"comment\">/* private fields */</span> }</code></pre>",
            "<h4 class=\"code-header\">pub fn retain&lt;F&gt;(&amp;mut self, f: F)",
            "<div class=\"where\">where\n    F: <a class=\"trait\" href=\"trait.FnMut.html\">FnMut</a>,</div></h4>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // Inside the code block the clause breaks onto its own lines.
        assert!(
            md.contains("Global>\nwhere") && md.contains("Allocator,\n{"),
            "where clause not broken in code block: {md:?}"
        );
        assert!(!md.contains("Global>where"), "glued where survived: {md:?}");
        // In the single-line method header the clause is space-separated.
        assert!(
            md.contains("f: F) where F:"),
            "where not separated in header: {md:?}"
        );

        // Plain-text format gets the same multi-line declaration.
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("Global>\nwhere"),
            "where clause not broken in text: {text:?}"
        );
        assert!(
            !text.contains("Global>where"),
            "glued where in text: {text:?}"
        );
    }

    #[test]
    fn test_text_signature_has_no_trailing_whitespace_before_where() {
        // A signature inside a `<pre>` ends with a space immediately before the
        // `where` div; that space is held as a PRE_SPACE sentinel and survives
        // line normalisation, so `decode_pre` would otherwise restore a dangling
        // trailing space on the signature line in text output.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre class=\"rust item-decl\"><code>fn step_by(self, step: usize) -> ",
            "StepBy&lt;Self&gt; ",
            "<div class=\"where\">where\n    Self: Sized,</div>",
            "</code></pre>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            !text.lines().any(|l| l.ends_with(' ') || l.ends_with('\t')),
            "text output has a line with trailing whitespace: {text:?}"
        );
        // The signature and the wrapped clause are still present and split.
        assert!(
            text.contains("-> StepBy<Self>") && text.contains("where"),
            "signature/where content lost: {text:?}"
        );
    }

    #[test]
    fn test_ui_glyph_anchors_stripped() {
        // rustdoc decorates impl/method headers (inside <summary>) with a
        // section-sign anchor `<a class="anchor">\u{00a7}</a>` and a
        // notable-trait marker `<a class="tooltip">\u{24d8}</a>`. Both are pure
        // UI affordances and must not leak into markdown or text output.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle implementors-toggle\" open><summary>",
            "<section id=\"impl-Clone\" class=\"impl\">",
            "<a href=\"#impl-Clone\" class=\"anchor\">\u{00a7}</a>",
            "<h3 class=\"code-header\">impl Clone for Foo</h3></section></summary></details>",
            "<details class=\"toggle method-toggle\" open><summary>",
            "<section id=\"method.keys\" class=\"method\">",
            "<h4 class=\"code-header\">fn <a href=\"#method.keys\" class=\"fn\">keys</a>(&amp;self) -&gt; ",
            "<a class=\"struct\" href=\"struct.Keys.html\">Keys</a> ",
            "<a href=\"#\" class=\"tooltip\" data-notable-ty=\"Keys\">\u{24d8}</a></h4>",
            "</section></summary></details>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            !md.contains('\u{00a7}'),
            "section-sign anchor leaked into markdown: {md:?}"
        );
        assert!(
            !md.contains('\u{24d8}'),
            "notable-trait marker leaked into markdown: {md:?}"
        );
        assert!(
            md.contains("impl Clone for Foo"),
            "impl header lost: {md:?}"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            !text.contains('\u{00a7}') && !text.contains('\u{24d8}'),
            "UI glyph leaked into text: {text:?}"
        );
    }

    #[test]
    fn test_scrape_help_question_mark_anchor_stripped() {
        // rustdoc adds a `<a class="scrape-help" href="...">?</a>` help link
        // beside the "Examples found in repository" heading of a scraped
        // example. It is pure UI chrome and its `?` glyph must not leak into
        // the rendered output (the heading text itself is preserved).
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"docblock scraped-example-list\"><span></span>",
            "<h5 id=\"scraped-examples\">",
            "<a href=\"#scraped-examples\">Examples found in repository</a>",
            "<a class=\"scrape-help\" href=\"../scrape-examples-help.html\">?</a>",
            "</h5></div>",
            "</section></body></html>"
        );
        for out in [
            extract_documentation(html),
            extract_documentation_as_text(html),
            extract_documentation_html(html),
        ] {
            assert!(
                !out.contains("?</a>") && !out.contains("scrape-help"),
                "scrape-help link leaked: {out:?}"
            );
            assert!(
                out.contains("Examples found in repository"),
                "scraped-example heading text lost: {out:?}"
            );
        }
        // The leaked `?` glyph must not survive as a trailing token in markdown.
        let md = extract_documentation(html);
        assert!(
            !md.contains("repository)?") && !md.contains("repository ?"),
            "stray scrape-help `?` leaked into markdown: {md:?}"
        );
    }

    #[test]
    fn test_multiline_signature_collapsed_to_single_line() {
        // rustdoc wraps long signatures across lines inside the (non-<pre>)
        // <h4 class="code-header"> using literal newlines + indentation. That
        // otherwise yields a broken two-line markdown heading and stray spaces
        // in text (`( self: Arc<Self>, )`).
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<section id=\"method.try_lock_owned\" class=\"method\">",
            "<h4 class=\"code-header\">pub fn <a href=\"#m\" class=\"fn\">try_lock_owned</a>(\n",
            "    self: <a class=\"struct\" href=\"struct.Arc.html\">Arc</a>&lt;Self&gt;,\n",
            ") -&gt; <a class=\"enum\" href=\"enum.Result.html\">Result</a>&lt;T&gt;</h4></section>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("(self: Arc<Self>) -> Result<T>"),
            "multi-line signature not collapsed cleanly (markdown): {md:?}"
        );
        assert!(
            !md.contains("( self") && !md.contains(", )") && !md.contains(",\n)"),
            "signature spacing artifacts survived: {md:?}"
        );
        // The collapsed heading stays on a single line.
        assert!(
            !md.contains("Result<T>\n)") && !md.contains(",\n)"),
            "heading split across lines: {md:?}"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("try_lock_owned(self: Arc<Self>) -> Result<T>"),
            "multi-line signature not collapsed cleanly (text): {text:?}"
        );
    }

    #[test]
    fn test_rustdoc_ui_web_components_stripped() {
        // rustdoc emits <rustdoc-toolbar> (inside #main-content, rendered empty)
        // and <rustdoc-topbar> (a duplicate breadcrumb heading). Neither should
        // leak into the html output.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"main-heading\"><h1>Struct Foo</h1>",
            "<rustdoc-toolbar></rustdoc-toolbar></div>",
            "<rustdoc-topbar><h2><a href=\"#\">Foo</a></h2></rustdoc-topbar>",
            "<p>Body.</p>",
            "</section></body></html>"
        );
        let out = extract_documentation_html(html);
        assert!(
            !out.contains("rustdoc-toolbar") && !out.contains("rustdoc-topbar"),
            "rustdoc UI web-component leaked into html: {out:?}"
        );
        assert!(out.contains("Body."), "body content lost: {out:?}");
    }

    #[test]
    fn test_rustdoc_breadcrumbs_stripped() {
        // rustdoc renders a navigation breadcrumb above the item title. Its
        // links are page-relative, so without removal they leave a dangling
        // bare line (`std::vec`, or a lone `std` on macro pages) that merely
        // duplicates our own title. The whole element must be stripped in all
        // three formats.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"main-heading\">",
            "<div class=\"rustdoc-breadcrumbs\"><a href=\"../index.html\">std</a>",
            "::<wbr><a href=\"index.html\">vec</a></div>",
            "<h1>Struct Vec</h1></div>",
            "<p>A contiguous growable array type.</p>",
            "</section></body></html>"
        );
        for out in [
            extract_documentation(html),
            extract_documentation_as_text(html),
            extract_documentation_html(html),
        ] {
            assert!(
                !out.contains("rustdoc-breadcrumbs"),
                "breadcrumb element leaked: {out:?}"
            );
            assert!(
                !out.contains("std::vec"),
                "dangling breadcrumb line leaked: {out:?}"
            );
            assert!(
                out.contains("Vec") && out.contains("contiguous growable"),
                "real content lost: {out:?}"
            );
        }
    }

    #[test]
    fn test_prose_admonition_pre_becomes_blockquote_not_code() {
        // rustdoc renders "Warning"/"Note" callouts as a prose-styled <pre>
        // (white-space:normal); it must become a blockquote, not a bare code
        // fence, so the prose (and its inline code/links) renders correctly.
        let html = concat!(
            "<section id=\"main-content\">",
            "<p>Intro.</p>",
            "<div class=\"example-wrap\"><pre class=\"compile_fail\" ",
            "style=\"white-space:normal;font:inherit;\">",
            "<p><strong>Warning</strong>: Do not hold <code>Span::enter</code> ",
            "across an await point.</p></pre></div>",
            "<p>Outro.</p>",
            "</section>"
        );
        let md = extract_documentation(html);
        assert!(
            !md.contains("```"),
            "prose admonition rendered as code fence in markdown: {md:?}"
        );
        assert!(
            md.contains("> ") && md.contains("Warning"),
            "admonition not rendered as blockquote: {md:?}"
        );
        assert!(
            md.contains("`Span::enter`"),
            "inline code lost in admonition: {md:?}"
        );
        let html_out = extract_documentation_html(html);
        assert!(
            !html_out.contains("white-space:normal"),
            "prose pre survived in html output: {html_out:?}"
        );
        // A genuine code example (default white-space) must stay a code block.
        let code_html = concat!(
            "<section id=\"main-content\">",
            "<pre class=\"rust rust-example-rendered\"><code>let x = 1;</code></pre>",
            "</section>"
        );
        assert!(
            extract_documentation(code_html).contains("```"),
            "genuine code example lost its fence"
        );
    }

    #[test]
    fn test_unsafe_function_marker_rendered_as_annotation() {
        // rustdoc marks unsafe fns in module lists with
        // `<sup title="unsafe function">WARN</sup>`; it must become a readable
        // ` (unsafe)` annotation, not a `^(...)` superscript glued to the name.
        let html = concat!(
            "<section id=\"main-content\"><dl class=\"item-table\">",
            "<dt><a class=\"fn\" href=\"fn.copy.html\">copy</a>",
            "<sup title=\"unsafe function\">\u{26a0}</sup></dt>",
            "<dd>Copies bytes.</dd></dl></section>"
        );
        for out in [
            extract_documentation(html),
            extract_documentation_as_text(html),
            extract_documentation_html(html),
        ] {
            assert!(
                !out.contains('\u{26a0}'),
                "unsafe marker glyph leaked: {out:?}"
            );
            assert!(
                !out.contains("^("),
                "unsafe marker rendered as superscript: {out:?}"
            );
            assert!(
                out.contains("(unsafe)"),
                "unsafe annotation missing: {out:?}"
            );
        }
    }

    #[test]
    fn test_hideme_show_methods_toggle_stripped() {
        // rustdoc puts a "Show N methods" collapse toggle
        // (`<summary class="hideme">`) *inside* the trait declaration <pre>;
        // its label must not leak into the rendered signature in any format.
        // The surrounding details content (the method list) must survive.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre class=\"rust item-decl\"><code>pub trait Iterator {\n",
            "    type Item;\n",
            "<details class=\"toggle type-contents-toggle\">",
            "<summary class=\"hideme\"><span>Show 76 methods</span></summary>",
            "    // Required method\n",
            "    fn next(&amp;mut self) -&gt; Option&lt;Self::Item&gt;;\n",
            "</details>}</code></pre>",
            "</section></body></html>"
        );
        for out in [
            extract_documentation(html),
            extract_documentation_as_text(html),
            extract_documentation_html(html),
        ] {
            assert!(
                !out.contains("Show 76 methods"),
                "collapse toggle label leaked: {out:?}"
            );
            assert!(
                out.contains("// Required method"),
                "details content lost: {out:?}"
            );
        }
    }

    #[test]
    fn test_impl_block_docblock_not_glued_to_declaration() {
        // rustdoc nests an impl block's own documentation
        // (`<div class="docblock">`) inside the `<summary>` that holds the
        // `impl ...` declaration. When the summary is flattened to text the
        // docblock heading otherwise glues onto the declaration
        // (e.g. `impl ArgBasic API`). It must be relocated so the declaration
        // stays clean and the docblock renders as its own content.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div id=\"implementations-list\">",
            "<details class=\"toggle implementors-toggle\" open><summary>",
            "<section id=\"impl-Arg\" class=\"impl\">",
            "<h3 class=\"code-header\">impl Arg</h3>",
            "<div class=\"docblock\"><h4 id=\"basic-api\">Basic API</h4></div>",
            "</section></summary>",
            "<div class=\"impl-items\"><details class=\"toggle method-toggle\" open>",
            "<summary><section id=\"method.new\" class=\"method\">",
            "<h4 class=\"code-header\">pub fn new() -&gt; Arg</h4></section></summary>",
            "<div class=\"docblock\"><p>Create a new Arg.</p></div></details></div>",
            "</details></div>",
            "</section></body></html>"
        );
        for out in [
            extract_documentation(html),
            extract_documentation_as_text(html),
            extract_documentation_html(html),
        ] {
            assert!(
                !out.contains("ArgBasic API"),
                "impl declaration glued to docblock heading: {out:?}"
            );
            assert!(
                out.contains("Basic API"),
                "impl-block docblock heading lost: {out:?}"
            );
        }
    }

    #[test]
    fn test_undocumented_assoc_item_not_rendered_as_heading() {
        // rustdoc wraps a *documented* associated item in
        // `<details><summary>...</summary><docblock></details>` (the signature
        // is flattened to plain text), but an *undocumented* sibling is a bare
        // `<section>` whose `<h4 class="code-header">` would otherwise survive
        // as a spurious `####` heading. Both must render as plain text so the
        // list is consistent.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle\" open><summary>",
            "<section id=\"associatedconstant.DOC\" class=\"associatedconstant\">",
            "<h4 class=\"code-header\">pub const DOC: Self</h4></section></summary>",
            "<div class=\"docblock\"><p>Documented constant.</p></div></details>",
            "<section id=\"associatedconstant.BARE\" class=\"associatedconstant\">",
            "<h4 class=\"code-header\">pub const BARE: Self</h4></section>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            !md.contains("#### pub const BARE"),
            "undocumented assoc const rendered as a heading: {md:?}"
        );
        assert!(
            md.contains("pub const BARE: Self"),
            "undocumented assoc const signature lost: {md:?}"
        );
        assert!(
            md.contains("pub const DOC: Self") && md.contains("Documented constant."),
            "documented assoc const rendering changed: {md:?}"
        );
    }

    #[test]
    fn test_multiline_signature_in_pre_block_preserved() {
        // A <pre> code example that legitimately wraps a call across lines must
        // not be touched by the code-header collapse.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre class=\"rust\"><code>foo(\n    a,\n    b,\n);</code></pre>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("foo(") && text.contains("a,") && text.contains("b,"),
            "pre-block example was altered: {text:?}"
        );
    }

    #[test]
    fn test_emoji_badge_separated_from_text() {
        // rustdoc renders the nightly-API marker as
        // `<span class="emoji">\u{1f52c}</span><span>This is ...</span>` with no
        // separating whitespace, so html2md glues the flask onto "This".
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"stab unstable\">",
            "<span class=\"emoji\">\u{1f52c}</span>",
            "<span>This is a nightly-only experimental API.</span></div>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("\u{1f52c} This is a nightly-only"),
            "emoji not separated from text in markdown: {md:?}"
        );
        assert!(
            !md.contains("\u{1f52c}This"),
            "emoji still glued in markdown: {md:?}"
        );
    }

    #[test]
    fn test_playground_run_button_stripped() {
        // rustdoc adds a "Run code" button to each example as an empty-text
        // anchor wrapping a long playground URL
        // (`<a class="test-arrow" href="https://play.rust-lang.org/...">`).
        // It must not leak as an empty-text markdown link.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"example-wrap\"><pre class=\"rust\"><code>let x = 1;</code></pre>",
            "<a class=\"test-arrow\" target=\"_blank\" title=\"Run code\" ",
            "href=\"https://play.rust-lang.org/?code=fn+main()+%7B%7D\"></a></div>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            !md.contains("play.rust-lang.org"),
            "playground run button leaked into markdown: {md:?}"
        );
        assert!(!md.contains("[]("), "empty-text link survived: {md:?}");
        assert!(md.contains("let x = 1;"), "example code lost: {md:?}");
    }

    #[test]
    fn test_orphan_since_middot_collapsed() {
        // rustdoc puts `<span class="since">1.0.0</span> \u{00b7} <src>` in a
        // method's right-side metadata. Stripping the source link leaves a
        // dangling middot that, once the <summary> is flattened, glues onto the
        // signature (`1.0.0 \u{00b7} fn ...`). It should collapse to a space.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle method-toggle\" open><summary>",
            "<section id=\"method.next\" class=\"method\">",
            "<span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span>",
            " \u{00b7} <a class=\"src\" href=\"../../src/x.html#1\">Source</a></span>",
            "<h4 class=\"code-header\">fn <a href=\"#method.next\" class=\"fn\">next</a>(&amp;mut self)</h4>",
            "</section></summary></details>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("1.0.0 fn next"),
            "version not cleanly separated from signature: {md:?}"
        );
        assert!(
            !md.contains("1.0.0 \u{00b7}") && !md.contains("\u{00b7} fn next"),
            "orphan middot survived: {md:?}"
        );
    }

    #[test]
    fn test_since_badge_separated_from_signature() {
        // On FFI structs (e.g. libc) the provided trait methods carry a
        // `<span class="since">1.0.0</span>` badge directly abutting the
        // method code-header with no middot or source link in between. When the
        // <summary> is flattened the badge fuses onto the signature
        // (`1.0.0fn clone_from`). It must be separated by a space.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle method-toggle\" open><summary>",
            "<section id=\"method.clone_from\" class=\"method trait-impl\">",
            "<span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span></span>",
            "<a href=\"#method.clone_from\" class=\"anchor\">\u{00a7}</a>",
            "<h4 class=\"code-header\">fn <a href=\"#method.clone_from\" class=\"fn\">clone_from</a>(&amp;mut self, source: &amp;Self)</h4>",
            "</section></summary></details>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        let text = extract_documentation_as_text(html);
        assert!(
            md.contains("1.0.0 fn clone_from"),
            "since badge glued onto signature (markdown): {md:?}"
        );
        assert!(
            !md.contains("1.0.0fn"),
            "since badge still fused in markdown: {md:?}"
        );
        assert!(
            !text.contains("1.0.0fn"),
            "since badge still fused in text: {text:?}"
        );
    }

    #[test]
    fn test_generics_survive_summary_method_header() {
        // rustdoc wraps method-detail signatures in <details><summary>. The
        // summary's decoded text turns `Option&lt;usize&gt;` into literal
        // `Option<usize>`; without re-escaping, the second parse drops the
        // `<usize>`/`<Self::Item>` as if they were unknown tags.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle method-toggle\" open><summary>",
            "<section id=\"method.size_hint\" class=\"method\">",
            "<span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span>",
            " \u{00b7} <a class=\"src\" href=\"../../src/x.html#1\">Source</a></span>",
            "<h4 class=\"code-header\">fn <a href=\"#method.size_hint\" class=\"fn\">size_hint</a>",
            "(&amp;self) -&gt; (<a class=\"primitive\" href=\"../primitive.usize.html\">usize</a>, ",
            "<a class=\"enum\" href=\"../option/enum.Option.html\">Option</a>&lt;",
            "<a class=\"primitive\" href=\"../primitive.usize.html\">usize</a>&gt;)</h4>",
            "</section></summary></details>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("Option<usize>"),
            "generic args dropped from summary method header (markdown): {md:?}"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("Option<usize>"),
            "generic args dropped from summary method header (text): {text:?}"
        );
    }

    #[test]
    fn test_escape_html_text_reescapes_special_chars() {
        assert_eq!(escape_html_text("Vec<u8>"), "Vec&lt;u8&gt;");
        assert_eq!(escape_html_text("a & b"), "a &amp; b");
        assert_eq!(escape_html_text("Option<&T>"), "Option&lt;&amp;T&gt;");
    }

    #[test]
    fn test_portability_badge_separated_from_item_name() {
        // rustdoc glues feature pills onto item names ("fs`fs`"); they should
        // render as a clearly separated parenthetical from the badge title.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<dl class=\"item-table\">",
            "<dt><a class=\"mod\" href=\"fs/index.html\">fs</a>",
            "<span class=\"stab portability\" title=\"Available on crate feature `fs` only\">",
            "<code>fs</code></span></dt><dd>Async files.</dd>",
            "<dt><a class=\"mod\" href=\"io/index.html\">io</a></dt><dd>Async IO.</dd>",
            "</dl></section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("fs (Available on crate feature `fs` only)"),
            "feature badge not separated/labelled: {md:?}"
        );
        // The glued form must be gone.
        assert!(!md.contains("fs`fs`"), "glued badge survived: {md:?}");
        // Items without a badge are untouched (no stray parens).
        assert!(
            md.contains("io — Async IO.") || md.contains("io —"),
            "io item altered: {md:?}"
        );

        // Same separation in the plain-text format. The feature name renders
        // as a real code element, so plain text shows it undecorated.
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("fs (Available on crate feature fs only)"),
            "text badge not separated: {text:?}"
        );
    }

    #[test]
    fn test_code_attribute_on_own_line() {
        // rustdoc puts declaration attributes in block-level
        // `<div class="code-attribute">` elements inside the item-decl <pre>.
        // The attribute must keep its own line, not glue onto the declaration
        // (regression: `#[repr(i8)]pub enum Ordering`).
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre class=\"rust item-decl\"><code>",
            "<div class=\"code-attribute\">#[repr(i8)]</div>",
            "<div class=\"code-attribute\">#[non_exhaustive]</div>",
            "pub enum Ordering {\n    Less = -1,\n}</code></pre>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("#[repr(i8)]\npub enum Ordering")
                || md.contains("#[non_exhaustive]\npub enum Ordering"),
            "attribute glued onto declaration in markdown: {md:?}"
        );
        assert!(
            !md.contains("]pub enum"),
            "attribute still glued in markdown: {md:?}"
        );

        let text = extract_documentation_as_text(html);
        assert!(
            !text.contains("]pub enum"),
            "attribute still glued in text: {text:?}"
        );
        // Both attributes are present, each on its own line.
        assert!(
            text.contains("#[repr(i8)]") && text.contains("#[non_exhaustive]"),
            "an attribute was dropped: {text:?}"
        );
    }

    #[test]
    fn test_reexport_link_flattened_in_inline_code() {
        // rustdoc renders re-exports as `<code>pub use <a ...>name</a>;</code>`.
        // In markdown an anchor inside a backtick span cannot render, so the
        // link wrapper must be flattened to its text (`pub use name;`). The
        // html output format must keep the anchor.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<h2 id=\"reexports\">Re-exports</h2>",
            "<dl class=\"item-table reexports\"><dt id=\"reexport.rand_core\">",
            "<code>pub use <a class=\"mod\" ",
            "href=\"https://docs.rs/rand_core/0.10.0/rand_core/index.html\" ",
            "title=\"mod rand_core\">rand_core</a>;</code></dt></dl>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("`pub use rand_core;`"),
            "re-export code span malformed: {md:?}"
        );
        assert!(
            !md.contains("[rand_core]"),
            "unrenderable link survived inside code span: {md:?}"
        );

        // The html output format keeps the anchor (browsers render it fine).
        let html_out = extract_documentation_html(html);
        assert!(
            html_out.contains("href=\"https://docs.rs/rand_core/0.10.0/rand_core/index.html\""),
            "html output dropped the re-export link: {html_out:?}"
        );
    }

    #[test]
    fn test_code_fence_language_preserved() {
        // rustdoc annotates code blocks with a class (`rust rust-example-rendered`
        // for Rust examples, `language-<name>` for other fenced languages).
        // html2md drops this, emitting a bare ``` fence and losing the language
        // hint. It must be preserved in markdown only; the text and html
        // formats must be unaffected and free of the internal sentinel char.
        let html = concat!(
            "<div class=\"docblock\">",
            "<pre class=\"rust rust-example-rendered\"><code>let x = 1;</code></pre>",
            "<pre class=\"language-toml\"><code>v = 1</code></pre>",
            "<pre><code>plain</code></pre>",
            "</div>"
        );
        let md = extract_documentation(html);
        assert!(md.contains("```rust"), "rust fence hint missing: {md:?}");
        assert!(md.contains("```toml"), "toml fence hint missing: {md:?}");
        assert!(
            !md.contains('\u{2}'),
            "internal sentinel leaked into markdown: {md:?}"
        );

        // Text and html formats must not gain fence hints or the sentinel.
        let text = extract_documentation_as_text(html);
        assert!(
            !text.contains('\u{2}'),
            "sentinel leaked into text: {text:?}"
        );
        assert!(
            !text.contains("```rust"),
            "text format gained a fence hint: {text:?}"
        );

        let html_out = extract_documentation_html(html);
        assert!(
            !html_out.contains('\u{2}'),
            "sentinel leaked into html: {html_out:?}"
        );
    }

    #[test]
    fn test_portability_badge_feature_with_underscore_not_escaped() {
        // A feature name containing an underscore is embedded in the badge
        // title inside literal backticks. It must render as a genuine code
        // span in markdown (no stray `\_` escape) and as undecorated text in
        // the plain-text format. Regression: `thread\_rng` leaked previously.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"item-name\">",
            "<a class=\"fn\" href=\"fn.fill.html\">fill</a>",
            "<span class=\"stab portability\" ",
            "title=\"Available on crate feature `thread_rng` only\">",
            "<code>thread_rng</code></span></div>",
            "<div class=\"desc\">Fill any type.</div>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("Available on crate feature `thread_rng` only"),
            "feature code span malformed: {md:?}"
        );
        assert!(
            !md.contains("thread\\_rng"),
            "stray underscore escape in feature name: {md:?}"
        );

        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("Available on crate feature thread_rng only"),
            "text feature name malformed: {text:?}"
        );
    }

    #[test]
    fn test_stab_badge_separated_from_item_name() {
        // rustdoc glues a stability pill onto the item name in module index
        // tables (e.g. `TryReserveErrorKindExperimental`); the marker should
        // render as a clearly separated parenthetical instead.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<dl class=\"item-table\">",
            "<dt><a class=\"enum\" href=\"enum.TryReserveErrorKind.html\">",
            "TryReserve<wbr>Error<wbr>Kind</a><wbr>",
            "<span class=\"stab unstable\" title=\"\">Experimental</span></dt>",
            "<dd>Details of the allocation.</dd>",
            "<dt><a class=\"enum\" href=\"enum.Plain.html\">Plain</a></dt><dd>Stable item.</dd>",
            "</dl></section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            md.contains("TryReserveErrorKind (Experimental)"),
            "stab badge not separated/labelled: {md:?}"
        );
        // The glued form must be gone.
        assert!(
            !md.contains("KindExperimental"),
            "glued stab badge survived: {md:?}"
        );
        // Items without a badge are untouched (no stray parens).
        assert!(
            md.contains("Plain — Stable item."),
            "unbadged item altered: {md:?}"
        );

        // Same separation in the plain-text format.
        let text = extract_documentation_as_text(html);
        assert!(
            text.contains("TryReserveErrorKind (Experimental)"),
            "text stab badge not separated: {text:?}"
        );
    }

    #[test]
    fn test_deprecation_badge_separated_from_signature() {
        // rustdoc places the deprecation/stability badge in a
        // `<span class="item-info">` immediately after the signature, with no
        // separating whitespace. Inside a collapsed `<summary>` the flattened
        // text glued the badge onto the signature (e.g. `-> &str\u{1f44e}
        // Deprecated since 1.42.0: ...`). It must be space-separated instead.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<details class=\"toggle method-toggle\" open><summary>",
            "<section id=\"method.description\" class=\"method\">",
            "<h4 class=\"code-header\">fn <a href=\"#method.description\" class=\"fn\">description</a>",
            "(&amp;self) -&gt; &amp;<a class=\"primitive\" href=\"../primitive.str.html\">str</a></h4></section>",
            "<span class=\"item-info\"><div class=\"stab deprecated\">",
            "<span class=\"emoji\">\u{1f44e}</span>",
            "<span>Deprecated since 1.42.0: <p>use the Display impl or to_string()</p></span>",
            "</div></span></summary></details>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // The glued form must be gone; a space must separate signature & badge.
        assert!(
            !md.contains("str\u{1f44e}"),
            "deprecation badge glued onto signature (markdown): {md:?}"
        );
        assert!(
            md.contains("str \u{1f44e}") || md.contains("&str \u{1f44e}"),
            "deprecation badge not space-separated (markdown): {md:?}"
        );
        // Plain-text format must also separate them.
        let text = extract_documentation_as_text(html);
        assert!(
            !text.contains("str\u{1f44e}"),
            "deprecation badge glued onto signature (text): {text:?}"
        );
    }

    #[test]
    fn test_blockquote_empty_marker_lines_removed() {
        // html2md pads blockquotes with empty `>` lines; the boundary/duplicate
        // markers must be removed while genuine paragraph breaks are preserved.
        let single = concat!(
            "<html><body><section id=\"main-content\">",
            "<blockquote><p><strong>Note here</strong></p></blockquote>",
            "<p>after</p></section></body></html>"
        );
        let md = extract_documentation(single);
        assert!(
            md.contains("> **Note here**"),
            "blockquote content missing: {md:?}"
        );
        // No empty `>` marker lines should survive.
        assert!(
            !md.lines().any(|l| l.trim() == ">"),
            "empty blockquote marker line survived: {md:?}"
        );

        // A multi-paragraph blockquote keeps its internal separator line.
        let multi = concat!(
            "<html><body><section id=\"main-content\">",
            "<blockquote><p>First para.</p><p>Second para.</p></blockquote>",
            "</section></body></html>"
        );
        let md = extract_documentation(multi);
        assert!(
            md.contains("> First para.\n>\n> Second para."),
            "multi-paragraph blockquote break not preserved: {md:?}"
        );
    }

    #[test]
    fn test_superscript_footnote_converted_in_markdown() {
        // html2md has no handler for <sup>/<sub>, so rustdoc footnote
        // references leak as raw HTML into the markdown (e.g.
        // `<sup id="fnref1"><a href="#fn1">1</a></sup>`). They must be converted
        // to plain-text `^(...)` notation with nested tags stripped.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<p>zero-padded to 2 digits. ",
            "<sup id=\"fnref1\"><a href=\"#fn1\">1</a></sup></p>",
            "<p>water is H<sub>2</sub>O.</p>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        assert!(
            !md.contains("<sup") && !md.contains("</sup>") && !md.contains("<a href"),
            "superscript/anchor HTML leaked into markdown: {md:?}"
        );
        assert!(
            md.contains("2 digits. ^(1)"),
            "footnote reference not converted to ^(1): {md:?}"
        );
        assert!(
            md.contains("H_(2)O"),
            "subscript not converted to _(...): {md:?}"
        );

        // The HTML output format must keep <sup>/<sub> intact (valid markup).
        let html_out = extract_documentation_html(html);
        assert!(
            html_out.contains("<sup") && html_out.contains("<sub"),
            "html format wrongly stripped super/subscript: {html_out:?}"
        );
    }

    #[test]
    fn test_markdown_restores_space_after_inline_link() {
        // html2md drops a newline after an inline </a>, gluing the next word
        // onto the (downgraded) link, e.g. tokio docs: "moved into the
        // <a>tokio-stream</a>\ncrate.".
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<p>moved into the <a href=\"https://docs.rs/tokio-stream\">tokio-stream</a>\n",
            "crate. See <a href=\"struct.X.html\">X</a>\nfor details.</p>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // External link keeps its URL; a space now follows it.
        assert!(
            md.contains("tokio-stream) crate"),
            "missing space after external link: {md:?}"
        );
        // Downgraded relative link is followed by a space, not glued.
        assert!(
            md.contains("See X for details"),
            "missing space after downgraded link: {md:?}"
        );

        // A wrapped run before an opening parenthesis (parenthetical aside) must
        // also gain a space; html2md otherwise glues the `(` onto the link, e.g.
        // std slice docs: "the rules of references</a>\n(though ...".
        let aside = concat!(
            "<html><body><section id=\"main-content\">",
            "<p>would violate <a href=\"x.html\">the rules of references</a>\n",
            "(though possible).</p>",
            "</section></body></html>"
        );
        let aside_md = extract_documentation(aside);
        assert!(
            aside_md.contains("references (though"),
            "missing space before parenthetical aside: {aside_md:?}"
        );

        // Negative: a function-style link with no whitespace before `(` stays
        // glued (no spurious space inserted into a call expression).
        let call = concat!(
            "<html><body><section id=\"main-content\">",
            "<p>call <a href=\"x.html\">foo</a>(arg) now</p>",
            "</section></body></html>"
        );
        let call_md = extract_documentation(call);
        assert!(
            call_md.contains("foo(arg)"),
            "spurious space inserted into call expression: {call_md:?}"
        );
    }

    #[test]
    fn test_markdown_preserves_code_block_whitespace() {
        // The inline-whitespace fix must not touch <pre> contents: highlighted
        // code blocks wrap <a>/<span> tokens across indented newlines.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<pre><code>fn main() {\n",
            "    let x =\n",
            "        <a href=\"x.html\">HashMap</a>::new();\n",
            "}</code></pre>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // Indentation inside the code block is preserved (not collapsed to a
        // single leading space).
        assert!(
            md.contains("    let x ="),
            "code block indentation collapsed: {md:?}"
        );
    }

    #[test]
    fn test_markdown_unescapes_identifiers_outside_code() {
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<h1>Crate serde_json</h1>",
            "<p>Use <code>serde_json::value</code> to build <code>Vec&lt;u8&gt;</code>.</p>",
            "<p>pub fn get(&amp;self) -&gt; Option&lt;&amp;Value&gt;</p>",
            "<pre><code>let v: Vec&lt;u8&gt; = path\\to;</code></pre>",
            "</section></body></html>"
        );
        let md = extract_documentation(html);
        // Escapes removed from ordinary text and signatures.
        assert!(
            md.contains("Crate serde_json"),
            "heading still escaped: {md:?}"
        );
        assert!(
            md.contains("-> Option<&Value>"),
            "signature still escaped: {md:?}"
        );
        assert!(!md.contains("\\_"), "stray underscore escape: {md:?}");
        assert!(
            !md.contains("\\<") && !md.contains("\\>"),
            "stray angle escape: {md:?}"
        );
        // Inline code span is preserved verbatim (no escaping introduced).
        assert!(
            md.contains("`serde_json::value`"),
            "inline code mangled: {md:?}"
        );
        // Fenced code content (a genuine backslash) is left untouched.
        assert!(md.contains("path\\to"), "fenced backslash altered: {md:?}");
    }

    #[test]
    fn test_clean_html_strips_oddly_formatted_block_elements() {
        // Navigation/header/footer/aside elements must be removed even when
        // their source markup is not formatted the way html5ever serializes it
        // (e.g. extra whitespace inside the tag). Previously the cleanup relied
        // on string-matching the serialized element against the raw HTML, which
        // silently leaked such elements into the output.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<nav  class=\"sidebar\">NAVLEAK</nav>",
            "<header  data-x=\"1\">HEADERLEAK</header>",
            "<footer   >FOOTERLEAK</footer>",
            "<aside  role=\"note\">ASIDELEAK</aside>",
            "<p>Real doc.</p>",
            "</section></body></html>"
        );
        let cleaned = clean_html(html);
        for leak in ["NAVLEAK", "HEADERLEAK", "FOOTERLEAK", "ASIDELEAK"] {
            assert!(!cleaned.contains(leak), "{leak} leaked: {cleaned}");
        }
        assert!(cleaned.contains("Real doc."), "content dropped: {cleaned}");
    }

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
    fn test_html_to_text_superscript_uses_caret_notation() {
        // In plain text a bare footnote number is easily mistaken for body
        // text; <sup>/<sub> should render as `^(...)`/`_(...)`, matching the
        // markdown path.
        let html = "<p>zero-padded to 2 digits. <sup id=\"f\"><a href=\"#fn1\">1</a></sup></p>                    <p>water is H<sub>2</sub>O.</p>";
        let text = html_to_text(html);
        assert!(
            text.contains("2 digits. ^(1)"),
            "superscript not rendered as ^(1): {text:?}"
        );
        assert!(
            text.contains("H_(2)O"),
            "subscript not rendered as _(2): {text:?}"
        );
        // No bare anchor/tag leakage.
        assert!(
            !text.contains("<sup") && !text.contains("<a href"),
            "raw tags leaked into text: {text:?}"
        );
    }

    #[test]
    fn test_html_to_text_table_rows_stay_on_one_line() {
        // Table cells in a row must render on a single line joined by ` | `
        // (not scattered one-cell-per-line), so the row's columns stay
        // associated in the plain-text output.
        let html = concat!(
            "<table><thead><tr><th>Spec.</th><th>Example</th><th>Description</th></tr></thead>",
            "<tbody><tr><td>%Y</td><td>2001</td><td>The full year.</td></tr>",
            "<tr><td>%m</td><td>07</td><td>Month number.</td></tr></tbody></table>"
        );
        let text = html_to_text(html);
        assert!(
            text.contains("Spec. | Example | Description"),
            "header row not joined with ` | `: {text:?}"
        );
        assert!(
            text.contains("%Y | 2001 | The full year."),
            "data row not joined with ` | `: {text:?}"
        );
        // Distinct rows remain on separate lines.
        assert!(
            text.contains("The full year.\n%m | 07 | Month number."),
            "rows not on separate lines: {text:?}"
        );
    }

    #[test]
    fn test_html_to_text_table_preserves_empty_leading_cell() {
        // A table whose header has an empty leading (row-label) cell must keep
        // that empty cell in the text output so the header columns stay aligned
        // with the data rows (header and every data row keep the same column
        // count).
        let html = concat!(
            "<table><thead><tr><th></th><th>get(i)</th><th>insert(i)</th></tr></thead>",
            "<tbody><tr><td>Vec</td><td>O(1)</td><td>O(n-i)</td></tr></tbody></table>"
        );
        let text = html_to_text(html);
        let header = text
            .lines()
            .find(|l| l.contains("get(i)"))
            .expect("header row missing");
        let data = text
            .lines()
            .find(|l| l.contains("Vec"))
            .expect("data row missing");
        // Both rows must have the same number of ` | `-joined columns (3).
        assert_eq!(
            header.matches('|').count(),
            data.matches('|').count(),
            "header/data column counts misaligned: header={header:?} data={data:?}"
        );
        assert_eq!(
            data.trim(),
            "Vec | O(1) | O(n-i)",
            "data row not joined correctly: {data:?}"
        );
    }

    #[test]
    fn test_html_to_text_drops_empty_spacer_rows() {
        // Some tables insert all-empty "visual spacer" rows between data rows.
        // In text these must be dropped, not rendered as content-free `| |`
        // noise; rows with any content are kept (with their column structure).
        let html = concat!(
            "<table><tbody>",
            "<tr><td>%h</td><td>Jul</td><td>Same as %b.</td></tr>",
            "<tr><td></td><td></td><td></td></tr>",
            "<tr><td>%d</td><td>08</td><td>Day number.</td></tr>",
            "</tbody></table>"
        );
        let text = html_to_text(html);
        assert!(
            !text.lines().any(|l| l.trim() == "| |" || l.trim() == "|"),
            "empty spacer row rendered as pipe noise: {text:?}"
        );
        // Genuine data rows are preserved.
        assert!(
            text.contains("%h | Jul | Same as %b.") && text.contains("%d | 08 | Day number."),
            "data rows lost: {text:?}"
        );
    }

    #[test]
    fn test_structfield_spans_render_on_separate_lines() {
        // rustdoc emits one `<span class="structfield section-header">` per
        // field with no separating whitespace; adjacent spans must not glue
        // (markdown `a: A``b: B`) or fuse tokens in text (`A_tb`).
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<h2>Fields</h2>",
            "<span id=\"structfield.sa_family\" class=\"structfield section-header\">",
            "<a href=\"#structfield.sa_family\" class=\"anchor field\">\u{a7}</a>",
            "<code>sa_family: <a class=\"type\" href=\"type.sa_family_t.html\">sa_family_t</a></code></span>",
            "<span id=\"structfield.sa_data\" class=\"structfield section-header\">",
            "<a href=\"#structfield.sa_data\" class=\"anchor field\">\u{a7}</a>",
            "<code>sa_data: [<a class=\"type\" href=\"type.c_char.html\">c_char</a>; 14]</code></span>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(
            !text.contains("sa_family_tsa_data"),
            "struct field tokens fused in text: {text:?}"
        );
        assert!(
            text.contains("sa_family: sa_family_t") && text.contains("sa_data: [c_char; 14]"),
            "field declarations missing in text: {text:?}"
        );
        let md = extract_documentation(html);
        // Each field is on its own line (no two field decls share a line).
        assert!(
            !md.lines()
                .any(|l| l.contains("sa_family") && l.contains("sa_data")),
            "struct fields glued on one line in markdown: {md:?}"
        );
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
        assert!(
            text.contains("Dl_info\nElf32_Chdr\nFoo"),
            "blocks not on separate lines: {text}"
        );
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
    fn test_text_strips_trailing_orphan_middot() {
        // The out-of-band stability row (`1.0.0 \u{00b7} <source>`) leaves a
        // dangling middot once the source link is stripped.
        let html = concat!(
            "<html><body><section id=\"main-content\">",
            "<div class=\"out-of-band\">1.0.0 \u{00b7} ",
            "<a class=\"src\" href=\"../src/x.rs.html\">source</a></div>",
            "<p>Body text.</p>",
            "</section></body></html>"
        );
        let text = extract_documentation_as_text(html);
        assert!(text.contains("Body text."), "body dropped: {text:?}");
        assert!(
            !text.contains("1.0.0 \u{00b7}"),
            "orphan middot survived in text: {text:?}"
        );
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
        // Block elements are now separated by a newline rather than a space.
        assert!(
            text.contains("data\nRandomState"),
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
    fn test_clean_markdown_keeps_external_html_links() {
        // Absolute external links that happen to end in `.html` must keep their
        // URL rather than being downgraded to bare label text.
        let md = "See the [Guide](https://example.com/book/ch01.html) for details.";
        let out = clean_markdown(md);
        assert!(
            out.contains("[Guide](https://example.com/book/ch01.html)"),
            "external link should be preserved, got: {out}"
        );
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
    fn test_clean_markdown_relative_link_with_bracketed_label() {
        // Intra-doc links whose label contains `]` (Rust attribute syntax
        // `#[tokio::main]`, slice/array types `[u8]`, `[T; N]`) must still be
        // downgraded to their text. Previously the label pattern stopped at the
        // first `]`, leaving a broken docs.rs-relative `.html` link.
        let md = concat!(
            "Use [`#[tokio::main]`](attr.main.html) and the slice ",
            "[`[u8]`](primitive.slice.html) plus [Foo](struct.Foo.html)."
        );
        let out = clean_markdown(md);
        assert!(!out.contains(".html"), "relative link survived: {out}");
        assert!(
            !out.contains("](attr"),
            "bracketed-label link survived: {out}"
        );
        assert!(
            out.contains("`#[tokio::main]`"),
            "attribute label text dropped: {out}"
        );
        assert!(out.contains("`[u8]`"), "slice label text dropped: {out}");
        assert!(out.contains("Foo"), "plain label text dropped: {out}");
    }

    #[test]
    fn test_negative_impl_trait_not_rendered_as_image() {
        // rustdoc negative auto-trait impls (`impl<T> !Freeze for Mutex<T>`)
        // place a text `!` directly before the linkified trait, which html2md
        // fuses into `![Freeze](url)` \u{2014} markdown image syntax that renders as
        // a broken embedded image. The `!` must be backslash-escaped so it stays
        // literal text.
        let input = concat!(
            "### impl<T> ![Freeze]",
            "(https://doc.rust-lang.org/nightly/core/marker/trait.Freeze.html)",
            " for Mutex<T>\n"
        );
        let md = clean_markdown(input);
        assert!(
            md.contains(r"\![Freeze]"),
            "negative-impl marker not escaped: {md:?}"
        );
        assert!(
            !md.contains("> ![Freeze]"),
            "unescaped image syntax survived: {md:?}"
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
    fn test_clean_markdown_drops_relative_read_more_keeps_absolute() {
        // rustdoc appends a "Read more" link to inherited/derived method
        // summaries. A docs.rs-relative target is unreachable and would be
        // downgraded to a dangling "Read more"; it must be dropped entirely.
        // An absolute (scheme://) target stays a usable link.
        let md = "Returns a duplicate of the value. [Read more](../clone/trait.Clone.html#tymethod.clone)";
        let out = clean_markdown(md);
        assert_eq!(
            out.trim(),
            "Returns a duplicate of the value.",
            "relative Read more affordance not dropped cleanly: {out:?}"
        );
        let md2 = "Formats the value. [Read more](https://doc.rust-lang.org/core/fmt/trait.Debug.html#tymethod.fmt)";
        let out2 = clean_markdown(md2);
        assert!(
            out2.contains(
                "[Read more](https://doc.rust-lang.org/core/fmt/trait.Debug.html#tymethod.fmt)"
            ),
            "absolute Read more link wrongly dropped: {out2:?}"
        );
    }

    #[test]
    fn test_clean_markdown_downgrades_rustdoc_item_anchors() {
        // rustdoc cross-links items with type-prefixed fragment anchors
        // (`#method.X`, `#associatedtype.X`, `#impl-...`). These ids do not
        // exist in the rendered markdown, so the links are dead and must be
        // downgraded to their label. Genuine section anchors must be kept.
        let md = concat!(
            "fn [parse](#method.parse)() -> Box and ",
            "[`Error`](#associatedtype.Error) plus ",
            "[here](#impl-Clone-for-Foo). See [Quick start](#quick-start)."
        );
        let out = clean_markdown(md);
        assert!(
            !out.contains("#method.parse"),
            "method anchor survived: {out}"
        );
        assert!(
            !out.contains("#associatedtype.Error"),
            "assoc-type anchor survived: {out}"
        );
        assert!(!out.contains("#impl-"), "impl anchor survived: {out}");
        // Labels are kept as text.
        assert!(out.contains("fn parse()"), "method label dropped: {out}");
        assert!(out.contains("`Error`"), "assoc-type label dropped: {out}");
        assert!(out.contains("here"), "impl label dropped: {out}");
        // Genuine section anchors are preserved.
        assert!(
            out.contains("[Quick start](#quick-start)"),
            "section anchor wrongly downgraded: {out}"
        );
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
    fn test_clean_markdown_strips_trailing_middot_and_nbsp() {
        // The stability/out-of-band line keeps a dangling middot once the
        // trailing source link is stripped (e.g. "1.0.0 \u{00b7}"); and rustdoc
        // headings often end with a non-breaking space.
        let md = "Struct HashMap\u{00a0} \n==========\n\n1.0.0 \u{00b7}\n\nBody.";
        let out = clean_markdown(md);
        assert!(
            out.contains("Struct HashMap\n"),
            "trailing nbsp not trimmed from heading: {out:?}"
        );
        assert!(
            out.contains("1.0.0\n") || out.ends_with("1.0.0\n\nBody."),
            "trailing middot not stripped: {out:?}"
        );
        assert!(
            !out.contains("1.0.0 \u{00b7}"),
            "orphan middot survived: {out:?}"
        );
        // Inline middots between words on the same line are preserved.
        assert!(
            clean_markdown("a \u{00b7} b").contains('\u{00b7}'),
            "inline middot wrongly dropped"
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
