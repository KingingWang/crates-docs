# Docs Lookup Internal Dedup Design

## Summary

This design proposes a maintainability-focused internal refactor for the docs lookup tools so that `lookup_crate` and `lookup_item` share one internal fetch/cache execution pattern while preserving current external behavior. The change is intentionally narrow: it targets duplicated flow in the docs tools, keeps cache key and TTL semantics unchanged, and avoids public API, transport, HTML hot-path, and test-suite organization changes.

## Problem Statement

`src/tools/docs/lookup_crate.rs` and `src/tools/docs/lookup_item.rs` currently implement nearly the same internal sequence for markdown lookups:

1. check the relevant `DocCache` entry
2. build the docs.rs URL
3. fetch HTML through `DocService::fetch_html`
4. extract markdown from the HTML
5. write the rendered result back to the cache

They also repeat closely related non-markdown flows for text and HTML output. This duplication increases maintenance cost in a few ways:

- fixes to fetch/cache behavior must be made in multiple places
- tool-specific code mixes orchestration with content-specific extraction
- tests have to cover similar control flow through multiple implementations
- future internal changes risk accidental divergence in error handling or cache-write behavior

The duplication is not currently a user-visible bug, but it makes low-risk internal changes harder than they need to be.

## Goals

- Deduplicate the internal execution flow shared by `lookup_crate` and `lookup_item`.
- Improve readability by separating shared orchestration from tool-specific URL building, extraction, and cache access.
- Keep the refactor small and reviewable.
- Preserve current external behavior, including:
  - tool names
  - request/response shape
  - output content behavior by format
  - cache key semantics
  - cache TTL semantics
  - external error contract and error-prefix style
- Keep the design compatible with existing `DocService` and `DocCache` responsibilities.

## Non-Goals

- Rewriting `src/tools/docs/html.rs` or changing extraction algorithms.
- Changing tool schemas or any public API surface.
- Changing server, transport, or registry behavior beyond local internal wiring needed by the refactor.
- Reorganizing the broader test suite.
- Unifying search-tool behavior with the lookup tools in this PR.
- Changing cache normalization, key generation, TTL values, jitter, or invalidation semantics.

## Current-State Observations

- `lookup_crate` and `lookup_item` each own their own fetch/cache orchestration.
- Both tools use `DocService::fetch_html` and `DocCache` directly.
- Cache APIs are intentionally specific today: `get_crate_docs` / `set_crate_docs` and `get_item_docs` / `set_item_docs`.
- Text and HTML formats are not cached today; only rendered markdown goes through the docs cache.
- Shared helpers already exist in `src/tools/docs/mod.rs` for format parsing and URL construction, so this module is the natural place for a small internal execution abstraction.

## Approaches Considered

### Approach 1: Keep separate tool implementations and only extract tiny helper functions

Extract small free functions such as `fetch_or_cache_crate_docs(...)` and `fetch_or_cache_item_docs(...)`, leaving most control flow in each tool.

**Pros**
- Lowest immediate code churn.
- Easy to land quickly.
- Minimal structural change.

**Cons**
- Deduplicates only fragments, not the actual execution pattern.
- Still leaves parallel logic paths that can drift over time.
- Shared behavior remains spread across multiple files.

**Assessment**
This is safe but too shallow for the stated maintainability goal.

### Approach 2: Add a small internal shared executor in `src/tools/docs/mod.rs` and keep tool-specific logic as callbacks or typed operations

Introduce an internal abstraction for the common orchestration steps, while each tool supplies its own URL builder, markdown extractor, cache read/write operations, and optional text post-processing.

**Pros**
- Deduplicates the important flow without changing public behavior.
- Keeps tool-specific behavior explicit and local.
- Fits existing module boundaries because `mod.rs` already hosts shared docs-tool internals.
- Easy to test at the tool level, with a small number of new shared-unit tests if useful.

**Cons**
- Requires some careful API design to avoid over-generalization.
- A closure-heavy design could become harder to read if pushed too far.

