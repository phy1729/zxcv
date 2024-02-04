use ego_tree::NodeRef;
use scraper::ElementRef;
use scraper::Html;
use scraper::Node;
use scraper::Selector;
use unicode_width::UnicodeWidthStr;
use url::Url;

mod escape_markdown;
mod squeeze_whitespace;
mod state;

use self::state::Block;
use self::state::State;

/// Return the single element matched by `selector` or `None` if there are zero or more than one
/// matches.
///
/// # Panics
///
/// It is the caller's responsibility to ensure the `selector` is valid.
pub(crate) fn select_single_element<'a>(
    tree: &'a Html,
    selector_string: &str,
) -> Option<ElementRef<'a>> {
    let selector = Selector::parse(selector_string).expect("Caller must supply a valid selector");
    let mut iter = tree.select(&selector).fuse();
    match (iter.next(), iter.next()) {
        (Some(element), None) => Some(element),
        _ => None,
    }
}

pub(crate) fn render(html: &str, url: &Url) -> String {
    let mut state = State::default();
    render_node_inner(
        *Html::parse_fragment(html).root_element(),
        url,
        &mut state.root_block(),
    );
    state.render()
}

fn render_node_inner(node: NodeRef<'_, Node>, url: &Url, block: &mut Block) {
    match node.value() {
        Node::Text(t) => block.push(t),

        Node::Element(e) => match e.name() {
            "b" | "strong" => {
                block.push_raw("**");
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
                block.push_raw("**");
            }

            "br" => block.newline(),

            "div" | "p" => {
                let mut block = block.new_block();
                node.children()
                    .for_each(|node| render_node_inner(node, url, &mut block));
            }

            "em" | "i" => {
                block.push_raw("_");
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
                block.push_raw("_");
            }

            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let mut sub_state = State::default();
                node.children()
                    .fold(&mut sub_state.root_block(), |block, node| {
                        render_node_inner(node, url, block);
                        block
                    });
                let header = sub_state.render();

                if !header.is_empty() {
                    let mut block = block.new_block();
                    match e.name() {
                        "h1" | "h2" => {
                            // Already escaped
                            block.push_raw(&header);
                            block.newline();
                            block.push_raw(
                                &(if e.name() == "h1" { "=" } else { "-" }).repeat(header.width()),
                            );
                        }
                        "h3" | "h4" | "h5" | "h6" => {
                            block.push_raw(match e.name() {
                                "h3" => "### ",
                                "h4" => "#### ",
                                "h5" => "##### ",
                                "h6" => "###### ",
                                _ => unreachable!(),
                            });
                            // Already escaped
                            block.push_raw(&header);
                        }
                        _ => unreachable!(),
                    }
                }
            }

            "img" => {
                if let Some(src) = e.attr("src") {
                    block.push_raw("![");
                    block.push(e.attr("alt").unwrap_or_default());
                    block.push_raw("](");
                    block.push_raw(
                        url.join(src)
                            .map(Into::<String>::into)
                            .as_deref()
                            .unwrap_or(src),
                    );
                    block.push_raw(")");
                }
            }

            _ => {
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
            }
        },

        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::render;

    macro_rules! render_tests {
        ($(($name: ident, $html: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(render($html, &Url::parse("https://example.com/").unwrap()), $expected);
                }
            )*
        }
    }

    render_tests!(
        (plain, "foo bar", "foo bar"),
        (escaped, "foo* bar_baz", "foo\\* bar\\_baz"),
        (whitespace_compress_spaces, "foo      bar", "foo bar"),
        (whitespace_compress_newlines, "foo\n\n  bar", "foo bar"),
        (whitespace_compress_tabs, "foo\t\t \tbar", "foo bar"),
        (whitespace_leading, "  foo bar", "foo bar"),
        (whitespace_trailing, "foo bar  ", "foo bar"),
        (whitespace_span_trailing, "<span>foo </span> bar", "foo bar"),
        (whitespace_span_middle, "<span>foo</span> <span>bar</span>", "foo bar"),
        (whitespace_zwsp, "foo <p>\u{200b}</p> bar", "foo\n\nbar"),
        (strong, "foo <strong>bar</strong> baz", "foo **bar** baz"),
        (br, "foo<br>bar", "foo\nbar"),
        (br_space, "foo<br> bar", "foo\nbar"),
        (br_space_span, "foo<br>\n<span>bar</span>", "foo\nbar"),
        (div, "<div>foo</div><div>bar</div>", "foo\n\nbar"),
        (p, "<p>foo</p><p>bar</p>", "foo\n\nbar"),
        (em, "foo <em>bar</em> baz", "foo _bar_ baz"),
        (header_h1, "<h1>header</h1>", "header\n======"),
        (header_ignore_empty, "<h1></h1>", ""),
        (header_escapes, "<h1>foo `bar` baz</h1>", "foo \\`bar\\` baz\n==============="),
        (header_h2, "<h2>header</h2>", "header\n------"),
        (header_h3, "<h3>header</h2>", "### header"),
        (img_escape_alt, "<img src=\"/foo.png\" alt=\"bar_baz\">", "![bar\\_baz](https://example.com/foo.png)"),
        (img_url_is_raw, "<img src=\"/foo_bar.png\" alt=\"baz\">", "![baz](https://example.com/foo_bar.png)"),
    );
}
