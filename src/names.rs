use std::time::{SystemTime, UNIX_EPOCH};

const ADJECTIVES: [&str; 24] = [
    "grumpy", "sleepy", "wobbly", "sneaky", "fluffy", "cranky", "dizzy", "jolly", "soggy",
    "bashful", "clumsy", "dapper", "frantic", "gloomy", "hasty", "itchy", "jittery", "lazy",
    "moody", "nervous", "plucky", "quirky", "rowdy", "smug",
];
const NOUNS: [&str; 24] = [
    "walrus", "badger", "otter", "llama", "pigeon", "narwhal", "goblin", "waffle", "pickle",
    "noodle", "muffin", "penguin", "hamster", "sloth", "turnip", "yak", "gecko", "moose", "potato",
    "ferret", "newt", "toaster", "beetle", "capybara",
];

/// A hyphenated adjective-noun name for which `taken` is false.
///
/// The clock seeds the start, so no rand dependency is needed. When every pair is taken,
/// the last candidate comes back and `Store::create` reports `Exists`.
pub fn generate(taken: impl Fn(&str) -> bool) -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as usize);
    let total = ADJECTIVES.len() * NOUNS.len();
    let at = |i: usize| {
        let i = (seed + i) % total;
        format!("{}-{}", ADJECTIVES[i / NOUNS.len()], NOUNS[i % NOUNS.len()])
    };
    (0..total)
        .map(at)
        .find(|n| !taken(n))
        .unwrap_or_else(|| at(0))
}
