pub(super) fn is_whitespace(c: char) -> bool {
    c.is_whitespace() || c == '\u{200b}'
}

pub(super) struct SqueezeWhitespace<T>
where
    T: Iterator<Item = char>,
{
    chars: T,
    next: Option<char>,
}

impl<T> SqueezeWhitespace<T>
where
    T: Iterator<Item = char>,
{
    pub fn new(mut chars: T) -> Self {
        let next = chars.find(|c| !is_whitespace(*c));
        Self { chars, next }
    }
}

impl<T> Iterator for SqueezeWhitespace<T>
where
    T: Iterator<Item = char>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next {
            self.next = None;
            return Some(next);
        }
        if let Some(next) = self.chars.next() {
            if is_whitespace(next) {
                self.next = self.chars.find(|c| !is_whitespace(*c));
                if self.next.is_some() { Some(' ') } else { None }
            } else {
                Some(next)
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SqueezeWhitespace;

    macro_rules! tests {
        ($(($name: ident, $input: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(SqueezeWhitespace::new($input.chars()).collect::<String>(), $expected);
                }
            )*
        }
    }

    tests!(
        (plain, "foo bar baz", "foo bar baz"),
        (leading, "  foo bar baz", "foo bar baz"),
        (trailing, "foo bar baz  ", "foo bar baz"),
        (middle, "foo  bar  baz", "foo bar baz"),
        (newline, "foo\nbar\n  baz \nquux", "foo bar baz quux"),
        (tab, "foo\tbar\t  baz \tquux", "foo bar baz quux"),
        (
            zwsp,
            "foo\u{200b}bar\u{200b}  baz \u{200b}quux",
            "foo bar baz quux"
        ),
    );
}
