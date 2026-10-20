use super::canvas::{Index, BULK};
use super::{Canvas, Element, End, Id, LineKind, ModelError, ShapeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    From,
    To,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Z {
    Forward,
    Backward,
    Front,
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Insert(usize, Element),
    Remove(Id),
    /// Reorders without removing, so bound lines stay bound.
    MoveTo {
        id: Id,
        to: usize,
    },
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
    SetLineKind {
        id: Id,
        kind: LineKind,
    },
    Batch(Vec<Command>),
}

impl Command {
    /// Names the action that this inverse command reverts, for example "move 3 elements".
    pub fn label(&self) -> String {
        let mut leaves = vec![];
        self.leaves(&mut leaves);
        let Some(first) = leaves.first() else {
            return "nothing".into();
        };
        let n = leaves.iter().filter(|c| c.verb() == first.verb()).count();
        let s = if n == 1 { "" } else { "s" };
        format!("{} {n} element{s}", first.verb())
    }

    fn leaves<'a>(&'a self, out: &mut Vec<&'a Command>) {
        match self {
            Command::Batch(cmds) => cmds.iter().for_each(|c| c.leaves(out)),
            c => out.push(c),
        }
    }

    fn verb(&self) -> &'static str {
        match self {
            Command::Remove(_) => "add",
            Command::Insert(..) => "delete",
            Command::MoveTo { .. } => "reorder",
            Command::Move { .. } => "move",
            Command::SetRect { .. } => "resize",
            Command::SetLabel { .. } => "relabel",
            Command::SetEnd { .. } => "reconnect",
            Command::SetKind { .. } | Command::SetLineKind { .. } => "restyle",
            Command::Batch(_) => unreachable!(),
        }
    }
}

impl Canvas {
    pub fn push(&mut self, e: Element) -> Result<Command, ModelError> {
        self.apply(Command::Insert(self.elements.len(), e))
    }

    /// Commands that change the draw order of `selection`; empty when nothing moves.
    pub fn z_order(&self, selection: &[Id], z: Z) -> Vec<Command> {
        let mut order: Vec<(Id, bool)> = self
            .elements
            .iter()
            .map(|e| (e.id().clone(), selection.contains(e.id())))
            .collect();
        let n = order.len();
        let mut cmds = vec![];
        let mut emit = |id: &Id, to| cmds.push(Command::MoveTo { id: id.clone(), to });
        match z {
            Z::Front => {
                // Selected elements above the last unselected one are already in place.
                let top = order.iter().rposition(|e| !e.1).unwrap_or(0);
                order[..top]
                    .iter()
                    .filter(|e| e.1)
                    .for_each(|e| emit(&e.0, n - 1));
            }
            Z::Back => {
                let low = order.iter().position(|e| !e.1).unwrap_or(n);
                order
                    .iter()
                    .skip(low)
                    .rev()
                    .filter(|e| e.1)
                    .for_each(|e| emit(&e.0, 0));
            }
            Z::Forward => {
                for i in (0..n.saturating_sub(1)).rev() {
                    if order[i].1 && !order[i + 1].1 {
                        emit(&order[i].0, i + 1);
                        order.swap(i, i + 1);
                    }
                }
            }
            Z::Backward => {
                for i in 1..n {
                    if order[i].1 && !order[i - 1].1 {
                        emit(&order[i].0, i - 1);
                        order.swap(i, i - 1);
                    }
                }
            }
        }
        cmds
    }

    /// Applies the command and returns the command that undoes it.
    pub fn apply(&mut self, cmd: Command) -> Result<Command, ModelError> {
        self.apply_in(cmd, &mut None)
    }

    /// `index` maps ids to positions while a long batch runs. `None` means scan.
    fn apply_in(&mut self, cmd: Command, index: &mut Option<Index>) -> Result<Command, ModelError> {
        match cmd {
            Command::Insert(i, e) => {
                let id = e.id().clone();
                if self.index_of(&id).is_some() {
                    return Err(ModelError::Duplicate(id));
                }
                self.elements.insert(i.min(self.elements.len()), e);
                *index = None;
                Ok(Command::Remove(id))
            }
            Command::Remove(id) => {
                let undo = self.remove(id);
                *index = None;
                undo
            }
            Command::MoveTo { id, to } => {
                let from = self
                    .index_of(&id)
                    .ok_or_else(|| ModelError::NotFound(id.clone()))?;
                let e = self.elements.remove(from);
                self.elements.insert(to.min(self.elements.len()), e);
                *index = None;
                Ok(Command::MoveTo { id, to: from })
            }
            Command::Batch(cmds) => {
                if index.is_none() && cmds.len() > BULK {
                    *index = Some(self.index_map());
                }
                let mut undo = vec![];
                for c in cmds {
                    match self.apply_in(c, index) {
                        Ok(u) => undo.push(u),
                        Err(e) => {
                            for u in undo.into_iter().rev() {
                                let _ = self.apply_in(u, index);
                            }
                            return Err(e);
                        }
                    }
                }
                undo.reverse();
                Ok(Command::Batch(undo))
            }
            other => self.edit(other, index),
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
                    if matches!(end, End::Bound(sid, _) if *sid == id) {
                        unbind.push((l.id.clone(), side, at, end.clone()));
                    }
                }
            }
        }
        let mut restore = vec![];
        for (lid, side, at, old) in unbind {
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

    fn edit(&mut self, cmd: Command, index: &Option<Index>) -> Result<Command, ModelError> {
        let wrong = |id: &Id| ModelError::WrongKind(id.clone());
        let missing = |id: &Id| ModelError::NotFound(id.clone());
        match cmd {
            Command::Move { id, dx, dy } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
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
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
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
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let slot = match &mut self.elements[i] {
                    Element::Shape(s) => &mut s.label,
                    Element::Line(l) => &mut l.label,
                    Element::Text(t) => &mut t.text,
                };
                let old = std::mem::replace(slot, label);
                Ok(Command::SetLabel { id, label: old })
            }
            Command::SetEnd { id, side, end } => {
                if let End::Bound(sid, _) = &end {
                    let bound = self.position(sid, index).map(|i| &self.elements[i]);
                    if !matches!(bound, Some(Element::Shape(_))) {
                        return Err(missing(sid));
                    }
                }
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
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
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.kind, kind);
                Ok(Command::SetKind { id, kind: old })
            }
            Command::SetLineKind { id, kind } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Line(l) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut l.kind, kind);
                Ok(Command::SetLineKind { id, kind: old })
            }
            Command::Insert(..)
            | Command::Remove(_)
            | Command::MoveTo { .. }
            | Command::Batch(_) => {
                unreachable!()
            }
        }
    }
}
