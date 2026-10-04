//! Rendering an mdast tree to HTML, plus syntax highlighting of code blocks.
//!
//! This exists because the `markdown` crate exposes no way to render a
//! `mdast::Node` back to HTML: `to_html_with_options` takes a `&str`, and the
//! event-stream compiler that actually emits HTML sits behind a private
//! module. So the conversion is done here, against the tree.

/// Link reference definitions and footnote definitions, hoisted out of the
/// tree before rendering.
///
/// Both are stored behind an `Rc` so that descending into a subtree is just a
/// pointer copy — the renderer threads a separate `tight` flag for the one bit
/// of state that actually changes as it descends.
#[derive(Default)]
pub struct RenderContext {
    definitions: std::rc::Rc<std::collections::HashMap<String, (String, Option<String>)>>,
    footnotes: std::rc::Rc<std::collections::HashMap<String, Vec<markdown::mdast::Node>>>,
}

impl RenderContext {
    fn new(root: &markdown::mdast::Node) -> Self {
        let mut definitions = std::collections::HashMap::new();
        let mut footnotes = std::collections::HashMap::new();
        collect_references(root, &mut definitions, &mut footnotes);
        RenderContext {
            definitions: std::rc::Rc::new(definitions),
            footnotes: std::rc::Rc::new(footnotes),
        }
    }
}

/// `Definition` and `FootnoteDefinition` render to nothing where they sit in
/// the tree, so we pull them out up front and look them up by identifier when
/// we hit a reference.
pub fn collect_references(
    node: &markdown::mdast::Node,
    definitions: &mut std::collections::HashMap<String, (String, Option<String>)>,
    footnotes: &mut std::collections::HashMap<String, Vec<markdown::mdast::Node>>,
) {
    use markdown::mdast::Node as N;
    match node {
        N::Definition(n) => {
            // The first definition of an identifier wins; later duplicates are
            // ignored, per CommonMark.
            definitions
                .entry(n.identifier.clone())
                .or_insert_with(|| (n.url.clone(), n.title.clone()));
        }
        N::FootnoteDefinition(n) => {
            footnotes.entry(n.identifier.clone()).or_insert_with(|| n.children.clone());
        }
        _ => {}
    }
    for child in children(node) {
        collect_references(child, definitions, footnotes);
    }
}

/// `Node` has no generic accessor for its children, so every arm that can hold
/// children has to be listed here.
pub fn children(node: &markdown::mdast::Node) -> &[markdown::mdast::Node] {
    use markdown::mdast::Node as N;
    match node {
        N::Root(n) => &n.children,
        N::Paragraph(n) => &n.children,
        N::Heading(n) => &n.children,
        N::Blockquote(n) => &n.children,
        N::List(n) => &n.children,
        N::ListItem(n) => &n.children,
        N::Emphasis(n) => &n.children,
        N::Strong(n) => &n.children,
        N::Link(n) => &n.children,
        N::LinkReference(n) => &n.children,
        N::FootnoteDefinition(n) => &n.children,
        N::Table(n) => &n.children,
        N::TableRow(n) => &n.children,
        N::TableCell(n) => &n.children,
        N::Delete(n) => &n.children,
        N::MdxJsxFlowElement(n) => &n.children,
        N::MdxJsxTextElement(n) => &n.children,
        _ => &[],
    }
}

/// Escape the characters `CommonMark` considers dangerous. Matches what the
/// `markdown` crate's own compiler does, so our output is byte-identical for
/// the constructs we both implement.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\0' => out.push('\u{fffd}'),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn render_all(nodes: &[markdown::mdast::Node], ctx: &RenderContext, tight: bool) -> String {
    nodes.iter().map(|n| render(n, ctx, tight)).collect()
}

pub fn render_children(node: &markdown::mdast::Node, ctx: &RenderContext, tight: bool) -> String {
    render_all(children(node), ctx, tight)
}

pub fn title_attr(title: &Option<String>) -> String {
    match title {
        Some(t) => format!(" title=\"{}\"", escape(t)),
        None => String::new(),
    }
}

