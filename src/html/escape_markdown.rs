pub(super) struct EscapeMarkdown<T> {
    chars: T,
    next: Option<char>,
}

impl<T> EscapeMarkdown<T>
where
    T: Iterator<Item = char>,
{
    pub fn new(chars: T) -> Self {
        Self { chars, next: None }
    }
}

impl<T> Iterator for EscapeMarkdown<T>
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
            if matches!(next, '#' | '*' | '\\' | '_' | '`') {
                self.next = Some(next);
                Some('\\')
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
    use super::EscapeMarkdown;

    macro_rules! tests {
        ($(($name: ident, $input: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(EscapeMarkdown::new($input.chars()).collect::<String>(), $expected);
                }
            )*
        }
    }

    tests!((plain, "foo bar baz", "foo bar baz"),);
    tests!((backtick, "foo`bar", "foo\\`bar"),);
}
