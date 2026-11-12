//! The second and third parts: lessons that arrive at the moment of the
//! act, and notes that are recalled only when they clearly belong.

use std::collections::HashMap;

use nahuali_core::{Lesson, Mind, NewLesson, NewNote, Scope, Thing, Time};
use serde_json::json;

use crate::remote::Remote;
use crate::{Failure, env, open, parse, sign, sign_anywhere, text, usage, words};

/// `nahuali learn "RULE" --moment "WHEN" [--when "WORDS"]… [--everywhere] [--stop]`
pub fn learn(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["moment", "when"], &["everywhere", "stop"])?;
    let [rule] = words(
        &args,
        "learn takes one rule, in quotes: nahuali learn \"Agent worktrees start from the default branch\" --moment \"before spawning an agent in a worktree\"",
    )?;
    let moment = args
        .value("moment")
        .ok_or_else(|| usage("a lesson needs the moment it applies: --moment \"before …\""))?;
    let new = NewLesson {
        rule,
        moment: moment.to_string(),
        when: args.values("when"),
        everywhere: args.has("everywhere"),
        stop: args.has("stop"),
    };
    let mut mind = open(&args)?;
    let lesson = mind.learn(sign(&args)?, new)?;
    let reach = if lesson.everywhere { "every project".to_string() } else { lesson.marks.project.clone() };
    let marks = if lesson.when.is_empty() {
        format!("the words of its moment ({})", lesson.moment)
    } else {
        lesson.when.iter().map(|w| format!("\"{w}\"")).collect::<Vec<_>>().join(" or ")
    };
    let does = if lesson.stop { "stops the act once" } else { "is told" };
    Ok(format!(
        "lesson:{} · in force in {reach} · {does} when what is about to be done has {marks}\n",
        lesson.id
    ))
}

/// A lesson that applies to an act: by the words that mark its moment, or
/// only by being close to it in meaning.
pub struct Told<'a> {
    pub lesson: &'a Lesson,
    pub because: Vec<String>,
    /// How alike, when it was not the words that brought it.
    pub alike: Option<f64>,
}

/// The lessons that apply to what is about to be done. Those brought by
/// their words come first; then, with `meaning` and a model in this memory,
/// those close in meaning that the words missed. Meaning is for whoever
/// asks: on real commands, unasked, it brought lessons nobody needed.
pub fn told<'a>(mind: &'a Mind<Remote>, project: &str, doing: &str, meaning: bool) -> Vec<Told<'a>> {
    let scope = Scope::Project(project);
    let state = mind.state();
    let mut told: Vec<Told<'a>> = state
        .check(scope, doing)
        .into_iter()
        .map(|a| Told { lesson: a.lesson, because: a.because, alike: None })
        .collect();
    // A long script is close to everything and to nothing: only a command
    // short enough to be about one thing is compared by meaning.
    if !meaning || doing.chars().count() > 400 {
        return told;
    }
    // No model, or a keeper that does not answer: the words are enough.
    let near = mind
        .journal()
        .near(doing, "project", project, Some("lesson"), 3, None)
        .map(|(near, _)| near)
        .unwrap_or_default();
    for (key, alike) in near {
        let Some(lesson) = key.strip_prefix("lesson:").and_then(|name| state.lesson(name)) else { continue };
        if lesson.in_force()
            && scope.reaches(&lesson.marks, lesson.everywhere)
            && !told.iter().any(|t| t.lesson.id == lesson.id)
        {
            told.push(Told { lesson, because: Vec::new(), alike: Some(alike) });
        }
    }
    told
}

/// `nahuali check "WHAT I AM ABOUT TO DO"`
pub fn check(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["session"], &["json"])?;
    let [doing] = words(
        &args,
        "check takes what you are about to do, in quotes: nahuali check \"git worktree add ../side\"",
    )?;
    let mut mind = open(&args)?;
    let sig = sign(&args)?;
    let now = Time::from_millis(sig.at_ms);
    let (out, names) = {
        let applies = told(&mind, &sig.project, &doing, true);
        let out = if args.has("json") {
            let lessons: Vec<_> = applies
                .iter()
                .map(|a| {
                    json!({
                        "lesson": a.lesson.id, "rule": a.lesson.rule, "moment": a.lesson.moment, "stop": a.lesson.stop,
                        "because": a.because, "alike": a.alike, "shown": a.lesson.shown,
                    })
                })
                .collect();
            format!("{}\n", json!({ "applies": lessons }))
        } else if applies.is_empty() {
            "no lesson applies\n".to_string()
        } else {
            let mut out = format!("{} to this\n", count(applies.len(), "lesson applies", "lessons apply"));
            for a in &applies {
                out.push_str(&text::lesson_block(a.lesson, &a.because, a.alike, now));
            }
            out
        };
        (out, applies.iter().map(|a| a.lesson.id.clone()).collect::<Vec<String>>())
    };
    mind.shown(sig, names, &doing, args.value("session").map(str::to_string), false)?;
    Ok(out)
}

/// `nahuali lessons`: every lesson in force here.
pub fn lessons(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &["all"])?;
    let [] = words(&args, "lessons takes no words")?;
    let mind = open(&args)?;
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    let scope = if args.has("all") { Scope::AllFrom(&project) } else { Scope::Project(&project) };
    let lessons = mind.state().lessons(scope);
    if lessons.is_empty() {
        return Ok(format!("{project} · no lesson in force\n"));
    }
    let now = Time::from_millis(env::now_ms());
    let mut out = format!("{project} · {} in force\n", count(lessons.len(), "lesson", "lessons"));
    for lesson in lessons {
        out.push_str(&text::lesson_block(lesson, &[], None, now));
    }
    Ok(out)
}

