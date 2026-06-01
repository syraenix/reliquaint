//! Companion Markdown → sanitized HTML rendering (v0.4 Milestone 2).
//!
//! Tap maintainers ship walkthroughs/hints as Markdown (see [`crate::companion`]).
//! This module turns that Markdown into HTML that is safe to drop into the
//! webview. The security model is `docs/adr-0006-companion-content-rendering.md`;
//! the short version: arbitrary tap content must not be able to run scripts,
//! reach the network, or escape the rendering boundary.
//!
//! Pipeline ([`render_markdown`]):
//! 1. Parse with `pulldown-cmark` (tables/footnotes/strikethrough/task lists on).
//! 2. Neutralize raw HTML: `Html`/`InlineHtml` events become `Text`, so a
//!    `<script>` in the source is escaped as visible text, never emitted as
//!    markup (Task 2.1).
//! 3. Rewrite URLs in the event stream (Task 2.4): relative image `src` →
//!    `reliquaint-content://` (served by [`crate::companion_protocol`]); relative
//!    `.md` links → `reliquaint-nav://` (an internal-navigation marker the GUI
//!    intercepts); `http(s)` links pass through; remote/`data:`/`file:` image
//!    sources are dropped.
//! 4. Render to HTML, then sanitize with `ammonia`'s strict whitelist (Task 2.2)
//!    as defense-in-depth.

use std::collections::{HashMap, HashSet};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};

/// Characters left un-encoded in a URL path segment: the unreserved set
/// (`A–Z a–z 0–9 - _ . ~`). Everything else — spaces, `%`, `#`, etc. — is
/// percent-encoded so companion paths survive the round trip through the URL.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Render a companion Markdown document to sanitized, webview-safe HTML.
///
/// `tap_id` and `game_id` scope the rewriting of relative image and link
/// targets to the document's owning tap and game.
pub fn render_markdown(md: &str, tap_id: &str, game_id: &str) -> String {
    // Span for profiling the render pipeline (ADR-0004); zero-cost when no
    // subscriber is attached. `RUST_LOG=reliquaint=debug` surfaces timings.
    let _span =
        tracing::info_span!("companion_render", tap_id, game_id, in_bytes = md.len()).entered();

    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let events = Parser::new_ext(md, options).map(|event| match event {
        // Raw HTML is never trusted: render it as escaped text, not markup.
        Event::Html(h) => Event::Text(h),
        Event::InlineHtml(h) => Event::Text(h),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: rewrite_image_dest(dest_url, tap_id, game_id),
            title,
            id,
        }),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: rewrite_link_dest(dest_url, tap_id, game_id),
            title,
            id,
        }),
        other => other,
    });

    let mut html_out = String::new();
    html::push_html(&mut html_out, events);
    let cleaned = sanitize(&html_out);
    tracing::debug!(out_bytes = cleaned.len(), "rendered companion markdown");
    cleaned
}

/// Relative image references in `md` (image `src`s with no URL scheme, i.e.
/// tap-local files like `maps/x.png`). Used by the doctor to flag references
/// that point at files that don't exist. Scheme'd sources (remote/`data:`) are
/// skipped — they're handled by the renderer/sanitizer, not on-disk files.
pub fn relative_image_refs(md: &str) -> Vec<String> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let mut refs = Vec::new();
    for event in Parser::new_ext(md, options) {
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            if !has_scheme(&dest_url) {
                refs.push(dest_url.trim_start_matches("./").to_string());
            }
        }
    }
    refs
}

/// Total bytes of raw HTML in `md` (the `Html`/`InlineHtml` events the renderer
/// escapes rather than emits). A non-trivial count means a tap submitted HTML
/// expecting it to render — the doctor warns so users know it was neutralized.
pub fn raw_html_byte_count(md: &str) -> usize {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    Parser::new_ext(md, options)
        .map(|event| match event {
            Event::Html(h) | Event::InlineHtml(h) => h.len(),
            _ => 0,
        })
        .sum()
}

