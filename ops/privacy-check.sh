#!/bin/sh
# No personal data in the tree: no absolute home path outside a few synthetic
# users, no e-mail address outside documentation domains, no image-generation
# session metadata in assets, and none of the terms of a private deny-list.
#
#   ops/privacy-check.sh [--exclude NAME]... [--message FILE] [DIR]
#   ops/privacy-check.sh --self-test
#
# DIR defaults to the repository root and does not have to be a git checkout:
# every regular text file under it is read, except .git/, target/ and
# node_modules/. --exclude skips every directory with that name (repeatable).
# --message FILE checks a commit message with the e-mail and deny-list rules
# (for a commit-msg hook) instead of a tree.
#
# The deny-list is never committed: NAHUALI_PRIVACY_DENYLIST names a file
# outside the repository with one term per line (blank lines and lines
# starting with # are ignored), matched as whole words, case-insensitively.
# Without it that rule is skipped, with a notice.
#
# ops/privacy-allow.txt (in DIR) may let through exact lines that must stay,
# one `path rule sha256-of-the-line  # reason` per line; see drop_allowed.
#
# Findings go to stderr as `path:line: rule`, never with the matched text, so
# the output itself leaks nothing. Exit 0 clean, 1 findings, 2 usage error.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
self="$here/ops/privacy-check.sh"

# Synthetic home users the tests use; anything else under /Users or /home is real.
ALLOWED_USERS='x someone p demo _nahuali runner'
# Domains reserved for documentation and tests (RFC 2606, RFC 6761), plus the
# host of the git remote form.
ALLOWED_MAIL='(^|\.)(example\.(com|org|net)|example|invalid|test|localhost|local)$|^github\.com$'
MAIL='[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}'

die() { echo "privacy: $*" >&2; exit 2; }

