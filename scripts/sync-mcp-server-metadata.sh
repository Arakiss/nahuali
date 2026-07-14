#!/bin/sh
set -eu

mode="write"
case "${1:-}" in
  ""|--write)
    ;;
  --check)
    mode="check"
    ;;
  -h|--help)
    echo "Usage: sh scripts/sync-mcp-server-metadata.sh [--check|--write]"
    exit 0
    ;;
  *)
    echo "usage: sh scripts/sync-mcp-server-metadata.sh [--check|--write]" >&2
    exit 2
    ;;
esac

repo_root="${NAHUALI_WORKSPACE_ROOT:-}"
if [ -z "$repo_root" ]; then
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi
cd "$repo_root"

metadata_file="$(mktemp)"
trap 'rm -f "$metadata_file"' EXIT
cargo metadata --locked --format-version 1 --no-deps > "$metadata_file"

NAHUALI_METADATA_JSON="$metadata_file" \
NAHUALI_SYNC_MODE="$mode" \
ruby <<'RUBY'
require "json"

metadata = JSON.parse(File.read(ENV.fetch("NAHUALI_METADATA_JSON")))
package = metadata.fetch("packages").find { |item| item.fetch("name") == "nahuali-mcp" }
abort "nahuali-mcp is missing from cargo metadata" unless package

version = package.fetch("version")
path = "server.json"
document = JSON.parse(File.read(path))
expected_name = "io.github.Arakiss/nahuali"
expected_identifier = "ghcr.io/arakiss/nahuali-mcp:#{version}"
current_name = document["name"]
current_version = document["version"]
current_identifier = document.dig("packages", 0, "identifier")

if current_name == expected_name && current_version == version && current_identifier == expected_identifier
  puts "MCP server metadata synchronized at #{version}"
  exit 0
end

if ENV.fetch("NAHUALI_SYNC_MODE") == "check"
  warn "MCP server metadata is stale: name=#{current_name.inspect}, version=#{current_version.inspect}, identifier=#{current_identifier.inspect}; expected #{expected_name.inspect}, #{version.inspect}, and #{expected_identifier.inspect}"
  exit 1
end

document["name"] = expected_name
document["version"] = version
document.fetch("packages").fetch(0)["identifier"] = expected_identifier
File.write(path, JSON.pretty_generate(document) + "\n")
puts "MCP server metadata synchronized: #{current_version} -> #{version}"
RUBY
