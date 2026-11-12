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

/// The terms of a text, in the order they stand: lower case, accents
/// folded, split at anything that is not a letter or a digit, without the
/// words that mark nothing.
pub fn sequence(text: &str) -> Vec<String> {
    let folded: String = text.to_lowercase().chars().map(fold).collect();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP.contains(w))
        .map(root)
        .filter(|w| !w.is_empty())
        .collect()
}

/// The terms of a text, as a set.
pub fn terms(text: &str) -> BTreeSet<String> {
    sequence(text).into_iter().collect()
}

/// How far apart the words of one mark may stand: a command is a few
/// words, and a script of two hundred lines has every word somewhere.
const SPAN: usize = 8;

/// The most terms of `wanted` that stand within `span` terms of each other
/// in `doing`, and which they are.
fn together(wanted: &BTreeSet<String>, doing: &[String], span: usize) -> Vec<String> {
    let mut best: Vec<String> = Vec::new();
    for start in 0..doing.len() {
        if !wanted.contains(&doing[start]) {
            continue;
        }
        let window = &doing[start..doing.len().min(start + span)];
        let found: BTreeSet<&String> = window.iter().filter(|term| wanted.contains(*term)).collect();
        if found.len() > best.len() {
            best = found.into_iter().cloned().collect();
        }
    }
    best
}

/// How many other terms may stand between two terms of a mark: `push
/// origin main --force` is still "push force", and `cat Cargo.toml` ten
/// words before an `install` is not "cargo install".
const GAP: usize = 2;

/// Whether the terms of `phrase` stand in `doing` in their order, each
/// within a few terms of the one before.
fn in_order(phrase: &[String], doing: &[String]) -> bool {
    let Some((first, rest)) = phrase.split_first() else { return false };
    doing.iter().enumerate().filter(|(_, term)| *term == first).any(|(start, _)| {
        let mut at = start;
        rest.iter().all(|wanted| {
            let reach = doing.len().min(at + 2 + GAP);
            match doing[at + 1..reach].iter().position(|term| term == wanted) {
                Some(found) => {
                    at += 1 + found;
                    true
                }
                None => false,
            }
        })
    })
}

/// What someone is about to do, without the text it carries along: the
/// body of a here-document is something written, not something done.
pub fn act(command: &str) -> String {
    let mut out = String::new();
    let mut until: Option<String> = None;
    for line in command.lines() {
        if let Some(end) = &until {
            if line.trim() == end {
                until = None;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if let Some(at) = line.find("<<") {
            let word: String = line[at + 2..]
                .trim_start_matches('-')
                .trim_start()
                .trim_start_matches(['\'', '"', '\\'])
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !word.is_empty() && !line[at + 2..].starts_with('<') {
                until = Some(word);
            }
        }
    }
    out
}

/// Whether a moment is in what is about to be done, and by which terms.
/// `doing` is [`sequence`] of it: the order matters.
///
/// With `when` given, one entry is enough: its terms have to be there in
/// their order, almost next to each other, as a phrase is. Without it, the
/// moment's own words are used: most of them have to be there, near each
/// other in any order, and never fewer than two unless it has only one.
pub fn marks(moment: &str, when: &[String], doing: &[String]) -> Option<Vec<String>> {
    for entry in when {
        let phrase = sequence(entry);
        if in_order(&phrase, doing) {
            return Some(terms(entry).into_iter().collect());
        }
    }
    if !when.is_empty() {
        return None;
    }
    let wanted = terms(moment);
    if wanted.is_empty() {
        return None;
    }
    let found = together(&wanted, doing, SPAN.max(wanted.len() * 3));
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
        let doing = sequence("git worktree add ../side -b side");
        let when = vec!["worktree add".to_string(), "spawn agent".to_string()];
        assert_eq!(marks("whatever", &when, &doing), Some(vec!["add".to_string(), "worktree".to_string()]));
        assert_eq!(
            marks("whatever", &when, &sequence("git worktree list")),
            None,
            "half an entry is not the moment"
        );
        assert_eq!(
            marks("worktree list", &when, &sequence("git worktree list")),
            None,
            "with when given, the moment's words are not used"
        );
    }

    #[test]
    fn without_when_most_of_the_moment_has_to_be_there() {
        let moment = "before building in more than two worktrees";
        assert!(marks(moment, &[], &sequence("cargo build in the third worktree, more to come")).is_some());
        assert!(marks(moment, &[], &sequence("cargo build")).is_none(), "one word in five is not the moment");
        assert!(
            marks("deploying", &[], &sequence("deploy to production")).is_some(),
            "a moment of one word needs that word"
        );
        assert!(
            marks("the", &[], &sequence("anything")).is_none(),
            "a moment that marks nothing matches nothing"
        );
    }

    #[test]
    fn the_words_of_a_mark_have_to_stand_near_each_other() {
        let when = vec!["replace database".to_string()];
        assert!(marks("x", &when, &sequence("replace the database with files")).is_some());
        let script =
            format!("s.replace(a, b) {} print the database name", "and many other words here ".repeat(6));
        assert!(marks("x", &when, &sequence(&script)).is_none(), "a long script has every word somewhere");
        let moment = "before building in more than two worktrees";
        let far = format!("cargo build {} more worktree", "then run tests again and again ".repeat(6));
        assert!(marks(moment, &[], &sequence(&far)).is_none());
    }

    #[test]
    fn a_mark_is_a_phrase_in_its_order_and_almost_unbroken() {
        let mark = |when: &str, doing: &str| marks("x", &[when.to_string()], &sequence(doing)).is_some();
        assert!(mark("cargo install", "PATH=x cargo install --path crates/cli --locked"));
        assert!(!mark(
            "cargo install",
            "cat Cargo.toml && cat crates/laws/Cargo.toml && ls crates && npm install"
        ));
        assert!(mark("push force", "git push origin main --force"));
        assert!(!mark("push force", "git push origin main and then, much later, use the force"));
        assert!(
            !mark("worktree add", "git add . && git worktree list"),
            "the other way round is another phrase"
        );
        assert!(mark("migrate", "./migrate up"));
        assert!(!mark("the", "anything at all"));
    }

    #[test]
    fn what_a_command_writes_is_not_what_it_does() {
        let command = "python3 - <<'EOF'\ns = s.replace(a, b)  # git worktree add\nEOF\ngit status";
        assert_eq!(act(command), "python3 - <<'EOF'\ngit status\n");
        assert_eq!(
            act("cat > f.txt <<-END\n\tcargo install\n\tEND\necho done"),
            "cat > f.txt <<-END\necho done\n"
        );
        assert_eq!(act("git worktree add ../side"), "git worktree add ../side\n");
        assert_eq!(
            act("cat <<< 'cargo install'"),
            "cat <<< 'cargo install'\n",
            "a here-string is one line and stays"
        );
        assert_eq!(act("cat <<EOF\nnever closed"), "cat <<EOF\n");
    }
}
