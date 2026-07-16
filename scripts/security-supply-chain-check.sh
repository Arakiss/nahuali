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

  assert_file_contains "$manifest" '^license-file\.workspace = true$' "$package must inherit the workspace license-file"
  assert_file_contains "$manifest" '^repository\.workspace = true$' "$package must inherit the workspace repository"
  assert_file_contains "$manifest" '^homepage\.workspace = true$' "$package must inherit the workspace homepage"
  assert_file_contains "$manifest" '^authors\.workspace = true$' "$package must inherit the workspace authors"
  assert_file_contains "$manifest" '^version\.workspace = true$' "$package must inherit the one product version"
  assert_file_contains "$manifest" '^description = ".+"$' "$package must include a package description"
  assert_file_contains "$manifest" '^readme = "README\.md"$' "$package must include a crate README"
}

assert_file_contains LICENSE '^FSL-1\.1-MIT$' "LICENSE must be FSL-1.1-MIT"
assert_file_contains Cargo.toml '^license-file = "LICENSE"$' "workspace must reference the FSL LICENSE file"
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
assert_file_contains .gitignore '^\.nahual-rust/$' "Nahual-named local workspace must stay ignored"

private_path_pattern='(^docs/|^\.private/|^\.local/|^\.runs/)'
if git ls-tree -r --name-only HEAD | grep -En "$private_path_pattern"; then
  echo "current HEAD tracks private documentation or local workspace paths" >&2
  exit 1
fi
if git log --all --name-only --format= | sort -u | grep -En "$private_path_pattern"; then
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
    if git log --all --name-only --format= | sort -u | grep -Ein "$private_pattern"; then
      echo "private denylist scan failed for historical paths" >&2
      exit 1
    fi
    if git log --all --format='%H%x09%an%x09%ae%x09%cn%x09%ce' | grep -Ein "$private_pattern"; then
      echo "private denylist scan failed for git identity metadata" >&2
      exit 1
    fi
  done <"$private_denylist"
fi

