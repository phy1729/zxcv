use scraper::ElementRef;
use scraper::Html;
use scraper::Node;
use scraper::Selector;

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
    Html::parse_fragment(html)
        .root_element()
        .descendants()
        .filter_map(|e| match e.value() {
            Node::Text(t) => Some(&**t),
            Node::Element(e) if e.name() == "br" => Some("\n"),
            Node::Element(e) if e.name() == "p" => Some("\n\n"),
            _ => None,
        })
        .skip_while(|&s| s == "\n\n")
        .collect::<String>()
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
        (br, "foo<br>bar", "foo\nbar"),
        (p, "<p>foo</p><p>bar</p>", "foo\n\nbar"),
    );
}
