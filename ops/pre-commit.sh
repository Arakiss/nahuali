#!/bin/sh
# Before a commit: the tree passes the privacy check, and the commit carries
# a public identity. Install with
#   ln -s ../../ops/pre-commit.sh .git/hooks/pre-commit
# Set NAHUALI_PRIVACY_DENYLIST to a file of private terms kept outside the
# repository to have those checked too (see docs/boundaries.md).
set -eu

root=$(git rev-parse --show-toplevel)
mail=$(git config user.email || true)
case $mail in
*@users.noreply.github.com | *@example.com) ;;
*)
	if [ "${NAHUALI_ALLOW_EMAIL:-}" != 1 ]; then
		echo "pre-commit: this commit would be signed with a personal e-mail address." >&2
		echo "  Set user.email to your GitHub noreply address (GitHub, Settings, Emails)." >&2
		echo "  Or, if it is meant to be public: NAHUALI_ALLOW_EMAIL=1 git commit …" >&2
		exit 1
	fi
	;;
esac

exec "$root/ops/privacy-check.sh" "$root"
