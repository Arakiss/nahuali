//! Just enough argument parsing: words, `--flag`, `--name value` and
//! `--name=value`. A name that is not expected is an error, not a word.

#[derive(Debug, Default)]
pub struct Args {
    pub words: Vec<String>,
    values: Vec<(String, String)>,
    flags: Vec<String>,
}

impl Args {
    /// `valued` names take a value; `flags` take none. Both without dashes.
    pub fn parse(argv: &[String], valued: &[&str], flags: &[&str]) -> Result<Self, String> {
        let mut args = Self::default();
        let mut rest = argv.iter();
        while let Some(arg) = rest.next() {
            let Some(name) = arg.strip_prefix("--") else {
                args.words.push(arg.clone());
                continue;
            };
            if name.is_empty() {
                args.words.extend(rest.by_ref().cloned());
                break;
            }
            let (name, inline) = match name.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (name, None),
            };
            if valued.contains(&name) {
                let value = match inline {
                    Some(value) => value,
                    None => rest.next().cloned().ok_or_else(|| format!("--{name} takes a value"))?,
                };
                args.values.push((name.to_string(), value));
            } else if flags.contains(&name) && inline.is_none() {
                args.flags.push(name.to_string());
            } else {
                let known: Vec<&str> = valued.iter().chain(flags).copied().collect();
                let hint = meant(name, &known).map_or(String::new(), |m| format!("\n  did you mean --{m}?"));
                return Err(format!("--{name} is not something this command takes{hint}"));
            }
        }
        Ok(args)
    }

    /// The value of a name; the last one given wins.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// Every value given under a name, in order.
    pub fn values(&self, name: &str) -> Vec<String> {
        self.values.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

/// Names agents reach for that mean one this command has and look nothing
/// like it: the board shows an ask's work as `holds importer`. Seen in
/// `nahuali usage`. A slip of a letter or two needs no entry here.
const SAID_FOR: [(&str, &str); 2] = [("holds", "work"), ("hold", "work")];

/// The name that was most likely meant: one the board's words point to, or
/// one a slip of two letters away. None when nothing is that close.
fn meant<'a>(typed: &str, known: &[&'a str]) -> Option<&'a str> {
    if let Some(&(_, to)) = SAID_FOR.iter().find(|(from, to)| *from == typed && known.contains(to)) {
        return known.iter().copied().find(|k| *k == to);
    }
    if typed.chars().count() < 4 {
        return None;
    }
    known
        .iter()
        .map(|k| (apart(typed, k), *k))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// How many letters to add, remove or change to turn one word into the other.
fn apart(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (last + usize::from(ca != *cb)).min(row[j] + 1).min(here + 1);
            last = here;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn words_values_and_flags_are_told_apart() {
        let a = Args::parse(
            &argv("importer --done a --line=b --done c --private"),
            &["done", "line"],
            &["private"],
        )
        .unwrap();
        assert_eq!(a.words, ["importer"]);
        assert_eq!(a.values("done"), ["a", "c"]);
        assert_eq!(a.value("line"), Some("b"));
        assert!(a.has("private"));
        assert!(!a.has("json"));
    }

    #[test]
    fn what_is_not_expected_is_refused() {
        assert!(Args::parse(&argv("--nope"), &[], &[]).is_err());
        assert!(Args::parse(&argv("--line"), &["line"], &[]).is_err());
        assert!(Args::parse(&argv("--private=yes"), &[], &["private"]).is_err());
        let a = Args::parse(&argv("x -- --literal"), &[], &[]).unwrap();
        assert_eq!(a.words, ["x", "--literal"]);
    }

    #[test]
    fn a_name_that_was_meant_is_named() {
        let said = |line: &str, valued: &[&str]| Args::parse(&argv(line), valued, &[]).unwrap_err();
        assert_eq!(
            said("importer --state done", &["line", "status"]),
            "--state is not something this command takes\n  did you mean --status?"
        );
        assert!(said("q --holds importer", &["blocks", "work"]).ends_with("did you mean --work?"));
        assert!(said("x --stauts done", &["status"]).ends_with("did you mean --status?"), "a slip");
        assert_eq!(
            said("x --zzz", &["status"]),
            "--zzz is not something this command takes",
            "nothing close"
        );
        assert_eq!(
            said("x --line=a --state done", &["line"]),
            "--state is not something this command takes",
            "never a name the command does not have"
        );
    }
}
