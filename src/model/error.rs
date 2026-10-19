use super::Id;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum ModelError {
    NotFound(Id),
    WrongKind(Id),
    Duplicate(Id),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::NotFound(id) => write!(f, "element {id} not found"),
            ModelError::WrongKind(id) => write!(f, "element {id} has the wrong kind"),
            ModelError::Duplicate(id) => write!(f, "element {id} already exists"),
        }
    }
}

impl std::error::Error for ModelError {}
