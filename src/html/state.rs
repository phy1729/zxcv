use super::escape_markdown::EscapeMarkdown;
use super::squeeze_whitespace::is_whitespace;
use super::squeeze_whitespace::SqueezeWhitespace;
use crate::LINE_LENGTH;

#[derive(Debug, Default)]
pub(super) struct State {
    result: String,
    pending: String,
}

impl State {
    pub fn root_block(&mut self) -> Block<'_> {
        Block {
            state: self,
            trailing_whitespace: false,
            in_code: false,
        }
    }

    pub fn render(self) -> String {
        debug_assert!(self.pending.is_empty());
        self.result
    }
}

#[derive(Debug)]
pub(super) struct Block<'s> {
    state: &'s mut State,
    trailing_whitespace: bool,
    in_code: bool,
}

impl<'s> Block<'s> {
    pub fn start_code(&mut self) {
        self.in_code = true;
    }

    pub fn end_code(&mut self) {
        self.in_code = false;
    }

    pub fn push(&mut self, s: &str) {
        self.push_inner(s, false);
    }

    pub fn push_raw(&mut self, s: &str) {
        self.push_inner(s, true);
    }

    fn push_inner(&mut self, s: &str, raw: bool) {
        if s.chars().all(is_whitespace) {
            self.trailing_whitespace |= !s.is_empty();
        } else {
            let initial_whitespace = s.chars().next().map(is_whitespace) == Some(true);
            if (self.trailing_whitespace || initial_whitespace)
                && !(self.state.pending.is_empty() || self.state.pending.ends_with('\n'))
            {
                self.state.pending.push(' ');
            }

            if raw {
                self.state.pending.push_str(s.trim());
            } else if self.in_code {
                self.state.pending.extend(SqueezeWhitespace::new(s.chars()));
            } else {
                self.state
                    .pending
                    .extend(EscapeMarkdown::new(SqueezeWhitespace::new(s.chars())));
            }

            self.trailing_whitespace = s.chars().last().map(is_whitespace) == Some(true);
        }
    }

    pub fn newline(&mut self) {
        self.state.pending.push('\n');
        self.trailing_whitespace = false;
    }

    fn push_pending(&mut self) {
        if !self.state.pending.is_empty() {
            self.push_gap();
            self.state
                .result
                .push_str(&textwrap::fill(&self.state.pending, LINE_LENGTH));
            self.state.pending.clear();
            self.trailing_whitespace = false;
        }
    }

    fn push_gap(&mut self) {
        if !self.state.result.is_empty() {
            self.state.result.push_str("\n\n");
        }
    }

    pub fn new_block(&mut self) -> Block<'_> {
        self.push_pending();
        Block {
            state: self.state,
            trailing_whitespace: false,
            in_code: false,
        }
    }

    pub fn new_raw_block(&mut self) -> RawBlock<'_> {
        self.push_pending();
        self.push_gap();
        RawBlock {
            state: self.state,
            at_sol: true,
        }
    }
}

impl Drop for Block<'_> {
    fn drop(&mut self) {
        self.push_pending();
    }
}

#[derive(Debug)]
pub(super) struct RawBlock<'s> {
    state: &'s mut State,
    at_sol: bool,
}

impl<'s> RawBlock<'s> {
    pub fn push(&mut self, s: &str) {
        for line in s.split_inclusive('\n') {
            self.state.result.push_str(line);
            self.at_sol = line.ends_with('\n');
        }
    }

    pub fn newline(&mut self) {
        self.state.result.push('\n');
        self.at_sol = true;
    }

    pub fn ensure_newline(&mut self) {
        if !self.at_sol {
            self.newline();
        }
    }
}
