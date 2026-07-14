#!/bin/sh
# Capture the public and maintainer-visible adoption signals for Nahuali.
#
# This script reads GitHub and MCP Registry data. It does not add product
# telemetry and it does not write anything unless the caller redirects stdout.
set -eu

REPO="${NAHUALI_GITHUB_REPO:-Arakiss/nahuali}"
MCP_NAME="${NAHUALI_MCP_NAME:-io.github.Arakiss/nahuali}"
REGISTRY_URL="${NAHUALI_MCP_REGISTRY_URL:-https://registry.modelcontextprotocol.io}"

command -v gh >/dev/null 2>&1 || {
  printf '%s\n' "error: gh is required" >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  printf '%s\n' "error: jq is required" >&2
  exit 1
}
command -v curl >/dev/null 2>&1 || {
  printf '%s\n' "error: curl is required" >&2
  exit 1
}

captured_at="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
repository="$(gh api "repos/${REPO}")"
views="$(gh api "repos/${REPO}/traffic/views")"
clones="$(gh api "repos/${REPO}/traffic/clones")"
releases="$(gh api "repos/${REPO}/releases?per_page=20")"
discussions="$(gh api graphql -f query='query($owner:String!,$name:String!){repository(owner:$owner,name:$name){discussions(first:100){totalCount}}}' -F owner="${REPO%/*}" -F name="${REPO#*/}")"
registry="$(curl -fsS --connect-timeout 10 --max-time 20 --get \
  --data-urlencode "search=${MCP_NAME}" \
  "${REGISTRY_URL}/v0.1/servers")"

jq -n \
  --arg captured_at "$captured_at" \
  --arg repository "$REPO" \
  --arg mcp_name "$MCP_NAME" \
  --argjson repository_data "$repository" \
  --argjson views "$views" \
  --argjson clones "$clones" \
  --argjson releases "$releases" \
  --argjson discussions "$discussions" \
  --argjson registry "$registry" \
  '{
    schema_version: 1,
    captured_at: $captured_at,
    repository: $repository,
    github: {
      stars: $repository_data.stargazers_count,
      forks: $repository_data.forks_count,
      watchers: $repository_data.subscribers_count,
      open_issues: $repository_data.open_issues_count,
      discussions: $discussions.data.repository.discussions.totalCount,
      traffic_14d: {
        views: $views.count,
        unique_visitors: $views.uniques,
        clones: $clones.count,
        unique_cloners: $clones.uniques
      },
      releases: [
        $releases[]
        | select(.tag_name | startswith("nahuali-cli-v"))
        | {
            tag: .tag_name,
            published_at: .published_at,
            assets: [.assets[] | {name: .name, downloads: .download_count}]
          }
      ]
    },
    mcp_registry: {
      name: $mcp_name,
      matches: [
        $registry.servers[]?
        | select((.server.name // .name) == $mcp_name)
      ]
    }
  }'
