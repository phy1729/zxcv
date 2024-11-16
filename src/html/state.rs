use textwrap::Options;
use unicode_width::UnicodeWidthStr;

use super::escape_markdown::EscapeMarkdown;
use super::squeeze_whitespace::is_whitespace;
use super::squeeze_whitespace::SqueezeWhitespace;
use crate::LINE_LENGTH;

#[derive(Debug, Default)]
pub(super) struct State {
    result: String,
    pending: String,
    initial_prefix: String,
    subsequent_prefix: String,
    gap_prefix_offset: usize,
}

impl State {
    pub fn root_block(&mut self) -> Block<'_> {
        Block {
            state: self,
            pending_whitespace: false,
            prefixes: None,
            pending_raw_start: None,
            must_emit: false,
            in_code: false,
            in_item: false,
        }
    }

    pub fn render(self) -> String {
        debug_assert!(self.pending.is_empty());
        self.result
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub(super) struct Block<'s> {
    state: &'s mut State,
    pending_whitespace: bool,
    prefixes: Option<(&'s str, &'s str, String)>,
    pending_raw_start: Option<&'static str>,
    must_emit: bool,
    in_code: bool,
    in_item: bool,
}

impl<'s> Block<'s> {
    pub fn prefix(&mut self, initial_prefix: &'s str, subsequent_prefix: &'s str) {
        debug_assert!(self.prefixes.is_none());
        debug_assert_eq!(initial_prefix.width(), subsequent_prefix.width());

        let mut new_initial_prefix = if self.state.gap_prefix_offset == 0 {
            self.state.subsequent_prefix.clone()
        } else {
            self.state.initial_prefix.clone()
        };
        new_initial_prefix.push_str(initial_prefix);
        let previous_prefix = std::mem::replace(&mut self.state.initial_prefix, new_initial_prefix);
        self.state.subsequent_prefix.push_str(subsequent_prefix);
        self.state.gap_prefix_offset += subsequent_prefix.len();
        self.prefixes = Some((initial_prefix, subsequent_prefix, previous_prefix));
    }

    pub fn must_emit(&mut self) {
        debug_assert!(self.prefixes.is_some());
        self.must_emit = true;
    }

    pub fn start_code(&mut self) {
        self.in_code = true;
    }

    pub fn end_code(&mut self) {
        self.in_code = false;
    }

    fn pre_push(&mut self) {
        if self.pending_whitespace
            && !(self.state.pending.is_empty() || self.state.pending.ends_with('\n'))
        {
            self.state.pending.push(' ');
            self.pending_whitespace = false;
        }

        if let Some(pending_raw_start) = self.pending_raw_start {
            self.state.pending.push_str(pending_raw_start);
            self.pending_raw_start = None;
        }
    }

    pub fn push(&mut self, s: &str) {
        if s.chars().all(is_whitespace) {
            self.pending_whitespace |= !s.is_empty();
        } else {
            self.pending_whitespace |= s.chars().next().map(is_whitespace) == Some(true);
            self.pre_push();

            if self.in_code {
                self.state.pending.extend(SqueezeWhitespace::new(s.chars()));
            } else {
                self.state
                    .pending
                    .extend(EscapeMarkdown::new(SqueezeWhitespace::new(s.chars())));
            }

            self.pending_whitespace = s.chars().last().map(is_whitespace) == Some(true);
        }
    }

    pub fn push_raw(&mut self, s: &str) {
        self.pre_push();
        self.state.pending.push_str(s);
    }

    pub fn push_raw_start(&mut self, s: &'static str) {
        self.pending_raw_start = Some(s);
    }

    pub fn push_raw_end(&mut self, s: &str) {
        if self.pending_raw_start.is_some() {
            self.pending_raw_start = None;
        } else {
            self.state.pending.push_str(s);
        }
    }

    pub fn newline(&mut self) {
        self.state.pending.push('\n');
        self.pending_whitespace = false;
    }

    fn push_pending(&mut self, drop: bool) {
        if !self.state.pending.is_empty() {
            self.push_gap();
            self.state.result.push_str(&textwrap::fill(
                &self.state.pending,
                Options::new(LINE_LENGTH)
                    .initial_indent(if self.state.gap_prefix_offset == 0 {
                        &self.state.subsequent_prefix
                    } else {
                        self.state.gap_prefix_offset = 0;
                        &self.state.initial_prefix
                    })
                    .subsequent_indent(&self.state.subsequent_prefix),
            ));
            self.state.pending.clear();
            self.pending_whitespace = false;
        } else if drop && self.must_emit && self.state.gap_prefix_offset != 0 {
            self.push_gap();
            self.state
                .result
                .push_str(self.state.initial_prefix.trim_end());
        }
    }

    fn push_gap(&mut self) {
        if !self.state.result.is_empty() {
            if !self.in_item {
                self.state.result.push('\n');
                self.state.result.push_str(
                    self.state.subsequent_prefix
                        [..self.state.subsequent_prefix.len() - self.state.gap_prefix_offset]
                        .trim_end(),
                );
            }
            self.state.result.push('\n');
        }
    }

    pub fn new_block(&mut self) -> Block<'_> {
        self.new_block_inner(self.in_item)
    }

    pub fn new_item(&mut self) -> Block<'_> {
        self.new_block_inner(true)
    }

    fn new_block_inner(&mut self, in_item: bool) -> Block<'_> {
        self.push_pending(false);
        Block {
            state: self.state,
            pending_whitespace: false,
            prefixes: None,
            pending_raw_start: None,
            must_emit: false,
            in_code: self.in_code,
            in_item,
        }
    }

    pub fn new_raw_block(&mut self) -> RawBlock<'_> {
        self.push_pending(false);
        self.push_gap();
        RawBlock {
            state: self.state,
            at_sol: true,
        }
    }
}

impl Drop for Block<'_> {
    fn drop(&mut self) {
        self.push_pending(true);
        if let Some((initial_prefix, subsequent_prefix, previous_prefix)) =
            std::mem::take(&mut self.prefixes)
        {
            debug_assert!(self.state.initial_prefix.ends_with(initial_prefix));
            debug_assert!(self.state.subsequent_prefix.ends_with(subsequent_prefix));

            self.state.gap_prefix_offset = self
                .state
                .gap_prefix_offset
                .saturating_sub(initial_prefix.len());
            self.state.initial_prefix = previous_prefix;
            self.state
                .subsequent_prefix
                .truncate(self.state.subsequent_prefix.len() - subsequent_prefix.len());
        }
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
            self.handle_prefix(line == "\n");
            self.state.result.push_str(line);
            self.at_sol = line.ends_with('\n');
        }
    }

    fn handle_prefix(&mut self, trim: bool) {
        if self.at_sol {
            let prefix = if self.state.gap_prefix_offset == 0 {
                &self.state.subsequent_prefix
            } else {
                self.state.gap_prefix_offset = 0;
                &self.state.initial_prefix
            };
            self.state
                .result
                .push_str(if trim { prefix.trim_end() } else { prefix });
        }
    }

    pub fn newline(&mut self) {
        self.handle_prefix(true);
        self.state.result.push('\n');
        self.at_sol = true;
    }

    pub fn ensure_newline(&mut self) {
        if !self.at_sol {
            self.newline();
        }
    }
}
