#[derive(Debug, Default)]
pub(super) struct State {
    result: String,
    pending: String,
}

impl State {
    pub fn root_block(&mut self) -> Block<'_> {
        Block { state: self }
    }

    pub fn render(self) -> String {
        debug_assert!(self.pending.is_empty());
        self.result
    }
}

#[derive(Debug)]
pub(super) struct Block<'s> {
    state: &'s mut State,
}

impl<'s> Block<'s> {
    pub fn push(&mut self, s: &str) {
        if !s.chars().all(char::is_whitespace) {
            self.state.pending.push_str(s);
        }
    }

    pub fn newline(&mut self) {
        self.state.pending.push('\n');
    }

    fn push_pending(&mut self) {
        if !self.state.pending.is_empty() {
            self.push_gap();
            self.state.result.push_str(&self.state.pending);
            self.state.pending.clear();
        }
    }

    fn push_gap(&mut self) {
        if !self.state.result.is_empty() {
            self.state.result.push_str("\n\n");
        }
    }

    pub fn new_block(&mut self) -> Block<'_> {
        self.push_pending();
        Block { state: self.state }
    }
}

impl Drop for Block<'_> {
    fn drop(&mut self) {
        self.push_pending();
    }
}
