# Nahuali Python

Python bindings are deferred from the first public release.

This directory is a placeholder only. It is not an installable package, it is
not published to PyPI, and it does not define a stable API yet.

It should remain README-only until the Rust public API is frozen. Do not add
`pyproject.toml`, setup files, generated clients, or registry metadata here
before publication is explicitly approved.

The intended future direction is a thin local package over the canonical Rust
`MemoryEngine`, not a hosted HTTP wrapper. Implementation should wait until the
`nahuali-core` contract has passed a release-candidate freeze.
Binding strategy and release criteria remain private until publication is
explicitly approved.
