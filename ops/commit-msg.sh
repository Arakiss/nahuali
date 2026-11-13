#!/bin/sh
# The commit message: a conventional commit (ops/release lint), and nothing
# private in it (ops/privacy-check.sh --message). Install with
#   ln -s ../../ops/commit-msg.sh .git/hooks/commit-msg
set -eu
root=$(git rev-parse --show-toplevel)
"$root/ops/release" lint --message "$1"
exec "$root/ops/privacy-check.sh" --message "$1"
