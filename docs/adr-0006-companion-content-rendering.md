# ADR-0006: Companion Content Rendering and Sandbox

## Status

Proposed.

## Context

v0.4 introduces companion content: per-game walkthroughs, maps, hint files, install notes, and similar supplementary material that taps carry alongside their catalog entries. This content is the project's preservation reach — the cultural artifacts around the games, not just the games themselves.

This content renders in the Tauri GUI. The webview shares context with the application, which creates a real security boundary issue: arbitrary Markdown from arbitrary tap maintainers cannot be allowed to execute scripts, leak data, or compromise the application. The launcher project takes no editorial position on what taps contain (per ADR-0003); the trust model is "users opt into taps they choose to subscribe to," but that does not extend to "tap maintainers can execute code in the user's launcher."

The threats worth addressing:

- **Script injection (XSS-equivalent).** Markdown allows raw HTML, which can carry `<script>` tags, event handlers (`onclick`, `onerror`), and similar.
- **Privacy leakage via remote resources.** Markdown image syntax can reference arbitrary URLs. A tap could include `![](https://tracker.example/pixel.gif)` that leaks the user's IP, user-agent, and viewing patterns.
- **Phishing via misleading links.** A link rendered with one URL but pointing at another can mislead users into trusting malicious destinations.
- **Filesystem access via path manipulation.** Image references with `..` or absolute paths could probe the user's filesystem if image resolution is naive.
- **Network leakage via CSS or other side channels.** External stylesheets, fonts, or background images can leak the same data as remote images.

Cheap to decide now, expensive to retrofit. The rendering pipeline is the kind of thing that calcifies fast once content starts depending on it.

## Decision

**Server-side Markdown rendering with strict HTML sanitization. No remote resources of any kind. All images served via a custom Tauri protocol that only resolves files within the tap's cache directory.**

### Rendering pipeline

- **Parse Markdown** with `pulldown-cmark` in the Rust backend. Parsing happens in the launcher process, not in the webview.
- **Sanitize the resulting HTML** with `ammonia` (whitelist approach). The webview receives only sanitized HTML.
- **No raw HTML in source Markdown.** `pulldown-cmark`'s raw-HTML pass-through is disabled. Markdown features only; if a tap maintainer needs richer formatting, that's a future ADR.
- **Sanitizer whitelist:** standard text formatting (`<p>`, `<em>`, `<strong>`, `<code>`, `<pre>`, `<blockquote>`), headings (`<h1>` through `<h6>`), lists (`<ul>`, `<ol>`, `<li>`), links (`<a>` with restricted `href` schemes), images (`<img>` with restricted `src` schemes), and tables. No `<script>`, no `<style>`, no `<iframe>`, no event handler attributes, no `style=` attributes, no `srcset`, no `<object>` or `<embed>`.

### Images

- **Tap-local only.** Images resolve via a custom Tauri protocol — `reliquaint-content://<tap-id>/<game-id>/<path>` — handled by the launcher.
- **Path traversal blocked.** The protocol handler validates that the resolved path stays within `<tap-cache>/companion/<game-id>/`. Any `..`, absolute paths, or symlinks that escape are rejected.
- **No remote image URLs.** The sanitizer rewrites `http://`, `https://`, `data:`, and `file:` `src` attributes to broken-image placeholders. Tap maintainers who want to include images include them in the tap.
- **Allowed image formats:** PNG, JPEG, GIF, WebP. **SVG explicitly excluded** — SVG is structured XML that can contain `<script>` and event handlers.

### Links

- **External links (`http://`, `https://`)** render, but the click handler intercepts navigation and opens the URL in the user's default browser via Tauri's `shell.open`. Webview navigation away from the app is blocked.
- **Intra-content links (relative paths within the same companion directory)** are allowed; clicking navigates to the linked content within the companion viewer.
- **All other schemes** (`javascript:`, `data:`, `file:`, `mailto:`, `ftp:`, custom schemes) are stripped by the sanitizer.

### Styling

- **Content has no influence on its own styling.** The launcher's stylesheet handles all presentation. The sanitizer strips `<style>` blocks, `style=` attributes, and `class=` attributes (the launcher's CSS targets element types and a small set of generated class names only).
- This is intentional. Predictable visual consistency across content from many taps matters more than per-tap stylistic expression.

## Consequences

**Positive**

- Tight security envelope. No tap, however hostile, can execute code, leak data, or escape the rendering boundary by submitting content alone.
- Predictable styling — users get a consistent reading experience regardless of which tap content comes from.
- Privacy preserved. No network traffic happens as a side effect of viewing companion content. Consistent with `PRIVACY.md`.
- Standard well-maintained crates. `pulldown-cmark` and `ammonia` are both widely used and security-audited.
- Server-side rendering means the heavy lifting happens in Rust where the launcher's existing logging, error handling, and tooling apply.

**Negative**

- Tap maintainers cannot use HTML for advanced layout. Walkthroughs and hints work fine as Markdown; richer interactive content needs to wait for a future expansion.
- SVG exclusion means tap maintainers preparing map images must rasterize them. A small but real constraint.
- Re-render on every view (no cached HTML output for v0.4) means very large content has a small rendering latency. Acceptable for typical walkthrough sizes; revisit if scanned-manual-sized content becomes common.

## Alternatives considered

**Iframe sandbox.** Render content inside an `<iframe sandbox>`. More complex to integrate with the rest of the GUI, doesn't materially reduce attack surface for our threat model (we still need sanitization), introduces styling and navigation complications. Rejected.

**Client-side rendering with `marked` or `markdown-it`.** Markdown parsing happens in the webview. Worse: the parser runs in the same context as the app, making any bug in the parser an app-level vulnerability. Rejected.

**Allow raw HTML with `<script>` stripped.** Sounds reasonable until you remember `<img onerror="...">`, `<a href="javascript:...">`, `<iframe>`, and a hundred other vectors. Strict whitelist is the only safe default. Rejected.

**Allow remote images with a domain allowlist.** Useful for tap maintainers but starts an unbounded policy-maintenance problem. Privacy implications are unclear to users. Rejected; tap-local images are the right answer.

**Allow SVG.** SVG is great for diagrams and maps, but it's a script-execution surface. The marginal value of SVG support does not outweigh the cost of policing it. PNG/WebP rasterization is sufficient. Rejected.

## Open follow-ups

- **PDF rendering.** Scanned manuals are obvious preservation content. Renders securely with `pdfium` or similar. Likely its own follow-up ADR; not blocking v0.4.
- **Video and audio content.** Out of scope until there's clear demand.
- **Allow-listed HTML for trusted taps.** If a future need emerges for richer formatting in `reliquaint-core` or other curated taps, a per-tap trust signal could enable a relaxed sanitizer. Speculative; not building it now.
- **Cached rendered HTML.** A performance optimization if companion content rendering becomes a bottleneck. Trivial to add later, not needed for v0.4.
