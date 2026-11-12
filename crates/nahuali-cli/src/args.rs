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
                return Err(format!("--{name} is not something this command takes"));
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
}
