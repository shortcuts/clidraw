use super::{Canvas, Command, ModelError};

pub const HISTORY_LIMIT: usize = 1000;

pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
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
        self.undo.push(inverse);
        self.undo
            .drain(..self.undo.len().saturating_sub(self.limit));
        self.redo.clear();
    }

    pub fn undo(&mut self, canvas: &mut Canvas) -> Result<bool, ModelError> {
        Self::step(&mut self.undo, &mut self.redo, canvas)
    }

    pub fn redo(&mut self, canvas: &mut Canvas) -> Result<bool, ModelError> {
        Self::step(&mut self.redo, &mut self.undo, canvas)
    }

    fn step(
        from: &mut Vec<Command>,
        to: &mut Vec<Command>,
        c: &mut Canvas,
    ) -> Result<bool, ModelError> {
        let Some(cmd) = from.pop() else {
            return Ok(false);
        };
        to.push(c.apply(cmd)?);
        Ok(true)
    }
}
