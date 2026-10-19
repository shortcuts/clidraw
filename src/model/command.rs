use super::{Canvas, Element, End, Id, ModelError, ShapeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    From,
    To,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Insert(usize, Element),
    Remove(Id),
    Move {
        id: Id,
        dx: i32,
        dy: i32,
    },
    SetRect {
        id: Id,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    SetLabel {
        id: Id,
        label: String,
    },
    SetEnd {
        id: Id,
        side: Side,
        end: End,
    },
    SetKind {
        id: Id,
        kind: ShapeKind,
    },
    Batch(Vec<Command>),
}

impl Canvas {
    pub fn push(&mut self, e: Element) -> Result<Command, ModelError> {
        self.apply(Command::Insert(self.elements.len(), e))
    }

    /// Applies the command and returns the command that undoes it.
    pub fn apply(&mut self, cmd: Command) -> Result<Command, ModelError> {
        match cmd {
            Command::Insert(i, e) => {
                let id = e.id().clone();
                if self.index_of(&id).is_some() {
                    return Err(ModelError::Duplicate(id));
                }
                self.elements.insert(i.min(self.elements.len()), e);
                Ok(Command::Remove(id))
            }
            Command::Remove(id) => self.remove(id),
            Command::Batch(cmds) => {
                let mut undo = vec![];
                for c in cmds {
                    match self.apply(c) {
                        Ok(u) => undo.push(u),
                        Err(e) => {
                            for u in undo.into_iter().rev() {
                                let _ = self.apply(u);
                            }
                            return Err(e);
                        }
                    }
                }
                undo.reverse();
                Ok(Command::Batch(undo))
            }
            other => self.edit(other),
        }
    }

    fn remove(&mut self, id: Id) -> Result<Command, ModelError> {
        let i = self
            .index_of(&id)
            .ok_or_else(|| ModelError::NotFound(id.clone()))?;
        let mut undo = vec![];
        let mut unbind = vec![];
        if self.shape(&id).is_some() {
            for e in &self.elements {
                let Element::Line(l) = e else { continue };
                let (a, b) = self.line_ends(l);
                for (side, end, at) in [(Side::From, &l.from, a), (Side::To, &l.to, b)] {
                    if *end == End::Bound(id.clone()) {
                        unbind.push((l.id.clone(), side, at));
                    }
                }
            }
        }
        let mut restore = vec![];
        for (lid, side, at) in unbind {
            let old = End::Bound(id.clone());
            self.apply(Command::SetEnd {
                id: lid.clone(),
                side,
                end: End::Free(at.0, at.1),
            })?;
            restore.push(Command::SetEnd {
                id: lid,
                side,
                end: old,
            });
        }
        let i = self.index_of(&id).unwrap_or(i);
        let e = self.elements.remove(i);
        undo.push(Command::Insert(i, e));
        undo.extend(restore);
        Ok(Command::Batch(undo))
    }

    fn edit(&mut self, cmd: Command) -> Result<Command, ModelError> {
        let wrong = |id: &Id| ModelError::WrongKind(id.clone());
        let missing = |id: &Id| ModelError::NotFound(id.clone());
        match cmd {
            Command::Move { id, dx, dy } => {
                let i = self.index_of(&id).ok_or_else(|| missing(&id))?;
                match &mut self.elements[i] {
                    Element::Shape(s) => (s.x, s.y) = (s.x + dx, s.y + dy),
                    Element::Text(t) => (t.x, t.y) = (t.x + dx, t.y + dy),
                    Element::Line(l) => {
                        for end in [&mut l.from, &mut l.to] {
                            if let End::Free(x, y) = end {
                                (*x, *y) = (*x + dx, *y + dy);
                            }
                        }
                    }
                }
                Ok(Command::Move {
                    id,
                    dx: -dx,
                    dy: -dy,
                })
            }
            Command::SetRect { id, x, y, w, h } => {
                let i = self.index_of(&id).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = Command::SetRect {
                    id,
                    x: s.x,
                    y: s.y,
                    w: s.w,
                    h: s.h,
                };
                (s.x, s.y, s.w, s.h) = (x, y, w, h);
                Ok(old)
            }
            Command::SetLabel { id, label } => {
                let i = self.index_of(&id).ok_or_else(|| missing(&id))?;
                let slot = match &mut self.elements[i] {
                    Element::Shape(s) => &mut s.label,
                    Element::Line(l) => &mut l.label,
                    Element::Text(t) => &mut t.text,
                };
                let old = std::mem::replace(slot, label);
                Ok(Command::SetLabel { id, label: old })
            }
            Command::SetEnd { id, side, end } => {
                if let End::Bound(sid) = &end {
                    self.shape(sid).ok_or_else(|| missing(sid))?;
                }
                let i = self.index_of(&id).ok_or_else(|| missing(&id))?;
                let Element::Line(l) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let slot = if side == Side::From {
                    &mut l.from
                } else {
                    &mut l.to
                };
                let old = std::mem::replace(slot, end);
                Ok(Command::SetEnd { id, side, end: old })
            }
            Command::SetKind { id, kind } => {
                let i = self.index_of(&id).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.kind, kind);
                Ok(Command::SetKind { id, kind: old })
            }
            Command::Insert(..) | Command::Remove(_) | Command::Batch(_) => unreachable!(),
        }
    }
}
