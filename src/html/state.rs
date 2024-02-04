#[derive(Debug, Default)]
pub(super) struct State {
    result: String,
}

impl State {
    pub fn root_block(&mut self) -> Block<'_> {
        Block { state: self }
    }

    pub fn render(self) -> String {
        self.result
    }
}

#[derive(Debug)]
pub(super) struct Block<'s> {
    state: &'s mut State,
}

impl<'s> Block<'s> {
    pub fn push(&mut self, s: &str) {
        if !s.chars().all(char::is_whitespace) || !self.state.result.is_empty() {
            self.state.result.push_str(s);
        }
    }
}