**Assessment**
Best balance of maintainability, clarity, and low implementation risk.

### Approach 3: Expand `DocService` or `DocCache` to own lookup-specific execution

Move most or all lookup orchestration into `DocService` methods such as `lookup_crate_markdown(...)` and `lookup_item_markdown(...)`.

**Pros**
- Very strong centralization.
- Tools become extremely thin wrappers.

**Cons**
- Pushes tool-specific extraction behavior into a service layer that currently acts more like infrastructure.
- Risks broadening `DocService` responsibilities beyond transport/cache plumbing.
- Harder to keep the change obviously scoped to docs-tool internals.

**Assessment**
Too invasive for a low-risk maintainability PR.

## Recommendation

Use **Approach 2**: a small internal shared executor located in `src/tools/docs/mod.rs` (or a private sibling submodule declared there) that centralizes the repeated lookup flow while keeping crate- and item-specific behavior in their respective tool files.

This keeps the refactor focused, preserves current layering, and avoids turning `DocService` into a domain-specific lookup engine.

## Proposed Architecture

### Design Principle

Share the orchestration, not the content rules.

The common logic should answer: “how does a docs lookup execute?” The individual tools should still answer: “what URL do I fetch, how do I extract markdown, and which cache entry do I use?”

### Shared Internal Boundary

Add a private internal executor for docs lookup formats with responsibilities like:

- dispatch on `Format`
- for markdown:
  - attempt cache read
  - fetch HTML on miss
  - run tool-specific markdown extraction
  - persist rendered markdown through the existing cache API
- for text:
  - fetch HTML
  - convert HTML to text
  - allow optional tool-specific text decoration
- for HTML:
  - fetch raw HTML
- reject unsupported JSON format in the same tool-specific way used today

This executor should not know about crate names or item paths semantically; it should operate on supplied closures or operation structs.

### Suggested Shape

One pragmatic shape is an internal operation struct, for example in concept:

- `tool_name: &'static str`
- `format: Format`
- `fetch_url: Fn() -> String`
- `read_markdown_cache: async fn -> Option<String>`
- `write_markdown_cache: async fn(String) -> Result<(), crate::error::Error>`
- `extract_markdown: Fn(&str) -> String`
- `text_from_html: Fn(&str) -> String` or a `postprocess_text` hook layered over `html::html_to_text`
- `json_unsupported_message: &'static str`

The exact Rust shape can be tuned during implementation, but the important boundary is:

- shared executor owns orchestration and error mapping for shared steps
- tool impls own only parameter parsing, operation construction, and tool-specific extraction/text shaping

### Flow by Format

#### Markdown

1. tool parses input and resolves `Format::Markdown`
2. tool builds a shared operation for crate or item lookup
3. shared executor attempts the existing markdown cache read
4. on miss, executor fetches HTML using `DocService::fetch_html(..., Some(tool_name))`
5. executor applies the tool-specific markdown extractor:
   - crate: `html::extract_documentation`
   - item: `html::extract_search_results`
6. executor writes markdown back via the existing cache-specific setter
7. executor returns the rendered text content

#### Text

1. executor fetches HTML with the same URL path as today
2. executor converts to plain text via `html::html_to_text`
3. item lookup applies its existing prefix decoration (`Search results: {item_path}\n\n...`)
4. crate lookup returns the raw text conversion unchanged

#### HTML

1. executor fetches HTML
2. executor returns it unchanged

### Error and Cache Semantics

The implementation must preserve these behaviors exactly:

- continue using the existing `DocCache` getter/setter methods so key semantics stay unchanged
- continue using the existing `DocCacheTtl` configuration path so TTL semantics stay unchanged
- continue routing network failures through `DocService::fetch_html(..., Some(TOOL_NAME))`
- continue mapping cache write failures to `CallToolError::from_message(format!("[{TOOL_NAME}] Cache set failed: {e}"))`
- continue rejecting JSON for both tools with the same tool-specific invalid-arguments contract

The refactor should centralize shared logic without normalizing away these externally observable details.

## File Touch List