/// `nahuali retire LESSON "WHY"`
pub fn retire(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let [lesson, why] = words(
        &args,
        "retire takes a lesson and the reason, in quotes: nahuali retire ab12cd \"no longer true since 2.0\"",
    )?;
    let mut mind = open(&args)?;
    let lesson = mind.retire(sign_anywhere(&args)?, &lesson, &why)?;
    Ok(format!("lesson:{} · retired · {}\n", lesson.id, lesson.rule))
}

/// `nahuali note "TEXT" [--tag T]… [--about WORK]`
pub fn note(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["tag", "about"], &[])?;
    let [said] = words(
        &args,
        "note takes one thing worth knowing, in quotes: nahuali note \"The audit is on the 20th\"",
    )?;
    let new =
        NewNote { text: said, tags: args.values("tag"), about: args.value("about").map(str::to_string) };
    let mut mind = open(&args)?;
    let note = mind.note(sign(&args)?, new)?;
    Ok(format!("note:{} · kept in {}\n", note.id, note.marks.project))
}

/// `nahuali forget NOTE "WHY"`
pub fn forget(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let [note, why] =
        words(&args, "forget takes a note and the reason, in quotes: nahuali forget ab12cd \"it moved\"")?;
    let mut mind = open(&args)?;
    let note = mind.forget(sign_anywhere(&args)?, &note, &why)?;
    Ok(format!("note:{} · forgotten · {}\n", note.id, note.text))
}

/// `nahuali recall "WHAT ABOUT"`: what is known about something. The
/// database finds what holds every word; an empty answer is a correct one.
pub fn recall(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["limit"], &["all", "json", "keys"])?;
    let [about] =
        words(&args, "recall takes what you want to know about, in quotes: nahuali recall \"audit date\"")?;
    let limit = match args.value("limit") {
        Some(n) => n.parse::<u64>().map_err(|_| usage("--limit takes a number"))?,
        None => 8,
    };
    let mind = open(&args)?;
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    let scope = if args.has("all") { Scope::AllFrom(&project) } else { Scope::Project(&project) };
    let asked: Vec<String> = nahuali_core::words::plain(&about);
    let keys = if asked.is_empty() {
        Vec::new()
    } else {
        mind.journal()
            .search(&asked.join(" "), if args.has("all") { "all" } else { "project" }, &project, limit)
            .map_err(|e| Failure::Refused(e.to_string()))?
    };
    // What the database answered, before anything here looks at it: for
    // whoever wants to see that its own reach is right.
    if args.has("keys") {
        let reach = if args.has("all") { "all" } else { "project" };
        let near = mind
            .journal()
            .near(&about, reach, &project, None, 5, None)
            .map(|(near, _)| near)
            .unwrap_or_default();
        let near = near.iter().filter(|(key, _)| !keys.contains(key)).map(|(key, _)| format!("near {key}\n"));
        return Ok(keys.iter().map(|k| format!("{k}\n")).chain(near).collect());
    }
    let mut held: HashMap<String, Thing> =
        mind.state().things().into_iter().map(|t| (t.key.clone(), t)).collect();
    // The database already left out what this project may not see. Checking
    // again here costs nothing and does not depend on the index being right.
    let found: Vec<Thing> = keys
        .iter()
        .filter_map(|k| held.remove(k))
        .filter(|t| scope.reaches(&t.marks, t.everywhere))
        .collect();
    // What the words missed and the meaning did not. Kept apart and said to
    // be so: it is a likeness, not a match.
    let near: Vec<(Thing, f64)> = mind
        .journal()
        .near(&about, if args.has("all") { "all" } else { "project" }, &project, None, 5, None)
        .map(|(near, _)| near)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, alike)| held.remove(&key).map(|thing| (thing, alike)))
        .filter(|(t, _)| scope.reaches(&t.marks, t.everywhere))
        .collect();

    let whose = if args.has("all") { "every project" } else { project.as_str() };
    if args.has("json") {
        let thing = |t: &Thing, alike: Option<f64>| {
            json!({
                "key": t.key, "kind": t.kind, "title": t.title, "detail": t.detail, "alike": alike,
                "marks": {
                    "who": t.marks.who, "sure": t.marks.sure, "at": t.marks.at,
                    "commit": t.marks.commit, "project": t.marks.project, "private": t.marks.private,
                },
            })
        };
        let found: Vec<_> = found.iter().map(|t| thing(t, None)).collect();
        let near: Vec<_> = near.iter().map(|(t, alike)| thing(t, Some(*alike))).collect();
        return Ok(format!("{}\n", json!({ "about": about, "found": found, "near": near })));
    }
    if found.is_empty() && near.is_empty() {
        return Ok(format!("{whose} · nothing known about \"{about}\"\n"));
    }
    let now = Time::from_millis(env::now_ms());
    let mut out = match (found.len(), near.len()) {
        (0, n) => format!(
            "{whose} · nothing holds the words \"{about}\"; {} close in meaning\n",
            count(n, "thing is", "things are")
        ),
        (f, 0) => format!("{whose} · {} known about \"{about}\"\n", count(f, "thing", "things")),
        (f, n) => format!(
            "{whose} · {} known about \"{about}\", and {n} more close in meaning\n",
            count(f, "thing", "things")
        ),
    };
    for thing in &found {
        out.push_str(&text::thing_block(thing, args.has("all"), None, now));
    }
    if !near.is_empty() {
        out.push_str("\nCLOSE IN MEANING (not the same words; judge them)\n");
        for (thing, alike) in &near {
            out.push_str(&text::thing_block(thing, args.has("all"), Some(*alike), now));
        }
    }
    Ok(out)
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
