use std::cmp::Ordering;
use std::fmt::Write;
use std::iter;
use std::num::NonZeroUsize;

use scraper::ElementRef;
use textwrap::WordSeparator;
use unicode_width::UnicodeWidthStr;
use url::Url;

use super::render_node;
use super::select_single_element;

#[derive(Debug)]
struct Table<'a> {
    data: Vec<Vec<ElementRef<'a>>>,
    headers: usize,
    footers: usize,
}

#[derive(Debug)]
struct ColumnStat {
    min: usize,
    avg: usize,
    max: usize,
}

fn parse_table(table: ElementRef<'_>) -> Table<'_> {
    let header = select_single_element(&table, ":scope > thead");
    let body = select_single_element(&table, ":scope > tbody");
    let footer = select_single_element(&table, ":scope > tfoot");

    let headers = header
        .map(|h| h.child_elements().count())
        .unwrap_or_default();
    let footers = footer
        .map(|f| f.child_elements().count())
        .unwrap_or_default();

    let rows = [header, body, footer]
        .into_iter()
        .flatten()
        .flat_map(|e| e.child_elements());
    let data = rows.map(|r| r.child_elements().collect()).collect();

    Table {
        data,
        headers,
        footers,
    }
}

fn compute_column_stats(data: &[Vec<ElementRef<'_>>], url: &Url) -> Vec<ColumnStat> {
    let column_count = data.iter().map(Vec::len).max().unwrap_or_default();
    let separator = WordSeparator::new();
    (0..column_count)
        .map(|i| {
            let (count, min, sum, max) = data.iter().filter_map(|r| r.get(i)).fold(
                (0, 0, 0, 0),
                |(count, min, sum, max), cell| {
                    let rendered = render_node(**cell, url, None);
                    let max_word_width = separator
                        .find_words(&rendered)
                        .map(|w| w.word.width())
                        .max()
                        .unwrap_or_default();
                    let max_line_width = rendered
                        .split('\n')
                        .map(UnicodeWidthStr::width)
                        .max()
                        .expect("str::split always returns an item");
                    (
                        count + 1,
                        std::cmp::max(min, max_word_width),
                        sum + max_line_width,
                        std::cmp::max(max, max_line_width),
                    )
                },
            );
            ColumnStat {
                min,
                avg: sum / count,
                max,
            }
        })
        .collect()
}

fn compute_widths(
    mut column_stats: Vec<ColumnStat>,
    max_width: Option<NonZeroUsize>,
) -> Vec<usize> {
    let Some(max_width) = max_width.map(NonZeroUsize::get) else {
        return column_stats.into_iter().map(|stat| stat.max).collect();
    };

    let col_sep_width = (column_stats.len() - 1) * 3;

    if column_stats.iter().map(|stat| stat.max).sum::<usize>() + col_sep_width <= max_width {
        return column_stats.into_iter().map(|stat| stat.max).collect();
    }

    if column_stats.iter().map(|stat| stat.min).sum::<usize>() + col_sep_width >= max_width {
        return column_stats.into_iter().map(|stat| stat.min).collect();
    }

    column_stats
        .iter_mut()
        .for_each(|stat| stat.avg = std::cmp::max(stat.avg, stat.min));

    let avg_total = column_stats.iter().map(|stat| stat.avg).sum::<usize>() + col_sep_width;
    match avg_total.cmp(&max_width) {
        Ordering::Less => {
            let extra = max_width - avg_total;
            let delta: usize = column_stats.iter().map(|stat| stat.max - stat.avg).sum();
            column_stats
                .into_iter()
                .map(|stat| stat.avg + extra * (stat.max - stat.avg) / delta)
                .collect()
        }

        Ordering::Equal => column_stats.into_iter().map(|stat| stat.avg).collect(),

        Ordering::Greater => {
            let extra = avg_total - max_width;
            let delta: usize = column_stats.iter().map(|stat| stat.avg - stat.min).sum();
            // div_ceil to ensure the sum is less than max_width
            column_stats
                .into_iter()
                .map(|stat| stat.avg - (extra * (stat.avg - stat.min)).div_ceil(delta))
                .collect()
        }
    }
}

pub(super) fn render_table(
    table_element: ElementRef<'_>,
    url: &Url,
    max_width: Option<NonZeroUsize>,
) -> String {
    let table = parse_table(table_element);
    if table.data.is_empty() {
        return String::new();
    }

    let widths = compute_widths(compute_column_stats(&table.data, url), max_width);

    let mut result = String::with_capacity(
        (widths.iter().sum::<usize>() + 3 * (widths.len() - 1) + 1) * table.data.len(),
    );
    let footer_start = table.data.len() - table.footers;
    table.data.into_iter().enumerate().for_each(|(i, row)| {
        if i != 0 {
            result.push('\n');
        }

        if i == footer_start {
            widths
                .iter()
                .zip(iter::successors(Some(""), |_| Some("-|-")))
                .for_each(|(width, sep)| {
                    write!(result, "{sep}{:-<width$}", "").expect("write into String can't fail");
                });
            result.push('\n');
        }

        let rendered_cells: Vec<_> = row
            .into_iter()
            .zip(widths.iter())
            .map(|(element, width)| render_node(*element, url, NonZeroUsize::new(*width)))
            .collect();
        let cell_lines: Vec<_> = rendered_cells
            .iter()
            .map(|c| c.split('\n').collect())
            .collect();
        let line_count = cell_lines
            .iter()
            .map(Vec::len)
            .reduce(std::cmp::max)
            .unwrap_or_default();
        for line in 0..line_count {
            if line != 0 {
                result.push('\n');
            }
            let separator = if line == 0 { " | " } else { "   " };
            cell_lines
                .iter()
                .zip(widths.iter())
                .zip(iter::successors(Some(""), |_| Some(separator)))
                .for_each(|((cell, width), sep)| {
                    let content = cell.get(line).unwrap_or(&"");
                    // fmt width is in characters; so munge to handle double width characters.
                    let width = width + content.chars().count() - content.width();
                    write!(result, "{sep}{content:width$}").expect("write into String can't fail");
                });
            result.truncate(result.trim_end().len());
        }

        if i + 1 == table.headers {
            result.push('\n');
            widths
                .iter()
                .zip(iter::successors(Some(""), |_| Some("=|=")))
                .for_each(|(width, sep)| {
                    write!(result, "{sep}{:=<width$}", "").expect("write into String can't fail");
                });
        }
    });
    result
}

#[cfg(test)]
mod tests {
    mod compute_widths {
        use std::num::NonZeroUsize;

        use super::super::compute_widths;
        use super::super::ColumnStat;

        macro_rules! compute_widths_tests {
            ($(($name: ident, $max_width: expr, [$(($min: expr, $avg: expr, $max: expr)),+], [$($expected: expr),+]),)*) => {
                $(
                    #[test]
                    fn $name() {
                        let data = vec![$(ColumnStat { min: $min, avg: $avg, max: $max },)*];
                        assert_eq!(compute_widths(data, NonZeroUsize::new($max_width)), vec![$($expected,)*]);
                    }
                )*
            }
        }

        compute_widths_tests!(
            (
                simple,
                80,
                [(1, 1, 1), (7, 7, 7), (2, 2, 2), (9, 9, 9)],
                [1, 7, 2, 9]
            ),
            (wrap, 20, [(4, 16, 16), (5, 5, 5)], [12, 5]),
            (longer_wrap, 28, [(5, 13, 13), (11, 97, 97)], [5, 19]),
            (avg_less_than_min, 40, [(10, 5, 15), (10, 25, 50)], [10, 26]),
            (avg_under, 40, [(9, 26, 35), (3, 5, 7)], [30, 6]),
            (avg_over, 30, [(9, 26, 35), (3, 5, 7)], [22, 4]),
        );
    }

    mod render_table {
        use std::num::NonZeroUsize;

        use scraper::ElementRef;
        use scraper::Html;
        use url::Url;

        use super::super::render_table;

        fn run_render_test(html: &'static str, expected: &'static str) {
            let tree = Html::parse_fragment(html);

            let root = tree.root_element();
            assert_eq!(root.value().name(), "html");
            assert_eq!(root.children().count(), 1);
            let table = root.child_elements().next().unwrap();
            assert_eq!(table.value().name(), "table");

            assert_eq!(
                render_table(
                    ElementRef::wrap(*table).expect("node is Node::Element"),
                    &Url::parse("https://example.com/").unwrap(),
                    NonZeroUsize::new(80),
                ),
                expected
            );
        }

        macro_rules! render_tests {
            ($(($name: ident, $html: expr, $expected: expr),)*) => {
                $(
                    #[test]
                    fn $name() {
                        run_render_test($html, $expected);
                    }
                )*
            }
        }

        render_tests!(
            (empty, "<table></table>", ""),
            (simple, "<table><tr><td>1</td><td>2</td><td>3</td></tr><tr><td>4</td><td>5</td><td>6</td></tr></table>", "1 | 2 | 3\n4 | 5 | 6"),
            (with_space, "<table><tr> <td>1</td> <td>2</td> <td>3</td> </tr><tr><td>4</td><td>5</td><td>6</td></tr></table>", "1 | 2 | 3\n4 | 5 | 6"),
            (one_column, "<table><tr><td>foo</td></tr><tr><td>bar</td></tr><tr><td>baz</td></tr></table>", "foo\nbar\nbaz"),
            (empty_column, "<table><tr><td>foo</td><td></td><td>bar</td></tr><tr><td>baz</td><td></td><td>quux</td></tr></table>", "foo |  | bar\nbaz |  | quux"),
            (width, "<table><tr><td>abcd</td><td>2</td><td>3</td></tr><tr><td>4</td><td>5</td><td>6</td></tr></table>", "abcd | 2 | 3\n4    | 5 | 6"),
            (unicode_width_zero, "<table><tr><td>foo</td><td>bar</td></tr><tr><td>\u{200d}</td><td>baz</td></tr></table>", "foo | bar\n\u{200d}    | baz"),
            (unicode_width_double, "<table><tr><td>foo</td><td>bar</td></tr><tr><td>\u{1f310}</td><td>baz</td></tr></table>", "foo | bar\n\u{1f310}  | baz"),
            (long, "<table><tr><td>1234567 10 234567 20 234567 30 234567 40 234567 50 234567 60 234567 70</td><td>foo bar</td><td>baz</td></tr><tr><td>foo</td><td>foo bar</td><td>baz</td></tr></table>", "1234567 10 234567 20 234567 30 234567 40 234567 50 234567 60     | foo bar | baz\n234567 70\nfoo                                                              | foo bar | baz"),
            (newline, "<table><tr><td>foo<br />bar</td><td>baz</td></tr><tr><td>1</td><td>2</td></tr></table>", "foo | baz\nbar\n1   | 2"),
            (ragged, "<table><tr><td>1</td><td>2</td></tr><tr><td>4</td><td>5</td><td>6</td></tr></table>", "1 | 2\n4 | 5 | 6"),
            (header, "<table><thead><tr><td>1</td><td>2</td><td>3</td></tr></thead><tr><td>4</td><td>5</td><td>6</td></tr></table>", "1 | 2 | 3\n==|===|==\n4 | 5 | 6"),
            (footer, "<table><tr><td>1</td><td>2</td><td>3</td></tr><tfoot><tr><td>4</td><td>5</td><td>6</td></tr></tfoot></table>", "1 | 2 | 3\n--|---|--\n4 | 5 | 6"),
            (no_body, "<table><thead><tr><td>1</td><td>2</td><td>3</td></tr></thead><tfoot><tr><td>4</td><td>5</td><td>6</td></tr></tfoot></table>", "1 | 2 | 3\n==|===|==\n--|---|--\n4 | 5 | 6"),
            (nested, "<table><tr><td><table><tr><td>1</td><td>2</td><td>3</td></tr><tr><td>4</td><td>5</td><td>6</td></tr></table></td></tr></table>", "1 | 2 | 3\n4 | 5 | 6"),
        );
    }
}
