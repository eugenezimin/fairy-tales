//! Shared utilities used by the filesystem repository implementation.

// ── Slug generation ───────────────────────────────────────────────────────────

/// Convert arbitrary text into a URL-safe slug.
///
/// ```
/// assert_eq!(slugify("Hello, World!"), "hello-world");
/// ```
pub fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Randomness ────────────────────────────────────────────────────────────────

/// Return a pseudo-random index in `0..len`.
///
/// Uses subsecond nanoseconds as a seed — good enough for "pick a random
/// article" but not cryptographically secure.
pub fn random_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % len
}

/// Fisher-Yates shuffle using a cheap hash-based seed.
pub fn shuffle<T>(slice: &mut [T]) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut h = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .hash(&mut h);
    std::thread::current().id().hash(&mut h);
    let mut state = h.finish();

    let n = slice.len();
    for i in (1..n).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        slice.swap(i, (state as usize) % (i + 1));
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("A--B"), "a-b");
    }
}
