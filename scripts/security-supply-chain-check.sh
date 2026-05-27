#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

assert_file_contains() {
  local file="$1"
  local pattern="$2"
  local message="$3"

  if ! grep -Eq -- "$pattern" "$file"; then
    echo "$message" >&2
    exit 1
  fi
}

assert_file_not_contains() {
  local file="$1"
  local pattern="$2"
  local message="$3"

  if grep -Eq -- "$pattern" "$file"; then
    echo "$message" >&2
    exit 1
  fi
}

require_package_metadata() {
  local manifest="$1"
  local package="$2"

  assert_file_contains "$manifest" '^license\.workspace = true$' "$package must inherit the workspace license"
  assert_file_contains "$manifest" '^repository\.workspace = true$' "$package must inherit the workspace repository"
  assert_file_contains "$manifest" '^homepage\.workspace = true$' "$package must inherit the workspace homepage"
  assert_file_contains "$manifest" '^authors\.workspace = true$' "$package must inherit the workspace authors"
  assert_file_contains "$manifest" '^description = ".+"$' "$package must include a package description"
  assert_file_contains "$manifest" '^readme = "README\.md"$' "$package must include a crate README"
}

assert_file_contains LICENSE '^MIT License$' "LICENSE must be MIT"
assert_file_contains Cargo.toml '^license = "MIT"$' "workspace license must be MIT"
assert_file_contains Cargo.toml '^repository = "https://github.com/Arakiss/nahuali"$' "workspace repository metadata is missing"
assert_file_contains Cargo.toml '^homepage = "https://github.com/Arakiss/nahuali"$' "workspace homepage metadata is missing"

require_package_metadata crates/nahuali-core/Cargo.toml nahuali-core
require_package_metadata crates/nahuali-cli/Cargo.toml nahuali-cli
require_package_metadata crates/nahuali-mcp/Cargo.toml nahuali-mcp
require_package_metadata crates/nahuali-api/Cargo.toml nahuali-api
assert_file_contains crates/nahuali-regression/Cargo.toml '^publish = false$' "nahuali-regression must stay unpublished"

assert_file_contains .gitignore '^docs/$' "private docs directory must stay ignored"
assert_file_contains .gitignore '^\.private/$' "private local workspace must stay ignored"
assert_file_contains .gitignore '^\.local/$' "local workspace must stay ignored"
assert_file_contains .gitignore '^\.runs/$' "local run workspace must stay ignored"

private_path_pattern='(^docs/|^\.private/|^\.local/|^\.runs/)'
if git ls-tree -r --name-only HEAD | rg -n "$private_path_pattern"; then
  echo "current HEAD tracks private documentation or local workspace paths" >&2
  exit 1
fi
if git log --all --name-only --format= | sort -u | rg -n "$private_path_pattern"; then
  echo "git history contains private documentation or local workspace paths" >&2
  exit 1
fi

