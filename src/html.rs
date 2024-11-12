use std::borrow::Cow;

use ego_tree::NodeRef;
use scraper::node::Element;
use scraper::ElementRef;
use scraper::Html;
use scraper::Node;
use scraper::Selector;
use unicode_width::UnicodeWidthStr;
use url::Url;

use crate::LINE_LENGTH;

mod escape_markdown;
mod squeeze_whitespace;
mod state;

use self::state::Block;
use self::state::State;

pub(crate) trait Selectable {
    fn select<'a, 'b>(&'a self, selector: &'b Selector) -> impl Iterator<Item = ElementRef<'a>>;
}

impl Selectable for Html {
    fn select<'a, 'b>(&'a self, selector: &'b Selector) -> impl Iterator<Item = ElementRef<'a>> {
        self.select(selector)
    }
}

impl Selectable for ElementRef<'_> {
    fn select<'a, 'b>(&'a self, selector: &'b Selector) -> impl Iterator<Item = ElementRef<'a>> {
        self.select(selector)
    }
}

/// Return the single element matched by `selector` or `None` if there are zero or more than one
/// matches.
///
/// # Panics
///
/// It is the caller's responsibility to ensure the `selector` is valid.
pub(crate) fn select_single_element<'a>(
    tree: &'a impl Selectable,
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
    render_node(*Html::parse_fragment(html).root_element(), url)
}

pub(crate) fn render_node(node: NodeRef<'_, Node>, url: &Url) -> String {
    let mut state = State::default();
    render_node_inner(node, url, &mut state.root_block());
    state.render()
}

