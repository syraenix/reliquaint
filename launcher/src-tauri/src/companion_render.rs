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

use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};

/// Render a companion Markdown document to sanitized, webview-safe HTML.
///
/// `tap_id` and `game_id` scope the rewriting of relative image and link
/// targets to the document's owning tap and game.
pub fn render_markdown(md: &str, tap_id: &str, game_id: &str) -> String {
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
    sanitize(&html_out)
}

/// Rewrite an image `src`. Relative paths resolve to the tap-local image
/// protocol; anything with a URL scheme (remote `http(s)`, `data:`, `file:`)
/// is dropped to an empty `src` so it cannot leak the user's activity or probe
/// the filesystem (ADR-0006: no remote resources).
fn rewrite_image_dest<'a>(dest: CowStr<'a>, tap_id: &str, game_id: &str) -> CowStr<'a> {
    if has_scheme(&dest) {
        CowStr::from("")
    } else {
        let rel = dest.trim_start_matches("./");
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
        let rel = dest.trim_start_matches("./");
        CowStr::from(format!("reliquaint-nav://{tap_id}/{game_id}/{rel}"))
    } else {
        dest
    }
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
