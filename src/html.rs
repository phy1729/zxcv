use ego_tree::NodeRef;
use scraper::ElementRef;
use scraper::Html;
use scraper::Node;
use scraper::Selector;

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

pub(crate) fn render(html: &str) -> String {
    let mut state = State::default();
    render_node_inner(
        *Html::parse_fragment(html).root_element(),
        &mut state.root_block(),
    );
    state.render()
}

fn render_node_inner(node: NodeRef<'_, Node>, block: &mut Block) {
    match node.value() {
        Node::Text(t) => block.push(t),

        Node::Element(e) => match e.name() {
            "br" => block.newline(),

            "div" | "p" => {
                let mut block = block.new_block();
                node.children()
                    .for_each(|node| render_node_inner(node, &mut block));
            }

            _ => {
                node.children()
                    .for_each(|node| render_node_inner(node, block));
            }
        },

        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::render;

    macro_rules! render_tests {
        ($(($name: ident, $html: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(render($html), $expected);
                }
            )*
        }
    }

    render_tests!(
        (plain, "foo bar", "foo bar"),
        (whitespace_compress_spaces, "foo      bar", "foo bar"),
        (whitespace_compress_newlines, "foo\n\n  bar", "foo bar"),
        (whitespace_compress_tabs, "foo\t\t \tbar", "foo bar"),
        (whitespace_leading, "  foo bar", "foo bar"),
        (whitespace_trailing, "foo bar  ", "foo bar"),
        (whitespace_span_trailing, "<span>foo </span> bar", "foo bar"),
        (whitespace_span_middle, "<span>foo</span> <span>bar</span>", "foo bar"),
        (br, "foo<br>bar", "foo\nbar"),
        (br_space, "foo<br> bar", "foo\nbar"),
        (br_space_span, "foo<br>\n<span>bar</span>", "foo\nbar"),
        (div, "<div>foo</div><div>bar</div>", "foo\n\nbar"),
        (p, "<p>foo</p><p>bar</p>", "foo\n\nbar"),
    );
}
