use super::canvas::{Index, BULK};
use super::{
    Border, Canvas, Element, End, Id, LineKind, ModelError, Paint, Routing, ShapeKind, Stroke,
    TextAlign,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    DistributeH,
    DistributeV,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    SetLabelAt {
        id: Id,
        at: Option<u16>,
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
    SetStroke {
        id: Id,
        stroke: Stroke,
    },
    SetRouting {
        id: Id,
        routing: Routing,
    },
    SetBorder {
        id: Id,
        border: Border,
    },
    SetColor {
        id: Id,
        color: Option<Paint>,
    },
    SetFill {
        id: Id,
        fill: Option<Paint>,
    },
    SetAlign {
        id: Id,
        align: TextAlign,
    },
    SetBold {
        id: Id,
        bold: bool,
    },
    /// Replaces the whole group list.
    SetGroups(Vec<Vec<Id>>),
    Batch(Vec<Command>),
}

impl Command {
    /// Names the action that this inverse command reverts, for example "move 3 elements".
    pub fn label(&self) -> String {
        let mut leaves = vec![];
        self.leaves(&mut leaves);
        if leaves.iter().all(|c| matches!(c, Command::SetGroups(_))) && !leaves.is_empty() {
            return "group change".into();
        }
        leaves.retain(|c| !matches!(c, Command::SetGroups(_)));
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
            Command::SetLabelAt { .. } => "reposition label of",
            Command::SetEnd { .. } => "reconnect",
            Command::SetKind { .. }
            | Command::SetLineKind { .. }
            | Command::SetStroke { .. }
            | Command::SetRouting { .. }
            | Command::SetBorder { .. }
            | Command::SetColor { .. }
            | Command::SetFill { .. }
            | Command::SetAlign { .. }
            | Command::SetBold { .. } => "restyle",
            Command::SetGroups(_) => "regroup",
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

    /// Moves the shapes and text in `selection`; lines are skipped because bound ends follow their
    /// shapes. Empty when fewer than two items (three to distribute) exist or nothing moves.
    pub fn align(&self, selection: &[Id], a: Align) -> Vec<Command> {
        let items: Vec<_> = self
            .elements
            .iter()
            .filter(|e| !matches!(e, Element::Line(_)) && selection.contains(e.id()))
            .map(|e| (e.id().clone(), self.bounds(e)))
            .collect();
        let min = if matches!(a, Align::DistributeH | Align::DistributeV) {
            3
        } else {
            2
        };
        if items.len() < min {
            return vec![];
        }
        let left = items.iter().map(|i| i.1 .0).min().unwrap_or(0);
        let top = items.iter().map(|i| i.1 .1).min().unwrap_or(0);
        let right = items.iter().map(|i| i.1 .0 + i.1 .2).max().unwrap_or(0);
        let bottom = items.iter().map(|i| i.1 .1 + i.1 .3).max().unwrap_or(0);
        let mut moves: Vec<(Id, i32, i32)> = match a {
            Align::DistributeH | Align::DistributeV => {
                let h = a == Align::DistributeH;
                let (start, end) = if h { (left, right) } else { (top, bottom) };
                let size = |b: &(i32, i32, i32, i32)| if h { b.2 } else { b.3 };
                let mut sorted = items;
                sorted.sort_by_key(|i| if h { i.1 .0 } else { i.1 .1 });
                let free = end - start - sorted.iter().map(|i| size(&i.1)).sum::<i32>();
                let last = sorted.len() as i32 - 1;
                let mut at = start;
                sorted
                    .into_iter()
                    .enumerate()
                    .map(|(n, (id, b))| {
                        let to = at + free * n as i32 / last;
                        at += size(&b);
                        let d = to - if h { b.0 } else { b.1 };
                        (id, if h { d } else { 0 }, if h { 0 } else { d })
                    })
                    .collect()
            }
            _ => items
                .into_iter()
                .map(|(id, (x, y, w, h))| {
                    let (dx, dy) = match a {
                        Align::Left => (left - x, 0),
                        Align::Center => (left + (right - left - w) / 2 - x, 0),
                        Align::Right => (right - w - x, 0),
                        Align::Top => (0, top - y),
                        Align::Middle => (0, top + (bottom - top - h) / 2 - y),
                        _ => (0, bottom - h - y),
                    };
                    (id, dx, dy)
                })
                .collect(),
        };
        moves.retain(|m| (m.1, m.2) != (0, 0));
        moves
            .into_iter()
            .map(|(id, dx, dy)| Command::Move { id, dx, dy })
            .collect()
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
            Command::SetLabelAt { id, at } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Line(l) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut l.label_at, at);
                Ok(Command::SetLabelAt { id, at: old })
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
            Command::SetStroke { id, stroke } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let slot = match &mut self.elements[i] {
                    Element::Shape(s) => &mut s.stroke,
                    Element::Line(l) => &mut l.stroke,
                    Element::Text(_) => return Err(wrong(&id)),
                };
                let old = std::mem::replace(slot, stroke);
                Ok(Command::SetStroke { id, stroke: old })
            }
            Command::SetBorder { id, border } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.border, border);
                Ok(Command::SetBorder { id, border: old })
            }
            Command::SetColor { id, color } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let slot = match &mut self.elements[i] {
                    Element::Shape(s) => &mut s.color,
                    Element::Line(l) => &mut l.color,
                    Element::Text(t) => &mut t.color,
                };
                let old = std::mem::replace(slot, color);
                Ok(Command::SetColor { id, color: old })
            }
            Command::SetFill { id, fill } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.fill, fill);
                Ok(Command::SetFill { id, fill: old })
            }
            Command::SetAlign { id, align } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.align, align);
                Ok(Command::SetAlign { id, align: old })
            }
            Command::SetBold { id, bold } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Shape(s) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut s.bold, bold);
                Ok(Command::SetBold { id, bold: old })
            }
            Command::SetRouting { id, routing } => {
                let i = self.position(&id, index).ok_or_else(|| missing(&id))?;
                let Element::Line(l) = &mut self.elements[i] else {
                    return Err(wrong(&id));
                };
                let old = std::mem::replace(&mut l.routing, routing);
                Ok(Command::SetRouting { id, routing: old })
            }
            Command::SetGroups(groups) => Ok(Command::SetGroups(std::mem::replace(
                &mut self.groups,
                groups,
            ))),
            Command::Insert(..)
            | Command::Remove(_)
            | Command::MoveTo { .. }
            | Command::Batch(_) => {
                unreachable!()
            }
        }
    }
}