assert_file_contains Cargo.toml '^nahuali-core = \{ version = "=[^"]+", path = "crates/nahuali-core" \} # x-release-please-version$' "nahuali-core must keep an exact publishable version and workspace path"
assert_file_contains Cargo.toml '^nahuali-ui = \{ version = "=[^"]+", path = "crates/nahuali-ui" \} # x-release-please-version$' "nahuali-ui must keep an exact publishable version and workspace path"
assert_file_contains crates/nahuali-cli/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-cli must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-mcp/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-mcp must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-api/Cargo.toml '^nahuali-core\.workspace = true$' "nahuali-api must use the workspace nahuali-core pin"
assert_file_contains crates/nahuali-regression/Cargo.toml '^nahuali-core = \{ workspace = true, features = \["regression-fixtures"\] \}$' "nahuali-regression must use the workspace nahuali-core pin with only the regression fixture seam"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_PRIVATE_DRY_RUN_BIN_DIR: target/release' "CI private dry-run smoke must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_DOGFOOD_BIN_DIR: target/release' "CI dogfood smokes must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_RECALL_CONTRACT_BIN_DIR: target/release' "CI recall contract smoke must reuse release artifacts"
assert_file_contains .github/workflows/ci.yml 'NAHUALI_REGRESSION_BIN_DIR: target/release' "CI regression fixtures must reuse the release regression runner"
assert_file_contains .github/workflows/ci.yml 'queued with the rest of CI' "release/install gate must stay independent from cheaper checks"
assert_file_contains .github/workflows/ci.yml 'github.event_name }}-\$\{\{ github.event.pull_request.number \|\| github.ref }}' "CI concurrency must isolate pull requests from main push runs"
assert_file_contains .github/workflows/ci.yml '^  push:' "CI must run on public main pushes"
assert_file_contains .github/workflows/ci.yml '^  pull_request:' "CI must run on public pull requests"
assert_file_not_contains .github/workflows/ci.yml 'pull_request_target' "CI must not use pull_request_target for public untrusted code"
assert_file_contains .github/workflows/ci.yml 'statuses: write' "CI rollup must be able to publish workflow_dispatch commit status for generated release PRs"
assert_file_contains .github/workflows/ci.yml 'TARGET_SHA: \$\{\{ github\.event\.pull_request\.head\.sha \|\| github\.sha \}\}' "CI rollup must select the exact pull request or dispatch head"
assert_file_contains .github/workflows/ci.yml 'repos/\$\{GITHUB_REPOSITORY\}/statuses/\$\{TARGET_SHA\}' "CI rollup must publish results as a visible commit status on the selected head"
assert_file_not_contains .github/workflows/audit.yml '^  (push|pull_request|pull_request_target|schedule):' "audit automatic triggers stay manual until the public security cadence is chosen"
assert_file_not_contains .github/workflows/audit.yml 'pull_request_target' "audit workflow must not keep pull_request_target-era checkout logic"
assert_file_not_contains .github/workflows/scorecard.yml '^  (branch_protection_rule|push|schedule):' "scorecard automatic triggers stay manual until the public security cadence is chosen"
assert_file_contains .github/workflows/release.yml '^  push:' "release workflow must run from controlled public push triggers"
assert_file_contains .github/workflows/release.yml '^    branches: \[main\]' "Release Please must update release PRs from main pushes"
assert_file_contains .github/workflows/release.yml '"v\*"' "release binary builds must stay scoped to product tags"
assert_file_contains .github/workflows/release.yml 'cancel-in-progress: false' "release workflow must not cancel in-flight tag artifact uploads"
assert_file_contains .github/workflows/release.yml 'attestations: write' "release binary builds must be allowed to publish artifact attestations"
assert_file_contains .github/workflows/release.yml 'actions/attest@v4' "release binary builds must generate GitHub artifact attestations"
assert_file_contains .github/workflows/release.yml 'subject-path: dist/\$\{\{ env\.ARCHIVE_BASENAME \}\}\.tar\.gz' "release attestations must bind the published platform archive"
assert_file_contains release-please-config.json '"prerelease-type": "beta"' "Release Please must emit beta prereleases for the beta release train"
assert_file_contains release-please-config.json '"bump-minor-pre-major": true' "Release Please must keep breaking pre-1.0 changes on the 0.x train"
assert_file_contains release-please-config.json '"versioning": "prerelease"' "Release Please must advance beta iterations instead of changing the product line implicitly"
assert_file_contains release-please-config.json '"component": "nahuali"' "Release Please must model one public product"
assert_file_contains .github/workflows/release.yml 'googleapis/release-please-action@v5' "release workflow must use the current Release Please action"
assert_file_contains .github/workflows/release.yml 'release_please:' "release workflow must expose a manual Release Please rerun"
assert_file_contains .github/workflows/release.yml "inputs\\.release_please == true" "release workflow must route manual Release Please dispatches to the release-please job"
assert_file_contains .github/workflows/release.yml "inputs\\.release_please != true && inputs\\.tag != ''" "release workflow must keep manual binary builds separate from Release Please dispatches"
assert_file_contains .github/workflows/release.yml 'scripts/check-version-policy\.sh' "release workflow must verify the product version policy before release-please"
assert_file_contains .github/workflows/release.yml 'scripts/sync-mcp-server-metadata\.sh' "release workflow must synchronize MCP package metadata in generated release PRs"
assert_file_contains .github/workflows/release.yml 'Sync generated release PR metadata' "release workflow must repair generated release PR metadata"
assert_file_contains .github/workflows/release.yml 'gh workflow run ci\.yml' "release workflow must dispatch CI for generated release PRs"
assert_file_contains .github/workflows/release.yml 'gh workflow run sbom\.yml' "release workflow must explicitly dispatch the CLI SBOM build"
assert_file_not_contains .github/workflows/release.yml 'for component in nahuali-core nahuali-mcp nahuali-api nahuali-regression' "release workflow must not multiply public component tags"
assert_file_contains README.md 'NAHUALI_VERIFY_GITHUB_SETTINGS=1 bash scripts/security-supply-chain-check.sh' "README must document the repository settings verification command"
assert_file_contains scripts/validate-clean-tree.sh 'scripts/check-version-policy\.sh' "local validation must check the product version policy"
assert_file_contains scripts/validate-clean-tree.sh 'scripts/sync-mcp-server-metadata\.sh --check' "local validation must check MCP server release metadata"
assert_file_contains scripts/release-dry-run.sh 'version\.txt' "release dry-run must use the canonical product version"
assert_file_contains scripts/check-release-assets.sh 'nahuali-v\$\{version\}-\$\{target\}\.tar\.gz' "release asset checker must verify versioned target archives"
assert_file_contains scripts/check-release-assets.sh '"required_assets": 13' "release asset checker must require all platform artifacts and the CycloneDX SBOM"
assert_file_contains scripts/check-release-assets.sh '\[ "\$sbom_count" -ne 1 \]' "release asset checker must fail when the CycloneDX SBOM is missing"
assert_file_contains scripts/check-release-page.sh 'Release Please changelog' "release page checker must reject raw generated changelog pages"
assert_file_contains scripts/check-release-page.sh 'Verify the release' "release page checker must require a verification section"
assert_file_contains scripts/check-release-page.sh 'Beta limits' "release page checker must require explicit beta limits"
assert_file_contains scripts/verify-release.sh 'cosign verify-blob' "release verifier must validate Sigstore bundles"
assert_file_contains scripts/verify-release.sh 'gh attestation verify' "release verifier must validate GitHub artifact provenance"
assert_file_contains scripts/verify-release.sh '\|\| "\$sbom_status" != "pass"' "release verifier must fail when the CycloneDX SBOM is missing"
assert_file_contains scripts/verify-release.sh '\|\| "\$provenance_status" != "pass"' "release verifier must fail when GitHub provenance is missing"
assert_file_not_contains scripts/verify-release.sh 'status="warn"' "release verifier must not downgrade missing supply-chain evidence to a warning"
assert_file_contains scripts/verify-release.sh 'NAHUALI_VERIFY_INSTALL_BIN_DIR' "release verifier must run the install smoke against extracted binaries"
assert_file_contains scripts/verify-install.sh 'export NAHUALI_HOME="\$STORE_DIR/home"' "install smoke must not contend with an operator store"
assert_file_contains scripts/release-candidate-check.sh 'scripts/check-release-page\.sh' "release-candidate gate must check the public release page"
assert_file_contains scripts/release-candidate-check.sh 'scripts/check-release-assets\.sh' "release-candidate gate must check existing release assets"
assert_file_contains scripts/release-candidate-check.sh 'scripts/verify-release\.sh' "release-candidate gate must verify existing release signatures and install smoke"
assert_file_contains scripts/release-candidate-check.sh '--require-sbom' "release-candidate gate must explicitly require the CycloneDX SBOM"
assert_file_contains scripts/release-candidate-check.sh '--require-provenance' "release-candidate gate must explicitly require GitHub provenance"
assert_file_contains scripts/release-candidate-check.sh 'NAHUALI_RELEASE_CANDIDATE_REQUIRE_CURRENT_RELEASE' "release-candidate gate must expose the signed-release/current-HEAD alignment switch"
assert_file_contains scripts/release-candidate-check.sh 'NAHUALI_RELEASE_CANDIDATE_EXPECT_VISIBILITY' "release-candidate gate must make expected repository visibility explicit"
assert_file_contains scripts/validate-clean-tree.sh 'scripts/verify-recall-contract\.sh' "local validation must run the native recall contract smoke"
assert_file_contains scripts/validate-clean-tree.sh 'NAHUALI_DOGFOOD_BIN_DIR="\$release_bin_dir"' "local dogfood smokes must reuse release artifacts"
assert_file_contains scripts/validate-clean-tree.sh 'NAHUALI_RECALL_CONTRACT_BIN_DIR="\$release_bin_dir"' "local recall contract smoke must reuse release artifacts"
assert_file_contains scripts/release-dry-run.sh 'nahuali-regression' "release dry-run must prebuild the regression runner used by fixture gates"
assert_file_contains .github/workflows/ci.yml 'Verify regression runner binary' "CI must verify the prebuilt release regression runner"
assert_file_contains scripts/validate-clean-tree.sh 'Regression runner release binary' "local validation must verify the prebuilt release regression runner"
assert_file_contains scripts/validate-clean-tree.sh 'NAHUALI_REGRESSION_BIN_DIR="\$release_bin_dir"' "local regression fixtures must reuse the release regression runner"
assert_file_contains scripts/run-regression-fixture.sh 'NAHUALI_REGRESSION_BIN_DIR' "regression fixture wrapper must support release runner reuse"
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
assert_file_contains .github/workflows/sbom.yml 'startsWith\(inputs\.tag, .v.\)' "SBOM workflow manual dispatch must stay scoped to product tags"
assert_file_contains .github/workflows/ci.yml 'cargo llvm-cov --locked --workspace --lcov' "CI must produce real workspace coverage"
assert_file_contains .github/workflows/ci.yml 'use_oidc: true' "Codecov uploads must use short-lived OIDC identity"
assert_file_contains .github/workflows/ci.yml 'id-token: write' "the coverage job must be able to request an OIDC token"
assert_file_contains .github/workflows/sbom.yml 'artifact-name: nahuali-\$\{\{ env\.RELEASE_TAG \}\}\.cdx\.json' "SBOM workflow must attach the canonical release SBOM asset"
assert_file_contains .github/workflows/sbom.yml 'gh release upload .*\$\{RELEASE_TAG\}.*--clobber' "SBOM workflow dispatches must explicitly attach the generated file to the selected release"
assert_file_contains docker-compose.yml 'nofile:' "Qdrant dev stack must raise nofile for long release-candidate gates"
assert_file_contains docker-compose.yml 'container_name: nahual-mictlan-surrealdb' "SurrealDB dev container must use the Nahual universe name"
assert_file_contains docker-compose.yml 'container_name: nahual-tonalli-qdrant' "Qdrant dev container must use the Nahual universe name"
assert_file_contains scripts/ensure-dev-stack.sh 'qdrant_nofile_ready' "dev stack bootstrap must recreate Qdrant when nofile is missing"
assert_file_contains scripts/ensure-dev-stack.sh 'LEGACY_SURREAL_CONTAINER="nahuali-oss-surrealdb"' "dev stack bootstrap must stop the legacy Rust SurrealDB container"
assert_file_contains scripts/ensure-dev-stack.sh 'LEGACY_QDRANT_CONTAINER="nahuali-oss-qdrant"' "dev stack bootstrap must stop the legacy Rust Qdrant container"
assert_file_contains README.md 'scripts/verify-controlled-beta\.sh' "README must document the controlled beta gate"
assert_file_contains BETA.md 'controlled beta gate' "controlled beta checklist must define the beta gate"
assert_file_contains scripts/verify-controlled-beta.sh 'scripts/security-supply-chain-check\.sh' "controlled beta gate must include security checks"
assert_file_contains scripts/verify-controlled-beta.sh 'scripts/verify-dogfood-daily-workflow\.sh' "controlled beta gate must include the daily-driver reliability gate"
assert_file_contains scripts/verify-controlled-beta.sh 'scripts/verify-recall-contract\.sh' "controlled beta gate must include the evidence-backed recall contract"