#[allow(clippy::too_many_lines)]
fn render_node_inner(node: NodeRef<'_, Node>, url: &Url, block: &mut Block) {
    match node.value() {
        Node::Text(t) => block.push(t),

        Node::Element(e) => match e.name() {
            "a" => {
                if let Some(link) = e.attr("href") {
                    let mut sub_state = State::default();
                    node.children()
                        .fold(&mut sub_state.root_block(), |block, node| {
                            render_node_inner(node, url, block);
                            block
                        });
                    let text = sub_state.render();

                    let destination: Option<Cow<str>> = match url.join(link) {
                        Ok(abs_link) => {
                            let is_anchor = url
                                .make_relative(&abs_link)
                                .map(|u| u.is_empty() || u.starts_with('#'))
                                == Some(true);
                            if !is_anchor
                                || text.chars().count() > if text.starts_with('\\') { 2 } else { 1 }
                            {
                                Some(Into::<String>::into(abs_link).into())
                            } else {
                                None
                            }
                        }
                        Err(_) => Some(link.into()),
                    };

                    if let Some(destination) = destination {
                        block.push_raw("[");
                        // Already escaped
                        block.push_raw(&text);
                        block.push_raw("](");
                        block.push_raw(&destination);
                        block.push_raw(")");
                    }
                }
            }

            "b" | "strong" => {
                block.push_raw_start("**");
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
                block.push_raw_end("**");
            }

            "blockquote" => {
                let mut block = block.new_block();
                block.prefix("> ", "> ");
                node.children()
                    .for_each(|node| render_node_inner(node, url, &mut block));
            }

            "br" => block.newline(),

            "code" => {
                block.push_raw_start("`");
                block.start_code();
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
                block.end_code();
                block.push_raw_end("`");
            }

            "div" | "p" => {
                let mut block = block.new_block();
                node.children()
                    .for_each(|node| render_node_inner(node, url, &mut block));
            }

            "em" | "i" => {
                block.push_raw_start("_");
                node.children()
                    .for_each(|node| render_node_inner(node, url, block));
                block.push_raw_end("_");
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
                                &(if e.name() == "h1" { "=" } else { "-" })
                                    .repeat(std::cmp::min(header.width(), LINE_LENGTH)),
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

            "ol" => {
                let child_count = node
                    .children()
                    .filter(|n| n.value().as_element().map(Element::name) == Some("li"))
                    .count();
                if child_count != 0 {
                    let num_width: usize = child_count.ilog10() as usize + 1;
                    let subsequent_prefix = format!("{:num_width$}  ", "");

                    let mut block = block.new_block();
                    let mut item_count = 0;
                    node.children()
                        .filter(|n| n.value().as_element().map(Element::name) == Some("li"))
                        .for_each(|node| {
                            item_count += 1;
                            let initial_prefix = format!("{item_count:num_width$}. ");
                            let mut item_block = block.new_item();
                            item_block.prefix(&initial_prefix, &subsequent_prefix);
                            item_block.must_emit();
                            render_node_inner(node, url, &mut item_block);
                        });
                }
            }

            "pre" => {
                let mut block = block.new_raw_block();
                block.push("```");
                if let Some(lang) = select_single_element(
                    &ElementRef::wrap(node).expect("node is Node::Element"),
                    "code",
                )
                .and_then(|c| c.attr("class"))
                .and_then(|c| c.split(' ').find_map(|x| x.strip_prefix("language-")))
                {
                    block.push(lang);
                }
                block.newline();

                node.descendants().for_each(|n| match n.value() {
                    Node::Element(e) if e.name() == "br" => block.newline(),
                    Node::Text(t) => block.push(t),
                    _ => {}
                });
                block.ensure_newline();
                block.push("```");
            }

            "ul" => {
                let mut block = block.new_block();
                node.children()
                    .filter(|n| n.value().as_element().map(Element::name) == Some("li"))
                    .for_each(|node| {
                        let mut item_block = block.new_item();
                        item_block.prefix("* ", "  ");
                        item_block.must_emit();
                        render_node_inner(node, url, &mut item_block);
                    });
            }

            "script" | "style" | "template" | "title" => {}

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
        (whitespace_link, "<span>foo </span><a href=\"/2\">bar</a>", "foo [bar](https://example.com/2)"),
        (link, "<a href=\"/foo\">bar</a>", "[bar](https://example.com/foo)"),
        (link_url_is_raw, "<a href=\"/foo_bar\">baz</a>", "[baz](https://example.com/foo_bar)"),
        (link_code, "<a href=\"/foo\"><code>bar</code></a>", "[`bar`](https://example.com/foo)"),
        (link_not_code, "<a href=\"/foo\">`bar`</a>", "[\\`bar\\`](https://example.com/foo)"),
        (link_anchor, "<a href=\"#somewhere\">text</a>", "[text](https://example.com/#somewhere)"),
        (link_anchor_useless, "<h3>header <a href=\"#somewhere\">#</a></h3>", "### header"),
        (strong, "foo <strong>bar</strong> baz", "foo **bar** baz"),
        (strong_leading_space, "foo<strong> bar</strong> baz", "foo **bar** baz"),
        (strong_trailing_space, "foo <strong>bar </strong>baz", "foo **bar** baz"),
        (strong_empty, "foo <strong> </strong>baz", "foo baz"),
        (blockquote, "<blockquote>foo</blockquote>", "> foo"),
        (blockquote_empty, "<blockquote></blockquote>", ""),
        (blockquote_over_ps, "before <blockquote>\n<p>foo</p><p>bar</p>\n</blockquote> after", "before\n\n> foo\n>\n> bar\n\nafter"),
        (blockquote_adjacent, "<blockquote>foo</blockquote><blockquote>bar</blockquote>", "> foo\n\n> bar"),
        (blockquote_nested_empty, "<blockquote>foo<blockquote></blockquote>bar</blockquote", "> foo\n>\n> bar"),
        (blockquote_pre, "<blockquote>foo<pre>  bar</pre>baz</blockquote>", "> foo\n>\n> ```\n>   bar\n> ```\n>\n> baz"),
        (blockquote_pre_newline, "<blockquote>foo<pre>  bar\n</pre>baz</blockquote>", "> foo\n>\n> ```\n>   bar\n> ```\n>\n> baz"),
        (br, "foo<br>bar", "foo\nbar"),
        (br_space, "foo<br> bar", "foo\nbar"),
        (br_space_span, "foo<br>\n<span>bar</span>", "foo\nbar"),
        (code, "foo <code>bar</code> baz", "foo `bar` baz"),
        (code_leading_space, "foo<code> bar</code> baz", "foo `bar` baz"),
        (code_trailing_space, "foo <code>bar </code>baz", "foo `bar` baz"),
        (code_empty, "foo <code> </code>baz", "foo baz"),
        (code_literals, "<code>*_foo</code>bar*", "`*_foo`bar\\*"),
        (div, "<div>foo</div><div>bar</div>", "foo\n\nbar"),
        (p, "<p>foo</p><p>bar</p>", "foo\n\nbar"),
        (em, "foo <em>bar</em> baz", "foo _bar_ baz"),
        (em_leading_space, "foo<em> bar</em> baz", "foo _bar_ baz"),
        (em_trailing_space, "foo <em>bar </em>baz", "foo _bar_ baz"),
        (em_empty, "foo <em> </em>baz", "foo baz"),
        (header_h1, "<h1>header</h1>", "header\n======"),
        (header_h1_long, "<h1>header header header header header header header header header header header header</h1>", "header header header header header header header header header header header\nheader\n================================================================================"),
        (header_ignore_empty, "<h1></h1>", ""),
        (header_escapes, "<h1>foo `bar` baz</h1>", "foo \\`bar\\` baz\n==============="),
        (header_h2, "<h2>header</h2>", "header\n------"),
        (header_h3, "<h3>header</h3>", "### header"),
        (img_escape_alt, "<img src=\"/foo.png\" alt=\"bar_baz\">", "![bar\\_baz](https://example.com/foo.png)"),
        (img_url_is_raw, "<img src=\"/foo_bar.png\" alt=\"baz\">", "![baz](https://example.com/foo_bar.png)"),
        (ol, "<ol><li>foo</li><li>bar</li></ol>", "1. foo\n2. bar"),
        (ol_whitespace, "<ol> <li>foo</li> <li>bar</li> <li>baz</li> <li>quux</li> <li>not ten</li> </ol>", "1. foo\n2. bar\n3. baz\n4. quux\n5. not ten"),
        (ol_empty_item, "<ol><li>foo</li><li></li><li>bar</li></ol>", "1. foo\n2.\n3. bar"),
        (ol_ten, "<ol><li>1</li><li>2</li><li>3</li><li>4</li><li>5</li><li>6</li><li>7</li><li>8</li><li>9</li><li>10</li></ol>", " 1. 1\n 2. 2\n 3. 3\n 4. 4\n 5. 5\n 6. 6\n 7. 7\n 8. 8\n 9. 9\n10. 10"),
        (pre, "<pre>\nfoo\n    bar\n</pre>", "```\nfoo\n    bar\n```"),
        (pre_no_whitespace_compress, "<pre>\nfoo  bar\n</pre>", "```\nfoo  bar\n```"),
        (pre_no_newline, "<pre>\n  foo</pre>", "```\n  foo\n```"),
        (pre_following_text, "foo bar<pre>baz</pre>", "foo bar\n\n```\nbaz\n```"),
        (pre_in_p, "<p>foo <pre>\nbar\n</pre>baz</p>", "foo\n\n```\nbar\n```\n\nbaz"),
        (pre_language, "<pre><code class=\"language-foo bar\">foo\n    bar\n</code></pre>", "```foo\nfoo\n    bar\n```"),
        (pre_br, "<pre>foo<br>bar</pre>", "```\nfoo\nbar\n```"),
        (pre_br_twice, "<blockquote><pre>foo<br><br>bar</pre></blockquote>", "> ```\n> foo\n>\n> bar\n> ```"),
        (ul, "<ul><li>foo</li><li>bar</li></ul>", "* foo\n* bar"),
        (ul_empty_item, "<ul><li>foo</li><li><li>bar</li></ul>", "* foo\n*\n* bar"),
        (ul_nested, "<ul><li>foo</li><li><ul><li>bar</li><li>baz</li></ul></li><li>quux</li></ul>", "* foo\n* * bar\n  * baz\n* quux"),
        (ul_nested_whitespace, "<ul><li>foo</li><li>before<ul>\n<li>bar</li>\n<li>baz</li>\n</ul>\nafter</li><li>quux</li></ul>", "* foo\n* before\n  * bar\n  * baz\n  after\n* quux"),
        (ul_pre, "<ul><li>foo</li><li><pre>bar</pre></li><li>baz</li></ul>", "* foo\n* ```\n  bar\n  ```\n* baz"),
        (script, "foo <script>bar</script>baz", "foo baz"),
        (cthulhu, "<p>foo<blockquote>bar<ul><li>baz</li><li><pre>quux</pre></li><li><blockquote>foo<pre>bar</pre>baz</blockquote></li></ul></blockquote>quux</p>", "foo\n\n> bar\n> * baz\n> * ```\n>   quux\n>   ```\n> * > foo\n>   > ```\n>   > bar\n>   > ```\n>   > baz\n\nquux"),
    );
}
