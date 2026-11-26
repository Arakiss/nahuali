//! The help of one verb, cut from the help page, so the two never say
//! different things; and what to say to someone who types a verb of the old
//! nahuali out of habit.

use crate::HELP;

/// What every refusal of a verb of the old nahuali says, after its name.
pub(crate) const OLD: &str = "was a verb of the old nahuali";

/// The verb whose help was asked for, if it was: `VERB --help`, `VERB -h`,
/// `help VERB` or `--help VERB`. Only what comes before `--` is looked at:
/// after it everything is words.
pub(crate) fn asked(argv: &[String]) -> Option<&str> {
    let (first, rest) = argv.split_first()?;
    if matches!(first.as_str(), "help" | "--help" | "-h") {
        return rest.first().map(String::as_str).filter(|verb| !verb.starts_with('-') && *verb != "help");
    }
    if first.starts_with('-') {
        return None;
    }
    rest.iter().take_while(|a| *a != "--").any(|a| a == "--help" || a == "-h").then_some(first.as_str())
}

/// The lines of the help page that speak of `verb`: where it is listed, the
/// flags it takes, and what every command takes. The whole page for a verb
/// it does not list on its own.
pub(crate) fn section(verb: &str) -> String {
    let blocks: Vec<&str> = HELP.split("\n\n").collect();
    let named = format!("  nahuali {verb}");
    let mut listed = Vec::new();
    for block in &blocks {
        let mut follows = false;
        for line in block.lines() {
            let starts =
                line.strip_prefix(&named).is_some_and(|after| after.is_empty() || after.starts_with(' '));
            // A line that goes on from the one before it is indented further.
            follows = starts || (follows && line.starts_with("    "));
            if follows {
                listed.push(line);
            }
        }
    }
    let own = blocks.iter().find(|b| b.split([' ', '\n']).next() == Some(verb));
    if listed.is_empty() && own.is_none() {
        return HELP.to_string();
    }
    let mut out = listed.join("\n") + "\n";
    if let Some(own) = own {
        out.push('\n');
        out.push_str(own.trim_end());
        out.push('\n');
    }
    if let Some(everywhere) = blocks.iter().find(|b| b.starts_with("everywhere\n")) {
        out.push('\n');
        out.push_str(everywhere.trim_end());
        out.push('\n');
    }
    out
}

/// What a verb of the old nahuali is now, in a few words.
fn now(verb: &str) -> Option<&'static str> {
    Some(match verb {
        "intention" | "intention-update" => {
            "a piece of work and what is true of it now is `nahuali update WORK --line \"…\"`"
        }
        "intention-status" => {
            "where a piece of work stands is `nahuali update WORK --status moving|not-started|done`"
        }
        "intention-complete" | "complete" => "work that is finished is `nahuali update WORK --status done`",
        "pending" | "intentions" => "what waits on the person and the work in motion is `nahuali status`",
        "reconcile-intentions" => {
            "the work in motion is `nahuali status`; a card that is out of date is set right with `nahuali update WORK`"
        }
        "episode" | "remember" | "claim" | "fact" => "something worth knowing is `nahuali note \"TEXT\"`",
        "relate" | "link" => "how something bears on a piece of work is `nahuali note \"TEXT\" --about WORK`",
        "procedure" | "preference" => {
            "a rule and the moment it applies is `nahuali learn \"RULE\" --moment \"WHEN\"`"
        }
        "timeline" | "inspect" | "show" => {
            "what is known about something is `nahuali recall \"ABOUT\"`; every event is `nahuali export`"
        }
        "backup" => "every event, to keep without nahuali, is `nahuali export`",
        _ => return None,
    })
}

/// The refusal of a verb nahuali does not have.
pub(crate) fn not_a_verb(typed: &str) -> String {
    match now(typed) {
        Some(now) => format!("`{typed}` {OLD}; {now}"),
        None => format!("`{typed}` is not something nahuali does"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn help_is_asked_for_in_every_way_an_agent_types_it() {
        for typed in
            ["setup --help", "setup -h", "help setup", "--help setup", "-h setup", "setup --hooks --help"]
        {
            assert_eq!(asked(&argv(typed)), Some("setup"), "{typed}");
        }
        for typed in [
            "help",
            "--help",
            "setup --hooks",
            "note -- --help",
            "--project atlas --help",
            "help --all",
            "help help",
        ] {
            assert_eq!(asked(&argv(typed)), None, "{typed}");
        }
    }

    #[test]
    fn a_section_is_the_help_page_cut_to_one_verb() {
        let update = section("update");
        assert!(update.starts_with("  nahuali update WORK …"), "{update}");
        assert!(update.contains("\nupdate WORK\n  --line"), "{update}");
        assert!(update.contains("\neverywhere\n  --project NAME"), "{update}");
        assert!(!update.contains("ask \"QUESTION\"\n"), "nothing of another verb: {update}");
        for line in update.lines().filter(|l| !l.is_empty()) {
            assert!(HELP.contains(line), "every line is a line of the help page: {line}");
        }

        let setup = section("setup");
        assert!(setup.contains("--hooks brings the memory to"), "{setup}");
        assert!(setup.contains("an agent by itself"), "what goes on to the next line goes with it: {setup}");
        assert!(setup.contains("--meaning"), "{setup}");
        assert!(!setup.contains("nahuali mcp"), "{setup}");

        let team = section("team");
        assert_eq!(team.lines().filter(|l| l.starts_with("  nahuali team")).count(), 5, "{team}");
        assert!(!section("forget").contains("nahuali node forget"));
        assert!(section("retire").starts_with("  nahuali retire LESSON"));
        assert_eq!(section("hook"), HELP, "what the page does not list on its own gets the whole page");
    }

    #[test]
    fn a_verb_of_the_old_nahuali_is_told_what_it_is_now() {
        let old = [
            "intention",
            "intention-complete",
            "intention-update",
            "intention-status",
            "intentions",
            "pending",
            "episode",
            "remember",
            "claim",
            "fact",
            "relate",
            "link",
            "procedure",
            "preference",
            "timeline",
            "inspect",
            "show",
            "backup",
            "reconcile-intentions",
            "complete",
        ];
        for verb in old {
            let said = not_a_verb(verb);
            assert!(said.starts_with(&format!("`{verb}` {OLD}; ")), "{said}");
            assert!(said.contains("`nahuali "), "it names what to use now: {said}");
        }
        assert_eq!(
            not_a_verb("pending"),
            format!("`pending` {OLD}; what waits on the person and the work in motion is `nahuali status`")
        );
        assert_eq!(not_a_verb("nope"), "`nope` is not something nahuali does");
    }
}