# grep -o lines "path:line:match" in, "path:line: rule" out for the bad ones.
filter() { # rule
  awk -v rule="$1" -v allowed_users="$ALLOWED_USERS" -v allowed_mail="$ALLOWED_MAIL" '
    BEGIN { n = split(allowed_users, u, " "); for (i = 1; i <= n; i++) ok[u[i]] = 1 }
    {
      p = index($0, ":"); path = substr($0, 1, p - 1); rest = substr($0, p + 1)
      q = index(rest, ":"); line = substr(rest, 1, q - 1); m = substr(rest, q + 1)
      bad = 1
      if (rule == "home-path") { sub(/^\/(Users|home)\//, "", m); bad = !(m in ok) }
      else if (rule == "email") { sub(/^.*@/, "", m); bad = (tolower(m) !~ allowed_mail) }
      if (bad) print path ":" line ": " rule
    }' | sort -u
}

scan_tree() { # dir grep-excludes...
  dir=$1; shift
  set -- --exclude=.git --exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules "$@"
  (
    cd "$dir"
    grep -rnoIE "$@" '/(Users|home)/[A-Za-z0-9._-]+' . | filter home-path
    grep -rnoIE "$@" '~/(Documents|Desktop|Downloads|Projects)/' . | filter home-dir
    grep -rnoIE "$@" "$MAIL" . | filter email
    if [ -d assets ]; then
      grep -rnoIE "$@" --include='*.json' '"(codex_thread|codex_source)"[[:space:]]*:' ./assets | filter sidecar-key
    fi
    if [ -n "$terms" ]; then
      grep -rnoIiwF "$@" -f "$terms" . | filter deny-list
    fi
  ) | sed 's#^\./##' | grep -v '^ops/privacy-check\.sh:' | drop_allowed "$dir" || true
}

# ops/privacy-allow.txt in the scanned tree lets through exact lines that have
# to stay: `path rule sha256-of-the-line  # why`. The hash pins the line, so an
# edit to it ends the allowance, and the file names no term it allows.
line_hash() { # dir path line
  sed -n "${3}p" "$1/$2" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -d' ' -f1
}
drop_allowed() { # dir; findings on stdin
  allow="$1/ops/privacy-allow.txt"
  if [ ! -f "$allow" ]; then cat; return; fi
  while IFS= read -r f; do
    path=${f%%:*}; rest=${f#*:}; line=${rest%%:*}; rule=${rest#*: }
    h=$(line_hash "$1" "$path" "$line")
    if grep -v -E '^[[:space:]]*(#|$)' "$allow" | awk -v p="$path" -v r="$rule" -v h="$h" \
      '$1 == p && $2 == r && $3 == h { found = 1 } END { exit !found }'; then
      continue
    fi
    printf '%s\n' "$f"
  done
}

scan_message() { # file
  grep -noIE "$MAIL" "$1" | sed 's#^#message:#' | filter email
  if [ -n "$terms" ]; then
    grep -noIiwF -f "$terms" "$1" | sed 's#^#message:#' | filter deny-list
  fi
}

load_terms() {
  terms=""
  list=${NAHUALI_PRIVACY_DENYLIST:-}
  if [ -z "$list" ]; then
    echo "privacy: NAHUALI_PRIVACY_DENYLIST is not set; the deny-list rule is skipped" >&2
    return
  fi
  [ -r "$list" ] || die "NAHUALI_PRIVACY_DENYLIST names $list, which cannot be read"
  case "$(cd "$(dirname "$list")" && pwd)/" in
    "$here"/*) die "the deny-list must live outside the repository" ;;
  esac
  terms=$(mktemp "${TMPDIR:-/tmp}/privacy-terms.XXXXXX")
  grep -v -E '^[[:space:]]*(#|$)' "$list" | sed 's/[[:space:]]*$//' >"$terms" || true
  if [ ! -s "$terms" ]; then
    rm -f "${terms:?}"
    terms=""
    echo "privacy: the deny-list is empty; the deny-list rule is skipped" >&2
  fi
}

# Each rule fires, alone, on a planted copy of a clean tree, and nothing fires
# on the clean tree itself.
plant_home_path() { printf 'path = "/Users/%s/notes"\n' "jdoe" >"$1/a.toml"; }
plant_home_dir() { printf 'see ~/Documents/private/x.md\n' >"$1/b.md"; }
plant_email() { printf 'author: %s@%s\n' "jdoe" "mailbox.org" >"$1/c.txt"; }
plant_sidecar_key() { printf '{"codex_thread": "0000"}\n' >"$1/assets/k.png.json"; }
plant_deny_list() { printf 'Invoice from zorblax for March\n' >"$1/d.rs"; }

self_test() {
  t=$(mktemp -d "${TMPDIR:-/tmp}/privacy-self.XXXXXX")
  trap 'rm -rf "${t:?}"' EXIT
  mkdir -p "$t/tree/assets" "$t/list"
  printf 'fn main() { let home = "/Users/x/.nahuali"; let mail = "ada@example.com"; }\n' >"$t/tree/clean.rs"
  printf '{"prompt_file": "assets/a.prompt.md"}\n' >"$t/tree/assets/ok.png.json"
  printf '# private terms\nZorblax Industries\nzorblax\n' >"$t/list/deny.txt"
  fail=0
  for rule in home-path home-dir email sidecar-key deny-list; do
    mkdir "$t/$rule"
    cp -R "$t/tree/." "$t/$rule/"
    "plant_$(echo "$rule" | tr - _)" "$t/$rule"
    if NAHUALI_PRIVACY_DENYLIST="$t/list/deny.txt" "$self" "$t/$rule" 2>"$t/err"; then
      echo "self-test: $rule did not fire" >&2; fail=1
    elif [ "$(grep -c ": $rule\$" "$t/err")" != 1 ] || grep ': [a-z-]*$' "$t/err" | grep -v -q ": $rule\$"; then
      echo "self-test: $rule fired the wrong rules: $(grep -o ': [a-z-]*$' "$t/err" | tr '\n' ' ')" >&2; fail=1
    else
      echo "self-test: $rule fires"
    fi
  done
  if NAHUALI_PRIVACY_DENYLIST="$t/list/deny.txt" "$self" "$t/tree" 2>"$t/err"; then
    echo "self-test: the clean tree passes"
  else
    echo "self-test: the clean tree fails: $(grep -o ': [a-z-]*$' "$t/err" | tr '\n' ' ')" >&2; fail=1
  fi
  if "$self" "$t/home-path" 2>"$t/err"; then
    echo "self-test: without a deny-list the other rules stop firing" >&2; fail=1
  elif ! grep -q 'deny-list rule is skipped' "$t/err"; then
    echo "self-test: a missing deny-list is not announced" >&2; fail=1
  else
    echo "self-test: without a deny-list it says so and still checks the rest"
  fi
  # An allow entry lets exactly its line through, and nothing once it changes.
  mkdir -p "$t/deny-list/ops"
  printf 'd.rs deny-list %s  # self-test\n' "$(line_hash "$t/deny-list" d.rs 1)" >"$t/deny-list/ops/privacy-allow.txt"
  if NAHUALI_PRIVACY_DENYLIST="$t/list/deny.txt" "$self" "$t/deny-list" 2>/dev/null; then
    echo "self-test: an allow entry lets its exact line through"
  else
    echo "self-test: an allow entry does not let its line through" >&2; fail=1
  fi
  printf 'Another invoice from zorblax\n' >"$t/deny-list/d.rs"
  if NAHUALI_PRIVACY_DENYLIST="$t/list/deny.txt" "$self" "$t/deny-list" 2>/dev/null; then
    echo "self-test: an allow entry survives an edit of its line" >&2; fail=1
  else
    echo "self-test: an allow entry ends when its line changes"
  fi
  printf 'Fix the thing\n\nThanks to Zorblax\n' >"$t/msg"
  if NAHUALI_PRIVACY_DENYLIST="$t/list/deny.txt" "$self" --message "$t/msg" 2>/dev/null; then
    echo "self-test: a commit message with a deny-listed term passes" >&2; fail=1
  else
    echo "self-test: deny-list fires on a commit message"
  fi
  [ "$fail" = 0 ] || exit 1
  echo "self-test: ok"
}

dir="" message="" excludes=""
while [ $# -gt 0 ]; do
  case $1 in
    --self-test) self_test; exit 0 ;;
    --exclude) [ $# -ge 2 ] || die "--exclude needs a directory name"; excludes="$excludes $2"; shift 2 ;;
    --message) [ $# -ge 2 ] || die "--message needs a file"; message=$2; shift 2 ;;
    -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) die "unknown option $1" ;;
    *) [ -z "$dir" ] || die "one directory only"; dir=$1; shift ;;
  esac
done

load_terms
if [ -n "$message" ]; then
  [ -r "$message" ] || die "cannot read $message"
  findings=$(scan_message "$message")
else
  dir=${dir:-$here}
  [ -d "$dir" ] || die "$dir is not a directory"
  set --
  for e in $excludes; do set -- "$@" --exclude-dir="$(basename "$e")"; done
  findings=$(scan_tree "$dir" "$@")
fi
if [ -n "$terms" ]; then rm -f "${terms:?}"; fi

if [ -n "$findings" ]; then
  printf '%s\n' "$findings" >&2
  echo "privacy: $(printf '%s\n' "$findings" | wc -l | tr -d ' ') finding(s) above (path:line: rule); the matched text is not printed" >&2
  exit 1
fi
echo "privacy: clean"
