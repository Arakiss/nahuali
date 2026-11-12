//! How a lesson's moment is recognised in what someone is about to do.
//!
//! No model is asked. Both texts become sets of plain terms, and a lesson
//! applies when the terms that mark its moment are all there. That makes
//! the answer the same every time, fast enough to run before every command,
//! and explainable: a lesson that is shown says which terms brought it.
//!
//! It is strict on purpose. A lesson that does not clearly belong to the
//! act is not shown, and "no lesson applies" is a correct answer.

use std::collections::BTreeSet;

/// Words that mark nothing, in English and Spanish.
const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "before", "after", "by", "do", "for", "from", "in", "into",
    "is", "it", "its", "not", "of", "on", "or", "own", "that", "the", "their", "then", "this", "to", "when",
    "with", "you", "your", "al", "antes", "con", "de", "del", "el", "en", "es", "la", "las", "lo", "los",
    "no", "para", "por", "que", "se", "su", "un", "una", "y",
];

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        other => other,
    }
}

/// The root of a word, roughly: `worktrees`, `spawning` and `pushed` become
/// `worktree`, `spawn` and `push`. Crude, and the same on both sides.
fn root(word: &str) -> String {
    for (ending, least) in [("ing", 4), ("ed", 4), ("s", 3)] {
        if let Some(stem) = word.strip_suffix(ending)
            && stem.chars().count() >= least
        {
            // `pushes` and `boxes` lose their whole ending; `worktrees` only its s.
            let hissing = ["sse", "she", "che", "xe", "ze"].iter().any(|h| stem.ends_with(h));
            return if ending == "s" && hissing {
                stem[..stem.len() - 1].to_string()
            } else {
                stem.to_string()
            };
        }
    }
    word.to_string()
}

/// The words of a text that mark something, as they were written: lower
/// case, accents folded, without the words that mark nothing and without
/// cutting them to a root. For whoever has a better root-finder of its own.
pub fn plain(text: &str) -> Vec<String> {
    let folded: String = text.to_lowercase().chars().map(fold).collect();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

/// The terms of a text: lower case, accents folded, split at anything that
/// is not a letter or a digit, without the words that mark nothing.
pub fn terms(text: &str) -> BTreeSet<String> {
    let folded: String = text.to_lowercase().chars().map(fold).collect();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP.contains(w))
        .map(root)
        .filter(|w| !w.is_empty())
        .collect()
}

/// Whether a moment is in what is about to be done, and by which terms.
///
/// With `when` given, one entry is enough and every term of that entry has
/// to be there. Without it, the moment's own words are used: most of them
/// have to be there, and never fewer than two unless it has only one.
pub fn marks(moment: &str, when: &[String], doing: &BTreeSet<String>) -> Option<Vec<String>> {
    for entry in when {
        let wanted = terms(entry);
        if !wanted.is_empty() && wanted.is_subset(doing) {
            return Some(wanted.into_iter().collect());
        }
    }
    if !when.is_empty() {
        return None;
    }
    let wanted = terms(moment);
    if wanted.is_empty() {
        return None;
    }
    let found: Vec<String> = wanted.intersection(doing).cloned().collect();
    let needed = (wanted.len() * 3).div_ceil(5).max(2).min(wanted.len());
    (found.len() >= needed).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(text: &str) -> Vec<String> {
        terms(text).into_iter().collect()
    }

    #[test]
    fn a_text_becomes_its_plain_terms() {
        assert_eq!(set("Before spawning agents in their own worktrees"), ["agent", "spawn", "worktree"]);
        assert_eq!(
            set("git worktree add ../atlas-side -b side"),
            ["add", "atla", "b", "git", "side", "worktree"]
        );
        // `atlas` loses its s: crude, and the same on both sides, so it still meets itself.
        assert_eq!(set("La decisión sobre el importador"), ["decision", "importador", "sobre"]);
        assert!(terms("the of and").is_empty());
    }

    #[test]
    fn an_entry_of_when_needs_all_of_its_terms() {
        let doing = terms("git worktree add ../side -b side");
        let when = vec!["worktree add".to_string(), "spawn agent".to_string()];
        assert_eq!(marks("whatever", &when, &doing), Some(vec!["add".to_string(), "worktree".to_string()]));
        assert_eq!(
            marks("whatever", &when, &terms("git worktree list")),
            None,
            "half an entry is not the moment"
        );
        assert_eq!(
            marks("worktree list", &when, &terms("git worktree list")),
            None,
            "with when given, the moment's words are not used"
        );
    }

    #[test]
    fn without_when_most_of_the_moment_has_to_be_there() {
        let moment = "before building in more than two worktrees";
        assert!(marks(moment, &[], &terms("cargo build in the third worktree, more to come")).is_some());
        assert!(marks(moment, &[], &terms("cargo build")).is_none(), "one word in five is not the moment");
        assert!(
            marks("deploying", &[], &terms("deploy to production")).is_some(),
            "a moment of one word needs that word"
        );
        assert!(
            marks("the", &[], &terms("anything")).is_none(),
            "a moment that marks nothing matches nothing"
        );
    }
}