Expected implementation touches for the later PR:

- `src/tools/docs/mod.rs`
  - add the private shared lookup execution abstraction
  - keep it internal to the docs tools module
- `src/tools/docs/lookup_crate.rs`
  - reduce to parameter parsing plus crate-specific operation wiring
- `src/tools/docs/lookup_item.rs`
  - reduce to parameter parsing plus item-specific operation wiring
- `src/tools/docs/cache/mod.rs` *(optional, only if a tiny ergonomic helper materially simplifies shared wiring)*
  - no semantic cache changes
- `src/tools/docs/cache/key.rs` *(not expected)*
  - only touch if a supporting comment or test is needed; no key behavior change
- `src/tools/docs/cache/ttl.rs` *(not expected)*
  - only touch if a supporting comment or test is needed; no TTL behavior change
- `tests/unit/tools_docs_tests.rs` and/or existing docs-tool unit tests
  - add focused regression coverage for unchanged behavior where appropriate

## Testing and Verification Strategy

The implementation PR should verify behavior stability rather than chase new behavior.

### Unit-Level Expectations

- existing URL-building tests for crate and item lookups remain green
- existing format parsing behavior remains green
- existing lookup tool tests continue to validate unchanged external outputs

### New or Updated Focused Tests

Add or update tests to confirm:

- markdown crate lookup still reads from and writes to the crate-docs cache path
- markdown item lookup still reads from and writes to the item-docs cache path
- cache-write failures still produce the same tool-prefixed external error messages
- text output remains unchanged for both tools, especially the item-search prefix behavior
- HTML output remains a direct fetch result with no cache coupling introduced accidentally
- JSON remains rejected for both lookup tools exactly as before

### Command Verification for the Implementation PR

At minimum, run:

- `cargo fmt -- --check`
- `cargo test --test unit`
- targeted tests for docs lookup behavior if they live in another existing test target

If the eventual implementation touches feature-independent docs internals only, broader clippy runs are still appropriate before merge, but the key regression signal for this refactor is unchanged tool behavior.

## Risks and Mitigations

### Risk: Over-generalized abstraction hurts readability

If the shared executor becomes too generic, maintainability can get worse rather than better.

**Mitigation**
- prefer a small private abstraction over a generic framework
- optimize for readability in exactly two consumers: `lookup_crate` and `lookup_item`
- stop short of pulling `search` into the same abstraction in this PR

### Risk: Silent behavior drift in error messages

Centralizing execution can accidentally normalize tool-specific wording.

**Mitigation**
- keep `TOOL_NAME` and JSON rejection messages supplied by each tool
- add focused tests for cache-write and invalid-format paths

### Risk: Accidental cache semantic changes

Refactoring shared code could inadvertently alter which getter/setter is used or when caching occurs.

**Mitigation**
- keep cache access delegated to tool-provided operations that call existing `DocCache` methods
- do not introduce new cache key builders or TTL logic in the shared executor
- ensure only markdown remains cached

### Risk: Scope creep into HTML extraction or service-layer redesign

The duplication may tempt follow-on cleanup in `html.rs` or `DocService`.

**Mitigation**
- explicitly keep extraction algorithms unchanged
- keep `DocService` as the fetch utility rather than moving lookup semantics into it
- treat any additional cleanup as a separate follow-up PR

## Rollout Notes

- This is a low-risk internal refactor intended for a normal PR into `main`/`master` later.
- Review should focus on behavioral parity, simpler ownership boundaries, and reduced duplicate orchestration.
- If the shared abstraction requires more than the expected file touch list above, that is a signal to narrow the implementation rather than expand the design.

## Acceptance Criteria

The later implementation should be considered complete when all of the following are true:

- `lookup_crate` and `lookup_item` share one internal execution pattern for docs fetch/cache orchestration
- tool-specific responsibilities remain limited to parameter handling and content-specific rules
- external outputs and error behavior remain unchanged
- cache key and TTL semantics remain unchanged
- no out-of-scope files or concerns are pulled into the PR without a new design decision