/// Rewrite an image `src`. Relative paths resolve to the tap-local image
/// protocol; anything with a URL scheme (remote `http(s)`, `data:`, `file:`)
/// is dropped to an empty `src` so it cannot leak the user's activity or probe
/// the filesystem (ADR-0006: no remote resources).
fn rewrite_image_dest<'a>(dest: CowStr<'a>, tap_id: &str, game_id: &str) -> CowStr<'a> {
    if has_scheme(&dest) {
        CowStr::from("")
    } else {
        let rel = encode_rel_path(dest.trim_start_matches("./"));
        CowStr::from(format!("reliquaint-content://{tap_id}/{game_id}/{rel}"))
    }
}

/// Rewrite a link `href`. `http(s)` URLs pass through (the GUI opens them in
/// the system browser); relative links to other `.md` files in the same
/// companion directory become internal-navigation markers; everything else is
/// left untouched for the sanitizer's scheme allowlist / relative-URL policy to
/// strip (`javascript:`, `data:`, `mailto:`, bare relative paths, …).
fn rewrite_link_dest<'a>(dest: CowStr<'a>, tap_id: &str, game_id: &str) -> CowStr<'a> {
    if has_scheme(&dest) {
        dest
    } else if is_markdown_target(&dest) {
        let rel = encode_rel_path(dest.trim_start_matches("./"));
        CowStr::from(format!("reliquaint-nav://{tap_id}/{game_id}/{rel}"))
    } else {
        dest
    }
}