pub fn align_attr(align: &[markdown::mdast::AlignKind], column: usize) -> &'static str {
    use markdown::mdast::AlignKind as A;
    match align.get(column) {
        Some(A::Left) => " align=\"left\"",
        Some(A::Right) => " align=\"right\"",
        Some(A::Center) => " align=\"center\"",
        _ => "",
    }
}

/// Render a single mdast node to HTML.
///
/// `tight` is true when we're inside a tight list item, where `<p>` wrappers
/// around paragraphs are suppressed.
///
/// This replaces `markdown::to_html_with_options`, which can't be used here:
/// it takes a `&str`, and the crate exposes no way to render a `Node` back to
/// HTML. The node types reachable with `ParseOptions::default()` are the
/// CommonMark ones — `frontmatter`, `math`, and MDX are all off by default —
/// but the remaining arms are handled anyway so adding a construct to
/// `ParseOptions` can't silently produce empty output.
pub fn render(node: &markdown::mdast::Node, ctx: &RenderContext, tight: bool) -> String {
    use markdown::mdast::Node as N;
    match node {
        N::Root(n) => render_all(&n.children, ctx, tight),

        N::Paragraph(n) => {
            let inner = render_all(&n.children, ctx, tight);
            if tight {
                inner
            } else {
                format!("<p>{}</p>", inner)
            }
        }

        N::Heading(n) => format!(
            "<h{depth}>{inner}</h{depth}>",
            depth = n.depth,
            inner = render_all(&n.children, ctx, tight)
        ),

        N::ThematicBreak(_) => "<hr />".to_string(),

        N::Blockquote(n) => format!(
            "<blockquote>{}</blockquote>",
            render_all(&n.children, ctx, false)
        ),

        N::List(n) => {
            // A list is loose — its item paragraphs get `<p>` wrappers — if the
            // list itself is spread *or* any of its items is. The second half
            // matters: `1. a\n\n   b\n2. c` has `List.spread == false`, but
            // because its first item holds two blank-line-separated blocks,
            // CommonMark loosens the whole list, including the second item.
            let loose = n.spread
                || n.children.iter().any(|child| match child {
                    N::ListItem(item) => item.spread,
                    _ => false,
                });
            let (open, close) = if n.ordered {
                match n.start {
                    Some(start) if start != 1 => (format!("<ol start=\"{start}\">"), "</ol>"),
                    _ => ("<ol>".to_string(), "</ol>"),
                }
            } else {
                ("<ul>".to_string(), "</ul>")
            };
            format!(
                "{open}{inner}{close}",
                inner = render_all(&n.children, ctx, !loose)
            )
        }

        N::ListItem(n) => {
            // The list's own tightness isn't sufficient: an item holding two
            // blank-line-separated blocks sets `ListItem.spread` even when its
            // list is tight, and those blocks must still get `<p>` wrappers.
            let item_tight = tight && !n.spread;
            // A task list item carries its checkbox separately from the item's
            // children, so it goes at the front of the first paragraph.
            let checkbox = match n.checked {
                Some(true) => "<input type=\"checkbox\" disabled=\"\" checked=\"\" />",
                Some(false) => "<input type=\"checkbox\" disabled=\"\" />",
                None => "",
            };
            let mut inner = render_all(&n.children, ctx, item_tight);
            if !checkbox.is_empty() {
                inner = match inner.strip_prefix("<p>") {
                    Some(stripped) => format!("<p>{checkbox}{stripped}"),
                    None => format!("{checkbox}{inner}"),
                };
            }
            format!("<li>{inner}</li>")
        }

        N::Code(n) => {
            let open = match &n.lang {
                Some(lang) => format!("<pre><code class=\"language-{}\">", escape(lang)),
                None => "<pre><code>".to_string(),
            };
            format!("{open}{}</code></pre>", escape(&n.value))
        }

        N::InlineCode(n) => format!("<code>{}</code>", escape(&n.value)),

        N::Text(n) => escape(&n.value),

        N::Emphasis(n) => format!("<em>{}</em>", render_all(&n.children, ctx, tight)),
        N::Strong(n) => format!("<strong>{}</strong>", render_all(&n.children, ctx, tight)),
        N::Delete(n) => format!("<del>{}</del>", render_all(&n.children, ctx, tight)),

        N::Break(_) => "<br />".to_string(),

        N::Link(n) => format!(
            "<a href=\"{url}\"{title}>{inner}</a>",
            url = markdown::sanitize(&n.url),
            title = title_attr(&n.title),
            inner = render_all(&n.children, ctx, tight)
        ),

        N::Image(n) => format!(
            "<img src=\"{url}\" alt=\"{alt}\"{title} />",
            url = markdown::sanitize(&n.url),
            alt = escape(&n.alt),
            title = title_attr(&n.title)
        ),

        // A reference with no matching definition renders as its contents alone
        // rather than as a broken link, matching the spec.
        N::LinkReference(n) => match ctx.definitions.get(&n.identifier) {
            Some((url, title)) => format!(
                "<a href=\"{url}\"{title}>{inner}</a>",
                url = markdown::sanitize(url),
                title = title_attr(title),
                inner = render_all(&n.children, ctx, tight)
            ),
            None => render_all(&n.children, ctx, tight),
        },

        N::ImageReference(n) => match ctx.definitions.get(&n.identifier) {
            Some((url, title)) => format!(
                "<img src=\"{url}\" alt=\"{alt}\"{title} />",
                url = markdown::sanitize(url),
                alt = escape(&n.alt),
                title = title_attr(title)
            ),
            None => escape(&n.alt),
        },

        N::Table(n) => {
            let mut rows = n.children.iter();
            let mut out = String::from("<table>");
            // The first row is the header, and gets `<th>` in a `<thead>`.
            if let Some(head) = rows.next() {
                out.push_str("<thead><tr>");
                out.push_str(&render_cells(head, &n.align, true, ctx));
                out.push_str("</tr></thead>");
            }
            out.push_str("<tbody>");
            for row in rows {
                out.push_str("<tr>");
                out.push_str(&render_cells(row, &n.align, false, ctx));
                out.push_str("</tr>");
            }
            out.push_str("</tbody></table>");
            out
        }

        N::TableRow(_) => String::new(), // Rendered by the parent table.
        N::TableCell(_) => String::new(), // Rendered by the parent row.

        // Hoisted into the context; emits nothing where it sits.
        N::Definition(_) => String::new(),

        N::Html(n) => n.value.clone(),

        // Below here: unreachable with `ParseOptions::default()`. Rendered
        // plainly rather than dropped, so enabling a construct later degrades
        // to "unstyled but visible" instead of "silently gone".
        N::FootnoteReference(n) => match ctx.footnotes.get(&n.identifier) {
            Some(body) => format!(
                "<sup data-footnote=\"{id}\">{id}</sup><div data-footnote-body=\"{id}\">{body}</div>",
                id = escape(&n.identifier),
                body = render_all(body, ctx, false)
            ),
            None => String::new(),
        },

        N::FootnoteDefinition(_) => String::new(),
        N::Math(n) => format!(
            "<pre><code class=\"language-math math-display\">{}</code></pre>",
            escape(&n.value)
        ),
        N::InlineMath(n) => format!(
            "<code class=\"language-math math-inline\">{}</code>",
            escape(&n.value)
        ),
        N::Yaml(_) | N::Toml(_) | N::MdxjsEsm(_) => String::new(),
        N::MdxTextExpression(n) => escape(&n.value),
        N::MdxFlowExpression(n) => escape(&n.value),
        N::MdxJsxFlowElement(n) => format!(
            "<{name}>{inner}</{name}>",
            name = n.name.clone().unwrap_or_else(|| "div".to_string()),
            inner = render_all(&n.children, ctx, tight)
        ),
        N::MdxJsxTextElement(n) => format!(
            "<{name}>{inner}</{name}>",
            name = n.name.clone().unwrap_or_else(|| "span".to_string()),
            inner = render_all(&n.children, ctx, tight)
        ),
    }
}