bash scripts/check-version-policy.sh

readme_protected_recipe_pattern='(Quadrant|generic[[:space:]]+vector[[:space:]]+database|vector[[:space:]]+database|graph[[:space:]]+storage|graph[[:space:]]+store|record/graph|database/vector|storage/vector|dockerized|json[[:space:]]*l|json[[:space:]]+lines)'
if grep -Ein "$readme_protected_recipe_pattern" README.md BETA.md; then
  echo "public docs contain unbounded implementation-recipe language outside the storage contract" >&2
  exit 1
fi

readme_hosted_overpromise_pattern='(Nahuali Cloud|public[[:space:]]+release[[:space:]]+(approved|ready)|ships[[:space:]]+with[[:space:]]+hosted|ships[[:space:]]+hosted|includes[[:space:]]+hosted[[:space:]]+operations|includes[[:space:]]+a[[:space:]]+hosted[[:space:]]+service|offers[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|provides[^.\n]*(hosted|managed|accounts|teams|billing|sync|dashboards)|hosted[[:space:]]+control[[:space:]]+plane[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+deployment[[:space:]]+is[[:space:]]+part[[:space:]]+of|accounts[[:space:]]+are[[:space:]]+part[[:space:]]+of|billing[[:space:]]+is[[:space:]]+part[[:space:]]+of|managed[[:space:]]+backup[[:space:]]+automation[[:space:]]+is[[:space:]]+included|point-in-time[[:space:]]+restore[[:space:]]+is[[:space:]]+included|SLA-backed[[:space:]]+recovery[[:space:]]+is[[:space:]]+included)'
if grep -Ein "$readme_hosted_overpromise_pattern" README.md BETA.md; then
  echo "public docs contain hosted-product claims that need review" >&2
  exit 1
