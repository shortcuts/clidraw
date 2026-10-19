use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    // std has no rng; a fresh RandomState carries OS-seeded keys.
    pub fn random() -> Id {
        let n = RandomState::new().build_hasher().finish();
        let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
        Id((0..8)
            .map(|i| alphabet[((n >> (i * 8)) % 36) as usize] as char)
            .collect())
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Id {
        Id(s.to_string())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