/// A `TableRow`'s children are its cells, which take `<th>`/`<td>` plus the
/// table's column alignment.
pub fn render_cells(
    row: &markdown::mdast::Node,
    align: &[markdown::mdast::AlignKind],
    head: bool,
    ctx: &RenderContext,
) -> String {
    children(row)
        .iter()
        .enumerate()
        .map(|(column, cell)| {
            format!(
                "<{tag}{align}>{inner}</{tag}>",
                tag = if head { "th" } else { "td" },
                align = align_attr(align, column),
                inner = render_children(cell, ctx, false)
            )
        })
        .collect()
}

pub fn compile_html_content_tree(html_content_tree: &markdown::mdast::Node) -> String {
    let ctx = RenderContext::new(html_content_tree);
    render(html_content_tree, &ctx, false)
}

// --- syntax highlighting -------------------------------------------------

use std::sync::OnceLock;

pub struct Highlighter {
    syntaxes: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::Theme,
}

pub fn highlighter() -> &'static Highlighter {
    static HIGHLIGHTER: OnceLock<Highlighter> = OnceLock::new();
    HIGHLIGHTER.get_or_init(|| {
        let syntaxes = syntect::parsing::SyntaxSet::load_defaults_newlines();
        let mut themes = syntect::highlighting::ThemeSet::load_defaults();
        let theme = themes
            .themes
            .remove("base16-ocean.dark")
            .or_else(|| themes.themes.values().next().cloned())
            .expect("syntect's default theme set is empty");
        Highlighter { syntaxes, theme }
    })
}