fi

legacy_line_format_pattern='(json[[:space:]_-]*l([^[:alpha:]]|$)|json[[:space:]]+lines)'
if git grep -n -I -i -E -- "$legacy_line_format_pattern" -- \
  ':!Cargo.lock' ':!*.snapshot.json' ':!*.backup.json' ':!*.interchange.json'; then
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

identity_pattern='([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|legal name|personal email)'
if git grep -n -I -i -E -- "$identity_pattern" -- \
  ':!*.snapshot.json' ':!*.backup.json' ':!*.interchange.json' \
  ':!scripts/security-supply-chain-check.sh' ':!scripts/go-public-audit.sh'; then
  echo "identity scan failed" >&2
  exit 1
fi

secret_pattern='(api[_-]?key[[:space:]]*[:=][[:space:]]*[a-z0-9._-]{8,}|secret[_-]?key[[:space:]]*[:=][[:space:]]*[a-z0-9._-]{8,}|password[[:space:]]*[:=][[:space:]]*["'\'']?[a-z0-9._-]{8,}["'\'']?|bearer[[:space:]]+[a-z0-9._-]{16,}|sk-[a-z0-9]{20,}|ghp_[a-z0-9]{20,}|github_pat_[a-z0-9_]{20,}|AKIA[0-9A-Z]{16})'
if git grep -n -I -i -E -- "$secret_pattern" -- \
  ':!Cargo.lock' ':!*.snapshot.json' ':!*.backup.json' ':!*.interchange.json' \
  ':!scripts/security-supply-chain-check.sh' ':!scripts/go-public-audit.sh'; then
  echo "secret scan failed" >&2
  exit 1
fi

publication_pattern='(cargo publish|npm publish|pnpm publish|bun publish|twine upload|gh release create|git tag)'
if git grep -n -I -E -- "$publication_pattern" -- '.github/**' 'scripts/**' \
  ':!scripts/security-supply-chain-check.sh'; then
  echo "publication command found in automation" >&2
  exit 1
fi

if [[ "${NAHUALI_VERIFY_GITHUB_SETTINGS:-0}" == "1" ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "gh CLI is required when NAHUALI_VERIFY_GITHUB_SETTINGS=1" >&2
    exit 1
  fi
  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required when NAHUALI_VERIFY_GITHUB_SETTINGS=1" >&2
    exit 1
  fi

  github_repo="${NAHUALI_GITHUB_REPOSITORY:-Arakiss/nahuali}"
  workflow_permissions="$(gh api "repos/${github_repo}/actions/permissions/workflow")"
  default_permissions="$(printf '%s' "$workflow_permissions" | jq -r '.default_workflow_permissions')"
  can_create_prs="$(printf '%s' "$workflow_permissions" | jq -r '.can_approve_pull_request_reviews')"

  if [[ "$default_permissions" != "read" ]]; then
    echo "GitHub Actions default workflow permissions must stay read-only for ${github_repo}" >&2
    exit 1
  fi
  if [[ "$can_create_prs" != "true" ]]; then
    echo "GitHub Actions must be allowed to create pull requests so Release Please can maintain the release PR for ${github_repo}" >&2
    exit 1
  fi
fi

echo "security and supply-chain check passed"
