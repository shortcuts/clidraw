use super::{Canvas, Command, ModelError};

pub const HISTORY_LIMIT: usize = 1000;

pub struct History {
    undo: Vec<(Command, String)>,
    redo: Vec<(Command, String)>,
    pub limit: usize,
}

impl Default for History {
    fn default() -> History {
        History::with_limit(HISTORY_LIMIT)
    }
}

impl History {
    pub fn with_limit(limit: usize) -> History {
        History {
            undo: vec![],
            redo: vec![],
            limit,
        }
    }

    pub fn exec(&mut self, canvas: &mut Canvas, cmd: Command) -> Result<(), ModelError> {
        let inverse = canvas.apply(cmd)?;
        self.record(inverse);
        Ok(())
    }

    /// Stores the inverse of a command that the caller already applied.
    pub fn record(&mut self, inverse: Command) {
        let label = inverse.label();
        self.undo.push((inverse, label));
        self.undo
            .drain(..self.undo.len().saturating_sub(self.limit));
        self.redo.clear();
    }

    pub fn commands(&self) -> Vec<Command> {
        self.undo.iter().map(|(c, _)| c.clone()).collect()
    }

    pub fn restore(&mut self, inverses: Vec<Command>) {
        inverses.into_iter().for_each(|c| self.record(c));
    }

    /// Returns the label of the reverted action, or `None` when the stack is empty.
    pub fn undo(&mut self, canvas: &mut Canvas) -> Result<Option<String>, ModelError> {
        Self::step(&mut self.undo, &mut self.redo, canvas)
    }

    pub fn redo(&mut self, canvas: &mut Canvas) -> Result<Option<String>, ModelError> {
        Self::step(&mut self.redo, &mut self.undo, canvas)
    }

    fn step(
        from: &mut Vec<(Command, String)>,
        to: &mut Vec<(Command, String)>,
        c: &mut Canvas,
    ) -> Result<Option<String>, ModelError> {
        let Some((cmd, label)) = from.pop() else {
            return Ok(None);
        };
        match c.apply(cmd.clone()) {
            Ok(inverse) => {
                to.push((inverse, label.clone()));
                Ok(Some(label))
            }
            Err(e) => {
                from.push((cmd, label));
                Err(e)
            }
        }
    }
}