private_denylist="${NAHUALI_PRIVATE_DENYLIST:-.git/info/nahuali-private-denylist}"
if [[ -f "$private_denylist" ]]; then
  history_revs=()
  while IFS= read -r rev; do
    history_revs+=("$rev")
  done < <(git rev-list --all)

  while IFS= read -r private_pattern; do
    [[ -n "$private_pattern" ]] || continue
    [[ "$private_pattern" != \#* ]] || continue

    if git grep -n -I -i -E -- "$private_pattern" -- ':!Cargo.lock' ':!target/**'; then
      echo "private denylist scan failed for tracked content" >&2
      exit 1
    fi
    if git grep -n -I -i -E -- "$private_pattern" "${history_revs[@]}" -- ':!Cargo.lock' ':!target/**'; then
      echo "private denylist scan failed for historical content" >&2
      exit 1
    fi
    if git log --all --name-only --format= | sort -u | rg -n -i "$private_pattern"; then
      echo "private denylist scan failed for historical paths" >&2
      exit 1
    fi
    if git log --all --format='%H%x09%an%x09%ae%x09%cn%x09%ce' | rg -n -i "$private_pattern"; then
      echo "private denylist scan failed for git identity metadata" >&2
      exit 1
    fi
  done <"$private_denylist"
fi

assert_file_contains Cargo.toml '^nahuali-core = \{ version = "=[^"]+", path = "crates/nahuali-core" \}$' "nahuali-core must be pinned as an exact workspace dependency"
assert_file_contains crates/nahuali-cli/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-cli must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-mcp/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-mcp must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-api/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-api must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-regression/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-regression must use the workspace nahuali-core pin"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_PRIVATE_DRY_RUN_BIN_DIR: target/release' "CI private dry-run smoke must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_DOGFOOD_BIN_DIR: target/release' "CI dogfood smokes must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_RECALL_CONTRACT_BIN_DIR: target/release' "CI recall contract smoke must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'queued with the rest of CI' "release/install gate must stay independent from cheaper checks"
assert_file_contains .github/workflows/ci.yml 'github.event_name }}-\$\{\{ github.event.pull_request.number \|\| github.ref }}' "CI concurrency must isolate pull_request_target from main push runs"
assert_file_not_contains .github/workflows/ci.yml '^  push:' "CI push trigger must stay paused while GitHub billing is blocked"
assert_file_not_contains .github/workflows/ci.yml '^  pull_request' "CI pull request triggers must stay paused while GitHub billing is blocked"
assert_file_not_contains .github/workflows/audit.yml '^  (push|pull_request|pull_request_target|schedule):' "audit automatic triggers must stay paused while GitHub billing is blocked"
assert_file_not_contains .github/workflows/scorecard.yml '^  (branch_protection_rule|push|schedule):' "scorecard automatic triggers must stay paused while GitHub billing is blocked"
assert_file_not_contains .github/workflows/release.yml '^  push:' "release tag push trigger must stay paused while GitHub billing is blocked"
assert_file_not_contains .github/workflows/release.yml '^    branches: \[main\]' "release-please main push trigger must stay paused while GitHub billing is blocked"
assert_file_contains release-please-config.json '"prerelease-type": "beta"' "Release Please must emit beta prereleases for the beta release train"
assert_file_contains .github/workflows/release.yml 'googleapis/release-please-action@v5' "release workflow must use the current Release Please action"
assert_file_contains .github/workflows/release.yml 'release_please:' "release workflow must expose a manual Release Please dispatch while automatic main triggers are paused"
assert_file_contains .github/workflows/release.yml "inputs\\.release_please == true" "release workflow must route manual Release Please dispatches to the release-please job"
assert_file_contains .github/workflows/release.yml "inputs\\.release_please != true && inputs\\.tag != ''" "release workflow must keep manual binary builds separate from Release Please dispatches"
assert_file_contains .github/workflows/release.yml 'scripts/sync-workspace-internal-deps\.sh --check' "release workflow must verify internal workspace dependency pins before release-please"
assert_file_contains .github/workflows/release.yml 'Sync generated release PR workspace pins' "release workflow must repair generated release PR dependency pins"
assert_file_contains .github/workflows/release.yml 'scripts/tag-skipped-release-please-components.sh' "release workflow must tag skipped release-please components"
assert_file_contains .github/workflows/release.yml 'gh workflow run ci\.yml' "release workflow must dispatch CI for generated release PRs"
assert_file_contains scripts/tag-skipped-release-please-components.sh 'skip-github-release=true' "internal release tag script must target skipped Release Please components"
assert_file_contains scripts/validate-clean-tree.sh 'scripts/sync-workspace-internal-deps\.sh --check' "local validation must check workspace internal dependency pins"
assert_file_contains scripts/release-dry-run.sh 'crates/nahuali-cli/Cargo\.toml' "release dry-run must use the user-facing CLI version"
assert_file_contains scripts/check-release-assets.sh 'nahuali-v\$\{version\}-\$\{target\}\.tar\.gz' "release asset checker must verify versioned target archives"
assert_file_contains scripts/verify-release.sh 'cosign verify-blob' "release verifier must validate Sigstore bundles"
assert_file_contains scripts/verify-release.sh 'NAHUALI_VERIFY_INSTALL_BIN_DIR' "release verifier must run the install smoke against extracted binaries"
assert_file_contains scripts/release-candidate-check.sh 'scripts/check-release-assets\.sh' "release-candidate gate must check existing release assets"
assert_file_contains scripts/release-candidate-check.sh 'scripts/verify-release\.sh' "release-candidate gate must verify existing release signatures and install smoke"
assert_file_contains scripts/release-candidate-check.sh 'NAHUALI_RELEASE_CANDIDATE_REQUIRE_CURRENT_RELEASE' "release-candidate gate must expose the signed-release/current-HEAD alignment switch"
assert_file_contains scripts/release-candidate-check.sh 'NAHUALI_RELEASE_CANDIDATE_EXPECT_VISIBILITY' "release-candidate gate must make expected repository visibility explicit"
assert_file_contains scripts/validate-clean-tree.sh 'scripts/verify-recall-contract\.sh' "local validation must run the native recall contract smoke"
assert_file_contains scripts/validate-clean-tree.sh 'NAHUALI_DOGFOOD_BIN_DIR="\$release_bin_dir"' "local dogfood smokes must reuse release artifacts"
assert_file_contains scripts/validate-clean-tree.sh 'NAHUALI_RECALL_CONTRACT_BIN_DIR="\$release_bin_dir"' "local recall contract smoke must reuse release artifacts"
assert_file_contains scripts/verify-release-please-dry-run.sh 'git clone --quiet "\$ROOT"' "release-please dry-run helper must use a temporary clone"
assert_file_contains scripts/verify-release-please-dry-run.sh '--dry-run' "release-please dry-run helper must never mutate GitHub state"
assert_file_contains scripts/verify-release-please-dry-run.sh 'BUN_INSTALL_CACHE_DIR="\$tmp_cache"' "release-please dry-run helper must isolate the Bun install cache"
assert_file_contains scripts/verify-release-please-dry-run.sh 'gh auth token' "release-please dry-run helper must use the authenticated gh token consistently"
assert_file_contains scripts/fresh-clone-validate.sh '\.local/fresh-clone-cache' "fresh clone validation must keep Docker build caches under ignored local state"
assert_file_contains scripts/fresh-clone-validate.sh '/usr/local/cargo/registry' "fresh clone validation must cache Cargo registry downloads"
assert_file_contains scripts/fresh-clone-validate.sh '/tmp/nahuali-target' "fresh clone validation must cache Cargo target artifacts"
assert_file_contains scripts/verify-recall-contract.sh '--require-evidence' "native recall contract must require evidence-backed recall"
assert_file_contains scripts/verify-recall-contract.sh '--authority' "native recall contract must require authority context"
assert_file_contains scripts/verify-recall-contract.sh 'jq -e' "native recall contract must validate structured JSON output"
assert_file_contains .github/workflows/sbom.yml 'workflow_dispatch:' "SBOM workflow must support manual reruns for existing beta tags"
assert_file_contains .github/workflows/sbom.yml 'startsWith\(inputs\.tag, .nahuali-cli-v.\)' "SBOM workflow manual dispatch must stay scoped to nahuali-cli tags"
assert_file_contains .github/workflows/sbom.yml 'artifact-name: nahuali-\$\{\{ env\.RELEASE_TAG \}\}\.cdx\.json' "SBOM workflow must attach the canonical release SBOM asset"
assert_file_contains docker-compose.yml 'nofile:' "Qdrant dev stack must raise nofile for long release-candidate gates"
assert_file_contains scripts/ensure-dev-stack.sh 'qdrant_nofile_ready' "dev stack bootstrap must recreate Qdrant when nofile is missing"

sh scripts/sync-workspace-internal-deps.sh --check

readme_protected_recipe_pattern='(?i)(Quadrant|generic[[:space:]]+vector[[:space:]]+database|vector[[:space:]]+database|graph[[:space:]]+storage|graph[[:space:]]+store|record/graph|database/vector|storage/vector|dockerized|json[[:space:]]*l|json[[:space:]]+lines)'
if rg -n "$readme_protected_recipe_pattern" README.md; then
  echo "README contains unbounded implementation-recipe language outside the storage contract" >&2
  exit 1
fi

readme_hosted_overpromise_pattern='(?i)(Nahuali Cloud|public[[:space:]]+release[[:space:]]+(approved|ready)|ships[[:space:]]+with[[:space:]]+hosted|ships[[:space:]]+hosted|includes[[:space:]]+hosted[[:space:]]+operations|includes[[:space:]]+a[[:space:]]+hosted[[:space:]]+service|offers[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|provides[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|hosted[[:space:]]+control[[:space:]]+plane[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+deployment[[:space:]]+is[[:space:]]+part[[:space:]]+of|accounts[[:space:]]+are[[:space:]]+part[[:space:]]+of|billing[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+backup[[:space:]]+automation[[:space:]]+is[[:space:]]+included|point-in-time[[:space:]]+restore[[:space:]]+is[[:space:]]+included|SLA-backed[[:space:]]+recovery[[:space:]]+is[[:space:]]+included)'
if rg -n "$readme_hosted_overpromise_pattern" README.md; then
  echo "README contains hosted-product claims that need review" >&2
  exit 1
fi

legacy_line_format_pattern='(?i)(json[[:space:]_-]*l([^[:alpha:]]|$)|json[[:space:]]+lines)'
if rg -n --hidden \
  --glob '!.git/**' \
  --glob '!target/**' \
  --glob '!.private/**' \
  --glob '!.local/**' \
  --glob '!.runs/**' \
  --glob '!.dev-bin/**' \
  --glob '!.nahuali-oss/**' \
  --glob '!.release-dry-run/**' \
  --glob '!.nahuali-demo' \
  --glob '!*.snapshot.json' \
  --glob '!*.backup.json' \
  --glob '!*.interchange.json' \
  --glob '!Cargo.lock' \
  "$legacy_line_format_pattern" .; then
  echo "legacy line-oriented file format references are not part of this codebase" >&2
  exit 1
fi

cargo metadata --locked --format-version 1 >/dev/null
cargo tree --workspace --locked --duplicates >/dev/null

large_files="$(
  while IFS= read -r -d '' file; do
    [[ -f "$file" ]] || continue
    [[ "$file" != "Cargo.lock" ]] || continue

    size="$(wc -c <"$file" | tr -d '[:space:]')"
    if (( size > 1048576 )); then
      printf './%s\n' "$file"
    fi
  done < <(git ls-files -z)
)"
if [[ -n "$large_files" ]]; then
  echo "large tracked-or-source files need explicit review:" >&2
  echo "$large_files" >&2
  exit 1
fi

identity_pattern='(?i)([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|legal name|personal email)'
if rg -n -i --glob '!target/**' --glob '!.private/**' --glob '!.local/**' --glob '!.runs/**' --glob '!.dev-bin/**' --glob '!.nahuali-oss/**' --glob '!.release-dry-run/**' --glob '!.nahuali-demo' --glob '!*.snapshot.json' --glob '!*.backup.json' --glob '!*.interchange.json' --glob '!scripts/security-supply-chain-check.sh' --glob '!scripts/go-public-audit.sh' "$identity_pattern" .; then
  echo "identity scan failed" >&2
  exit 1
fi

secret_pattern='(?i)(api[_-]?key[[:space:]]*[:=][[:space:]]*[a-z0-9._-]{8,}|secret[_-]?key[[:space:]]*[:=][[:space:]]*[a-z0-9._-]{8,}|password[[:space:]]*[:=][[:space:]]*["'\'']?[a-z0-9._-]{8,}["'\'']?|bearer[[:space:]]+[a-z0-9._-]{16,}|sk-[a-z0-9]{20,}|ghp_[a-z0-9]{20,}|github_pat_[a-z0-9_]{20,}|AKIA[0-9A-Z]{16})'
if rg -n --hidden --glob '!.git/**' --glob '!target/**' --glob '!.private/**' --glob '!.local/**' --glob '!.runs/**' --glob '!.dev-bin/**' --glob '!.nahuali-oss/**' --glob '!.release-dry-run/**' --glob '!.nahuali-demo' --glob '!*.snapshot.json' --glob '!*.backup.json' --glob '!*.interchange.json' --glob '!Cargo.lock' --glob '!scripts/security-supply-chain-check.sh' --glob '!scripts/go-public-audit.sh' "$secret_pattern" .; then
  echo "secret scan failed" >&2
  exit 1
fi

publication_pattern='(cargo publish|npm publish|pnpm publish|bun publish|twine upload|gh release create|git tag)'
if rg -n --hidden --glob '.github/**' --glob 'scripts/**' --glob '!scripts/security-supply-chain-check.sh' --glob '!scripts/tag-skipped-release-please-components.sh' "$publication_pattern" .; then
  echo "publication command found in automation" >&2
  exit 1
fi

echo "security and supply-chain check passed"
