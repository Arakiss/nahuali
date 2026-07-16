# Install and verify a Nahuali release

Nahuali publishes prebuilt beta archives for macOS and Linux on x86_64 and
Arm64. Each archive contains `nahuali`, `nahuali-mcp`, and `nahuali-api`.

## Installer behavior

The installer selects the newest `v*` prerelease that contains an
archive for the current platform. It always verifies the adjacent SHA-256 file
before installing. If `cosign` is available, it also verifies the Sigstore
bundle against the repository's release workflow identity.

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh | sh
export PATH="$HOME/.nahuali/bin:$PATH"
nahuali demo
```

The script never edits a shell profile. To require Sigstore verification rather
than treating an unavailable `cosign` command as a warning:

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh \
  | NAHUALI_REQUIRE_SIGSTORE=1 sh
```

Pin an exact release with `NAHUALI_VERSION`:

```bash
curl -fsSL https://raw.githubusercontent.com/Arakiss/nahuali/main/scripts/install.sh \
  | NAHUALI_VERSION=vX.Y.Z-beta.N sh
```

## Repository verifier

From a source checkout, the release verifier downloads the archive, checksum,
and Sigstore bundle for the current platform. It verifies SHA-256, the signing
identity, GitHub artifact provenance, the CycloneDX SBOM, and an install smoke
test. All of these checks are mandatory:

```bash
bash scripts/verify-release.sh \
  --tag vX.Y.Z-beta.N \
  --require-sbom \
  --require-provenance
```

The `--require-sbom` and `--require-provenance` flags remain accepted for
compatibility with existing automation; omitting them does not make either
check optional.

Required signing identity:

```text
https://github.com/Arakiss/nahuali/.github/workflows/release.yml@refs/tags/vX.Y.Z-beta.N
```

The release page should contain, for each supported target:

- `nahuali-vX.Y.Z-beta.N-<target>.tar.gz`
- the matching `.sha256` file
- the matching `.sigstore.json` bundle

It must also contain the CycloneDX SBOM named for the CLI tag. The supported
targets are:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-gnu`

## Build from source

Source builds use the locked workspace dependencies:

```bash
cargo build --workspace --locked
cargo test --workspace --locked
cargo install --path crates/nahuali-cli --locked
cargo install --path crates/nahuali-mcp --locked
cargo install --path crates/nahuali-api --locked
```

Default builds include attestation and the tamper-evident hash chain. Passing
`--no-default-features` deliberately selects the legacy unchained compatibility
build.
