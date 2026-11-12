//! Identifiers that sort by the time they were made, without a counter two
//! writers would have to share: 48 bits of milliseconds, then 80 bits the
//! caller drew at random, as 26 characters (a ULID).

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn ulid(at_ms: u64, entropy: u128) -> String {
    let value = ((u128::from(at_ms) & ((1 << 48) - 1)) << 80) | (entropy & ((1 << 80) - 1));
    (0..26).map(|i| ALPHABET[((value >> (5 * (25 - i))) & 31) as usize] as char).collect()
}

/// The short name an ask is called by: the last `len` characters of its
/// event's id, in lower case. They are the random part.
pub fn short(id: &str, len: usize) -> String {
    id[id.len().saturating_sub(len)..].to_ascii_lowercase()
}

/// A name for a project or a piece of work: lower-case letters, digits and
/// dashes, starting with a letter or digit, at most 48 characters.
pub fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 48
        && !text.starts_with('-')
        && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The nearest slug to any name: `My Project_v2` becomes `my-project-v2`.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    out.chars().take(48).collect::<String>().trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_ids_sort_later() {
        let early = ulid(1_794_496_811_000, u128::MAX);
        let late = ulid(1_794_496_811_001, 0);
        assert_eq!(early.len(), 26);
        assert!(early < late);
    }

    #[test]
    fn names_become_slugs() {
        assert_eq!(slugify("My Project_v2"), "my-project-v2");
        assert_eq!(slugify("  --hello--  "), "hello");
        assert_eq!(slugify("***"), "");
        assert!(is_slug("pricing-page"));
        assert!(!is_slug("Pricing"));
        assert!(!is_slug("-x"));
        assert!(!is_slug(""));
    }
}
