#!/bin/sh
set -eu

mode="write"
case "${1:-}" in
  "")
    ;;
  --check)
    mode="check"
    shift
    ;;
  --write)
    mode="write"
    shift
    ;;
  -h|--help)
    cat <<'EOF'
Usage: sh scripts/sync-workspace-internal-deps.sh [--check|--write]

Synchronize exact root [workspace.dependencies] versions for internal
nahuali-* crates from the package versions reported by cargo metadata.

--check  verify that the workspace is already synchronized
--write  update Cargo.toml in place (default)
EOF
    exit 0
    ;;
  *)
    echo "usage: sh scripts/sync-workspace-internal-deps.sh [--check|--write]" >&2
    exit 2
    ;;
esac

if [ "$#" -ne 0 ]; then
  echo "usage: sh scripts/sync-workspace-internal-deps.sh [--check|--write]" >&2
  exit 2
fi

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

mode = ENV.fetch("NAHUALI_SYNC_MODE")
metadata = JSON.parse(File.read(ENV.fetch("NAHUALI_METADATA_JSON")))
workspace_ids = metadata.fetch("workspace_members")
packages = metadata.fetch("packages").select { |package| workspace_ids.include?(package.fetch("id")) }
versions = packages.to_h { |package| [package.fetch("name"), package.fetch("version")] }
internal_versions = versions.select { |name, _version| name.start_with?("nahuali-") }

structural_failures = []
stale_failures = []
referenced_internal = {}
checked_edges = 0

packages.each do |package|
  package.fetch("dependencies", []).each do |dependency|
    name = dependency.fetch("name")
    expected_version = internal_versions[name]
    next unless expected_version

    referenced_internal[name] = true
    checked_edges += 1
    expected_req = "=#{expected_version}"
    actual_req = dependency.fetch("req")
    if actual_req != expected_req
      stale_failures << "#{package.fetch("name")}: #{name} uses #{actual_req.inspect}; expected #{expected_req.inspect}"
    end

    unless dependency["path"]
      structural_failures << "#{package.fetch("name")}: #{name} must point at the local workspace path"
    end
  end
end

root_manifest = ENV.fetch("NAHUALI_ROOT_CARGO_TOML", "Cargo.toml")
root_text = File.read(root_manifest)
in_workspace_dependencies = false
root_internal_seen = {}
updates = []

rewritten = root_text.each_line.with_index(1).map do |line, line_number|
  if (section = line.match(/^\s*\[([^\]]+)\]\s*$/))
    in_workspace_dependencies = section[1] == "workspace.dependencies"
    next line
  end

  next line unless in_workspace_dependencies

  assignment = line.match(/^(\s*)(nahuali-[A-Za-z0-9_-]+)(\s*=\s*)(.+?)(\s*(?:#.*)?)$/)
  next line unless assignment

  name = assignment[2]
  expected_version = internal_versions[name]
  next line unless expected_version

  root_internal_seen[name] = true
  body = assignment[4]
  expected_req = "=#{expected_version}"
  version = body[/\bversion\s*=\s*"([^"]+)"/, 1]
  path = body[/\bpath\s*=\s*"([^"]+)"/, 1]

  unless body.strip.start_with?("{") && body.strip.end_with?("}")
    structural_failures << "#{root_manifest}:#{line_number}: workspace dependency #{name} must use an inline table with version and path"
  end
  if version.nil?
    structural_failures << "#{root_manifest}:#{line_number}: workspace dependency #{name} must include an exact version requirement"
  elsif version != expected_req
    stale_failures << "#{root_manifest}:#{line_number}: workspace dependency #{name} uses #{version.inspect}; expected #{expected_req.inspect}"
    line = line.sub(/\bversion\s*=\s*"[^"]+"/, "version = \"#{expected_req}\"")
    updates << "#{name}: #{version} -> #{expected_req}"
  end
  if path.nil? || path.empty?
    structural_failures << "#{root_manifest}:#{line_number}: workspace dependency #{name} must include a local path"
  end

  line
end.join

missing_root = referenced_internal.keys.sort.reject { |name| root_internal_seen[name] }
missing_root.each do |name|
  structural_failures << "#{root_manifest}: root [workspace.dependencies] is missing internal dependency #{name}"
end

if structural_failures.any?
  warn "workspace internal dependency structure is not repairable automatically:"
  structural_failures.each { |failure| warn "  - #{failure}" }
  exit 1
end

if mode == "check"
  if stale_failures.any?
    warn "workspace internal dependency versions are stale:"
    stale_failures.each { |failure| warn "  - #{failure}" }
    warn "repair with: sh scripts/sync-workspace-internal-deps.sh"
    exit 1
  end

  puts "workspace internal dependency pins synchronized (#{internal_versions.size} internal crates, #{checked_edges} dependency edges)"
  exit 0
end

if updates.any?
  File.write(root_manifest, rewritten)
  updates.each { |update| puts "updated #{update}" }
  puts "workspace internal dependency pins synchronized"
else
  puts "workspace internal dependency pins already synchronized (#{internal_versions.size} internal crates, #{checked_edges} dependency edges)"
end
RUBY
