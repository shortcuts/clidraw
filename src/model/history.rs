use super::{Canvas, Command, ModelError};

#[derive(Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn exec(&mut self, canvas: &mut Canvas, cmd: Command) -> Result<(), ModelError> {
        let inverse = canvas.apply(cmd)?;
        self.record(inverse);
        Ok(())
    }

    /// Stores the inverse of a command that the caller already applied.
    pub fn record(&mut self, inverse: Command) {
        self.undo.push(inverse);
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