/// Percent-encode each `/`-separated segment of a relative path so the result
/// is a valid URL path that round-trips back to the original on-disk path.
/// Segment separators (`/`) are preserved; everything non-unreserved within a
/// segment (spaces, `#`, `%`, …) is escaped. Inverse: [`split_and_decode_content_path`].
fn encode_rel_path(rel: &str) -> String {
    rel.split('/')
        .map(|seg| utf8_percent_encode(seg, SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Split a `reliquaint-content://`/`reliquaint-nav://` URL *path* (the part
/// after the `<tap-id>` authority, e.g. `/qfg1-ega/maps/boss%20fight.png`) into
/// its decoded `(game_id, rel_path)`. Each segment is percent-decoded exactly
/// once, so the returned `rel_path` is the real on-disk relative path.
///
/// Decoding here (not deeper) keeps the filesystem boundary check in
/// `companion_protocol::resolve_within` working on real path components — an
/// encoded `..` decodes back to `..` and is still rejected there.
pub fn split_and_decode_content_path(path: &str) -> (String, String) {
    let mut segments = path
        .trim_start_matches('/')
        .split('/')
        .map(|seg| percent_decode_str(seg).decode_utf8_lossy().into_owned());
    let game_id = segments.next().unwrap_or_default();
    let rel_path = segments.collect::<Vec<_>>().join("/");
    (game_id, rel_path)
}

/// Does `url` begin with a URL scheme (`scheme:`)? A leading ASCII letter
/// followed by letters/digits/`+`/`-`/`.` and then a `:`. Relative paths like
/// `maps/x.png` or `02-mid.md` have no scheme.
fn has_scheme(url: &str) -> bool {
    let bytes = url.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b':' => return i > 0,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'-' | b'.' => continue,
            _ => return false,
        }
    }
    false
}

/// Is `dest` a relative link to a Markdown file (path part ends with `.md`,
/// ignoring any `#fragment` or `?query`)?
fn is_markdown_target(dest: &str) -> bool {
    let path = dest.split(['#', '?']).next().unwrap_or(dest);
    path.to_ascii_lowercase().ends_with(".md")
}

/// Sanitize rendered HTML to the strict ADR-0006 whitelist. Only the listed
/// tags/attributes survive; URLs are restricted to the four allowed schemes and
/// relative URLs are denied (the pipeline has already rewritten the legitimate
/// relative references to `reliquaint-content`/`reliquaint-nav` schemes).
fn sanitize(html: &str) -> String {
    let tags: HashSet<&str> = [
        "p",
        "em",
        "strong",
        "code",
        "pre",
        "blockquote",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "ul",
        "ol",
        "li",
        "a",
        "img",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "hr",
        "br",
        "del",
        "input",
        "sup",
    ]
    .into_iter()
    .collect();

    let mut tag_attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    tag_attributes.insert("a", ["href"].into_iter().collect());
    tag_attributes.insert("img", ["src", "alt"].into_iter().collect());
    tag_attributes.insert(
        "input",
        ["type", "checked", "disabled"].into_iter().collect(),
    );

    // Task-list checkboxes only: force `type="checkbox"`.
    let mut input_values: HashMap<&str, HashSet<&str>> = HashMap::new();
    input_values.insert("type", ["checkbox"].into_iter().collect());
    let mut tag_attribute_values: HashMap<&str, HashMap<&str, HashSet<&str>>> = HashMap::new();
    tag_attribute_values.insert("input", input_values);

    let url_schemes: HashSet<&str> = ["http", "https", "reliquaint-content", "reliquaint-nav"]
        .into_iter()
        .collect();

    ammonia::Builder::default()
        .tags(tags)
        .tag_attributes(tag_attributes)
        .tag_attribute_values(tag_attribute_values)
        .generic_attributes(HashSet::new())
        .url_schemes(url_schemes)
        .url_relative(ammonia::UrlRelative::Deny)
        .clean(html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(md: &str) -> String {
        render_markdown(md, "core", "qfg1-ega")
    }

    // ---- doctor helpers (Milestone 4) ------------------------------------

    // ---- URL segment encoding round-trip (PR #36 review) -----------------

    #[test]
    fn encode_rel_path_escapes_segments_and_keeps_separators() {
        assert_eq!(
            encode_rel_path("maps/boss fight.png"),
            "maps/boss%20fight.png"
        );
        // Unreserved chars pass through; `/` separators are kept.
        assert_eq!(encode_rel_path("01-a_b.c~/x.png"), "01-a_b.c~/x.png");
        // A `#` inside a segment is escaped so it can't be read as a fragment.
        assert_eq!(encode_rel_path("a#b.png"), "a%23b.png");
    }

    #[test]
    fn spaced_image_and_link_round_trip_through_url() {
        // A spaced filename is referenced with Markdown's angle-bracket form.
        let img = render("![m](<maps/boss fight.png>)\n");
        assert!(
            img.contains("src=\"reliquaint-content://core/qfg1-ega/maps/boss%20fight.png\""),
            "{img}"
        );
        let link = render("[next](<mid game.md>)\n");
        assert!(
            link.contains("href=\"reliquaint-nav://core/qfg1-ega/mid%20game.md\""),
            "{link}"
        );

        // The receiving side decodes back to the real on-disk path.
        let (game, rel) = split_and_decode_content_path("/qfg1-ega/maps/boss%20fight.png");
        assert_eq!(game, "qfg1-ega");
        assert_eq!(rel, "maps/boss fight.png");
    }

    #[test]
    fn encoding_covers_url_chars_pulldown_leaves_live() {
        // pulldown's own href-escaping leaves `#`/`?` intact (it treats them as
        // URL-safe), which would read as a fragment/query. encode_rel_path
        // escapes them so they round-trip as filename characters.
        let img = render("![m](<a#b.png>)\n");
        assert!(
            img.contains("src=\"reliquaint-content://core/qfg1-ega/a%23b.png\""),
            "{img}"
        );
        let (_, rel) = split_and_decode_content_path("/qfg1-ega/a%23b.png");
        assert_eq!(rel, "a#b.png");
    }

    #[test]
    fn decode_restores_dotdot_so_boundary_check_still_rejects() {
        // An encoded `..` must decode back to a literal `..` component, so
        // resolve_within rejects it rather than being fooled by `%2E%2E`.
        let (_, rel) = split_and_decode_content_path("/game/%2E%2E/secret.png");
        assert_eq!(rel, "../secret.png");
    }

    #[test]
    fn relative_image_refs_collects_only_local_paths() {
        let md = "![a](maps/x.png) ![b](./sub/y.gif) ![c](https://e/p.png) ![d](data:image/png;base64,AA)";
        let refs = relative_image_refs(md);
        assert_eq!(
            refs,
            vec!["maps/x.png".to_string(), "sub/y.gif".to_string()]
        );
    }

    #[test]
    fn relative_image_refs_empty_without_images() {
        assert!(relative_image_refs("# just text\n\n[a link](other.md)").is_empty());
    }

    #[test]
    fn raw_html_byte_count_counts_raw_html_only() {
        assert_eq!(
            raw_html_byte_count("# clean\n\nplain **markdown** only\n"),
            0
        );
        // Block and inline raw HTML are both counted.
        let n = raw_html_byte_count("text <span>x</span> more\n\n<div>block</div>\n");
        assert!(n > 0, "expected raw HTML to be counted, got {n}");
    }

    // ---- scheme / target classification ----------------------------------

    #[test]
    fn has_scheme_distinguishes_absolute_from_relative() {
        assert!(has_scheme("http://example.com"));
        assert!(has_scheme("https://example.com"));
        assert!(has_scheme("javascript:alert(1)"));
        assert!(has_scheme("data:image/png;base64,AAAA"));
        assert!(has_scheme("mailto:a@b.c"));
        assert!(!has_scheme("maps/spielburg.png"));
        assert!(!has_scheme("02-mid-game.md"));
        assert!(!has_scheme("./hints/boss.md"));
        assert!(!has_scheme("#anchor"));
        assert!(!has_scheme(""));
    }

    #[test]
    fn markdown_target_detects_md_paths() {
        assert!(is_markdown_target("02-mid-game.md"));
        assert!(is_markdown_target("hints/boss.MD"));
        assert!(is_markdown_target("page.md#section"));
        assert!(!is_markdown_target("maps/spielburg.png"));
        assert!(!is_markdown_target("notes.txt"));
    }

    // ---- feature coverage (Task 2.1) -------------------------------------

    #[test]
    fn renders_core_markdown_features() {
        let out = render(
            "# Title\n\nSome **bold** and *italic* and `code`.\n\n- a\n- b\n\n> quote\n\n```\nfenced\n```\n",
        );
        assert!(out.contains("<h1>Title</h1>"), "{out}");
        assert!(out.contains("<strong>bold</strong>"), "{out}");
        assert!(out.contains("<em>italic</em>"), "{out}");
        assert!(out.contains("<code>code</code>"), "{out}");
        assert!(out.contains("<ul>") && out.contains("<li>a</li>"), "{out}");
        assert!(out.contains("<blockquote>"), "{out}");
        assert!(out.contains("<pre><code>fenced"), "{out}");
    }

    #[test]
    fn renders_tables() {
        let out = render("| A | B |\n|---|---|\n| 1 | 2 |\n");
        assert!(out.contains("<table>"), "{out}");
        assert!(out.contains("<th>A</th>"), "{out}");
        assert!(out.contains("<td>1</td>"), "{out}");
    }

    #[test]
    fn renders_footnotes() {
        // The strict sanitizer strips the relative `#fn` jump-links and ids, so
        // footnotes degrade to inline text — but the note's content survives.
        let out = render("Text with a note[^1].\n\n[^1]: The note.\n");
        assert!(out.contains("The note."), "{out}");
    }

    #[test]
    fn renders_task_lists_with_forced_checkbox() {
        let out = render("- [x] done\n- [ ] todo\n");
        assert!(out.contains("type=\"checkbox\""), "{out}");
        assert!(out.contains("checked"), "{out}");
        // No other input types survive.
        assert!(!out.contains("type=\"text\""), "{out}");
    }

    #[test]
    fn renders_strikethrough() {
        let out = render("~~gone~~\n");
        assert!(out.contains("<del>gone</del>"), "{out}");
    }

    // ---- URL rewriting (Task 2.4) ----------------------------------------

    #[test]
    fn relative_image_src_rewritten_to_content_protocol() {
        let out = render("![map](maps/spielburg.png)\n");
        assert!(
            out.contains("src=\"reliquaint-content://core/qfg1-ega/maps/spielburg.png\""),
            "{out}"
        );
    }

    #[test]
    fn dot_slash_prefix_trimmed_in_rewrite() {
        let out = render("![map](./maps/spielburg.png)\n");
        assert!(
            out.contains("src=\"reliquaint-content://core/qfg1-ega/maps/spielburg.png\""),
            "{out}"
        );
    }

    #[test]
    fn relative_md_link_rewritten_to_nav_protocol() {
        let out = render("[next section](02-mid-game.md)\n");
        assert!(
            out.contains("href=\"reliquaint-nav://core/qfg1-ega/02-mid-game.md\""),
            "{out}"
        );
    }

    #[test]
    fn external_links_pass_through() {
        let out = render("[site](https://example.com/page)\n");
        assert!(out.contains("href=\"https://example.com/page\""), "{out}");
    }

    // ---- XSS corpus (Task 2.2) — documented threat model -----------------
    // Each input below is a known injection vector; every assertion confirms
    // the dangerous payload does not survive to the webview.

    #[test]
    fn raw_script_tag_is_escaped_not_emitted() {
        let out = render("Before <script>alert(1)</script> after\n");
        assert!(!out.contains("<script"), "script tag leaked: {out}");
        assert!(
            out.contains("&lt;script&gt;"),
            "expected escaped text: {out}"
        );
    }

    #[test]
    fn raw_img_onerror_is_neutralized() {
        // Raw HTML is escaped to inert text — no live <img> element, so the
        // onerror handler never attaches even though the literal text remains.
        let out = render("<img src=x onerror=alert(1)>\n");
        assert!(!out.contains("<img"), "live img tag leaked: {out}");
        assert!(out.contains("&lt;img"), "expected escaped text: {out}");
    }

    #[test]
    fn raw_iframe_is_neutralized() {
        let out = render("<iframe src=\"https://evil.example\"></iframe>\n");
        assert!(!out.contains("<iframe"), "iframe leaked: {out}");
    }

    #[test]
    fn raw_svg_onload_is_neutralized() {
        let out = render("<svg onload=alert(1)></svg>\n");
        assert!(!out.contains("<svg"), "live svg tag leaked: {out}");
        assert!(out.contains("&lt;svg"), "expected escaped text: {out}");
    }

    #[test]
    fn raw_style_block_is_neutralized() {
        let out = render("<style>body{display:none}</style>\n");
        assert!(!out.contains("<style"), "style block leaked: {out}");
    }

    #[test]
    fn javascript_link_scheme_is_stripped() {
        let out = render("[click](javascript:alert(1))\n");
        assert!(!out.contains("javascript:"), "js scheme leaked: {out}");
    }

    #[test]
    fn remote_image_src_is_dropped() {
        let out = render("![pixel](https://tracker.example/p.gif)\n");
        assert!(
            !out.contains("tracker.example"),
            "remote image leaked: {out}"
        );
        assert!(!out.contains("https://tracker"), "{out}");
    }

    #[test]
    fn data_uri_image_is_dropped() {
        let out = render("![x](data:image/png;base64,iVBORw0KGgo=)\n");
        assert!(!out.contains("data:image"), "data uri leaked: {out}");
    }

    #[test]
    fn file_uri_link_is_stripped() {
        let out = render("[secret](file:///etc/passwd)\n");
        assert!(!out.contains("file:"), "file scheme leaked: {out}");
        assert!(!out.contains("/etc/passwd"), "{out}");
    }

    #[test]
    fn raw_styled_paragraph_does_not_produce_a_live_styled_tag() {
        // A raw <p style=…> is escaped wholesale, so no live styled element is
        // ever emitted (the only path to style= would be raw HTML passthrough).
        let out = render("<p style=\"position:fixed\">x</p>\n");
        assert!(!out.contains("<p style"), "live styled tag leaked: {out}");
        assert!(out.contains("&lt;p"), "expected escaped text: {out}");
    }
}
