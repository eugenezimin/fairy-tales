//! Shared utilities used by the filesystem repository implementation.

// ── Slug generation ───────────────────────────────────────────────────────────

/// Convert arbitrary text into a URL-safe slug.
///
/// ```
/// assert_eq!(slugify("Hello, World!"), "hello-world");
/// ```
pub fn slugify(text: &str) -> String {
    let transliterated: String = text
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                transliterate_char(c).to_string()
            }
        })
        .collect();

    transliterated
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
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
// ── Transliteration ───────────────────────────────────────────────────────────

/// Transliterate a single Cyrillic character to its ASCII Latin equivalent.
fn transliterate_char(c: char) -> &'static str {
    match c {
        'а' | 'А' => "a",
        'б' | 'Б' => "b",
        'в' | 'В' => "v",
        'г' | 'Г' => "g",
        'д' | 'Д' => "d",
        'е' | 'Е' => "e",
        'ё' | 'Ё' => "yo",
        'ж' | 'Ж' => "zh",
        'з' | 'З' => "z",
        'и' | 'И' => "i",
        'й' | 'Й' => "y",
        'к' | 'К' => "k",
        'л' | 'Л' => "l",
        'м' | 'М' => "m",
        'н' | 'Н' => "n",
        'о' | 'О' => "o",
        'п' | 'П' => "p",
        'р' | 'Р' => "r",
        'с' | 'С' => "s",
        'т' | 'Т' => "t",
        'у' | 'У' => "u",
        'ф' | 'Ф' => "f",
        'х' | 'Х' => "kh",
        'ц' | 'Ц' => "ts",
        'ч' | 'Ч' => "ch",
        'ш' | 'Ш' => "sh",
        'щ' | 'Щ' => "sch",
        'ъ' | 'Ъ' => "",
        'ы' | 'Ы' => "y",
        'ь' | 'Ь' => "",
        'э' | 'Э' => "e",
        'ю' | 'Ю' => "yu",
        'я' | 'Я' => "ya",
        _ => "-",
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