pub fn style_to_css(style: syntect::highlighting::Style) -> String {
    use syntect::highlighting::FontStyle;
    let fg = style.foreground;
    let mut css = format!("color:#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);
    // Background is intentionally left to the page, so a dark theme's
    // background doesn't paint over the surrounding stylesheet.
    if style.font_style.contains(FontStyle::BOLD) {
        css.push_str(";font-weight:bold");
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        css.push_str(";font-style:italic");
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        css.push_str(";text-decoration:underline");
    }
    css
}

/// Highlight `value` as `lang`, returning the inner HTML of the `<code>`
/// element (already escaped, wrapped in styled spans). Returns `None` if
/// syntect doesn't know the language, so the caller can fall back to plain
/// escaped text.
pub fn highlight_code(value: &str, lang: &str) -> Option<String> {
    use syntect::easy::HighlightLines;

    let highlighter = highlighter();
    let syntax = highlighter
        .syntaxes
        .find_syntax_by_token(lang)
        .or_else(|| highlighter.syntaxes.find_syntax_by_extension(lang))?;
    let mut lines = HighlightLines::new(syntax, &highlighter.theme);

    let mut out = String::new();
    for line in value.split_inclusive('\n') {
        let regions = lines.highlight_line(line, &highlighter.syntaxes).ok()?;
        for (style, piece) in regions {
            out.push_str(&format!(
                "<span style=\"{}\">{}</span>",
                style_to_css(style),
                escape(piece)
            ));
        }
    }
    Some(out)
}

/// Walk the tree and replace every `Code` block we can highlight with the
/// equivalent `Html` node.
///
/// Replacing with `Html` rather than carrying highlighted markup on `Code`
/// itself is the only option: `Code` holds plain text, and the renderer would
/// escape it. `Html` passes through verbatim, so the spans reach the browser
/// intact. Blocks with no recognisable language are left alone, so the
/// renderer still emits them normally (escaped, with `class="language-…"`).
pub fn apply_syntax_highlighting(html_content_tree: &mut markdown::mdast::Node) {
    use markdown::mdast::Node as N;

    if let N::Code(n) = html_content_tree {
        let Some(lang) = n.lang.clone() else { return };
        let Some(highlighted) = highlight_code(&n.value, &lang) else { return };
        *html_content_tree = N::Html(markdown::mdast::Html {
            value: format!(
                "<pre><code class=\"language-{lang}\">{highlighted}</code></pre>",
                lang = escape(&lang)
            ),
            // The position pointed into the source we've just replaced.
            position: None,
        });
        return;
    }

    // Leaves and other childless nodes return `None`, so the loop is skipped
    // for them rather than needing a match arm of their own.
    if let Some(kids) = children_mut(html_content_tree) {
        for child in kids.iter_mut() {
            apply_syntax_highlighting(child);
        }
    }
}

/// The mutable counterpart to [`children`]. `None` for node types that can't
/// hold children. (An `&mut Vec::new()` would be a temporary dropped at the
/// end of the expression, and a `static` can't be borrowed mutably.)
pub fn children_mut(node: &mut markdown::mdast::Node) -> Option<&mut Vec<markdown::mdast::Node>> {
    use markdown::mdast::Node as N;
    match node {
        N::Root(n) => Some(&mut n.children),
        N::Paragraph(n) => Some(&mut n.children),
        N::Heading(n) => Some(&mut n.children),
        N::Blockquote(n) => Some(&mut n.children),
        N::List(n) => Some(&mut n.children),
        N::ListItem(n) => Some(&mut n.children),
        N::Emphasis(n) => Some(&mut n.children),
        N::Strong(n) => Some(&mut n.children),
        N::Link(n) => Some(&mut n.children),
        N::LinkReference(n) => Some(&mut n.children),
        N::FootnoteDefinition(n) => Some(&mut n.children),
        N::Table(n) => Some(&mut n.children),
        N::TableRow(n) => Some(&mut n.children),
        N::TableCell(n) => Some(&mut n.children),
        N::Delete(n) => Some(&mut n.children),
        N::MdxJsxFlowElement(n) => Some(&mut n.children),
        N::MdxJsxTextElement(n) => Some(&mut n.children),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference implementation we're replacing. `allow_dangerous_html` is
    /// on because content/ uses raw HTML (the old pipeline enabled it too), and
    /// dangerous protocols are on to match `allow_dangerous_protocol: true`.
    fn reference(md: &str) -> String {
        markdown::to_html_with_options(md, &markdown::Options {
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                allow_dangerous_protocol: true,
                ..markdown::CompileOptions::default()
            },
            ..markdown::Options::default()
        })
        .expect("reference compile failed")
    }

    /// Our renderer concatenates block elements with no separator; the crate
    /// emits a line ending between them. Normalise that away so the two are
    /// comparable on structure and escaping rather than whitespace.
    fn squash(html: &str) -> String {
        html.chars().filter(|c| !c.is_whitespace()).collect()
    }

    fn assert_matches_reference(md: &str) {
        let tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        let ours = compile_html_content_tree(&tree);
        let theirs = reference(md);
        assert_eq!(
            squash(&ours),
            squash(&theirs),
            "\n--- input ---\n{md}\n--- ours ---\n{ours}\n--- theirs ---\n{theirs}\n"
        );
    }

    #[test]
    fn matches_reference_on_commonmark_constructs() {
        let cases: &[&str] = &[
            "Hello world.",
            "# Heading one\n\n## Heading two",
            "Paragraph with *em*, **strong**, `code`, and [a link](https://example.com).",
            "![alt text](/img.png \"the title\")",
            "> A block quote\n>\n> with two paragraphs.",
            "- one\n- two\n- three",
            "1. one\n2. two",
            "5. five\n6. six",
            "* * *",
            "A line with  \ntwo spaces break.",
            "Auto <https://example.com> link.",
            "Escapes: \\*not em\\* and 5 * 3 = 15.",
            "Entities: &amp; &copy; &#35;",
            "  indented code block\n  second line",
            "```rust\nfn main() {}\n```",
            "```\nno language\n```",
            "```unknownlang\nbody\n```",
            "Text with <span>inline html</span> and a <br>.",
            "A URL with query: <https://example.com/?a=1&b=2>",
            "[ref link][a]\n\n[a]: /target \"T\"",
            "![ref img][a]\n\n[a]: /pic.png",
            "[collapsed][]\n\n[collapsed]: /c",
            "[shortcut]\n\n[shortcut]: /s",
            "A [dangling][missing] reference.",
            "<div class=\"block\">\n  <p>raw html block</p>\n</div>",
            "Trailing hard break\\\nnext line",
            "Ampersand in link: [x](/a?b=1&c=2&d=3)",
            "Quote in alt: ![a \"b\" c](/x.png)",
            "* * *\n\nAfter thematic break.",
            "Setext heading\n==============",
            "Nested list:\n\n- a\n  - b\n    - c",
            "Loose list:\n\n- a\n\n- b",
            "List with blank line then text\n\n- para one\n\n  para two",
            "1. a\n\n   b\n2. c",
            "- a\n  - b\n\n- c",
            "- > quoted\n- plain",
            "> - a\n> - b",
            "- item\n\n  > a quote in a loose item",
        ];
        for md in cases {
            assert_matches_reference(md);
        }
    }

    #[test]
    fn escapes_text_content() {
        let tree = markdown::to_mdast("5 < 6 && 7 > 6", &markdown::ParseOptions::default()).unwrap();
        assert_eq!(compile_html_content_tree(&tree), "<p>5 &lt; 6 &amp;&amp; 7 &gt; 6</p>");
    }

    #[test]
    fn escapes_code_block_content() {
        let tree = markdown::to_mdast("```\n<script>alert(1)</script>\n```", &markdown::ParseOptions::default()).unwrap();
        assert_eq!(
            compile_html_content_tree(&tree),
            "<pre><code>&lt;script&gt;alert(1)&lt;/script&gt;</code></pre>"
        );
    }

    #[test]
    fn highlights_a_known_language() {
        let md = "```rust\nfn main() {}\n```";
        let mut tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        apply_syntax_highlighting(&mut tree);
        let html = compile_html_content_tree(&tree);
        assert!(html.starts_with("<pre><code class=\"language-rust\">"), "{html}");
        // The highlighter should have produced styled spans, not escaped text.
        assert!(html.contains("<span style=\"color:#"), "{html}");
        assert!(!html.contains("&lt;"), "code was double-escaped: {html}");
    }

    #[test]
    fn leaves_unknown_languages_to_the_renderer() {
        // syntect doesn't know this, so the node stays a Code node and the
        // renderer handles it — still escaped, still carrying the class.
        let md = "```definitelynotalanguage\nx < y\n```";
        let mut tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        apply_syntax_highlighting(&mut tree);
        assert_eq!(
            compile_html_content_tree(&tree),
            "<pre><code class=\"language-definitelynotalanguage\">x &lt; y</code></pre>"
        );
    }

    /// The one CommonMark spec example our output does not match, pinned so
    /// the deviation is visible rather than discovered later.
    ///
    /// `to_mdast` collapses an image nested inside another image's alt text
    /// into the outer `alt` string, discarding the inner alt entirely:
    /// `![foo ![bar](/url)](/url2)` arrives as `alt: "foo "`, where CommonMark
    /// wants `foo bar`. The crate's own HTML compiler gets this right only
    /// because it walks the event stream, which retains the nesting. Since
    /// `mdast::Image` carries no children, there is nothing left in the tree to
    /// recover the lost text from — it would have to be re-read from the source
    /// via the node's `position`.
    #[test]
    fn known_deviation_nested_image_alt_text() {
        let md = "![foo ![bar](/url)](/url2)\n";
        let tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        assert_eq!(
            compile_html_content_tree(&tree),
            "<p><img src=\"/url2\" alt=\"foo \" /></p>"
        );
    }

    #[test]
    fn highlighting_leaves_prose_untouched() {
        let md = "Some **bold** text with a `span` inline.";
        let mut tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
        apply_syntax_highlighting(&mut tree);
        assert_eq!(
            compile_html_content_tree(&tree),
            reference(md).replace("\n", "").trim()
        );
    }
}
