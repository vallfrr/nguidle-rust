# ADR 0001 — Pin platform ports to an engine tag

Status — Accepted for local integration.

## Context

The engine, web and Vita projects live in independent Git repositories.
Both platform builds need an immutable engine revision. They also use
preparation tools that need to match the engine reading the generated data.

## Options

- Use a tagged Cargo dependency and a committed lockfile.
- Use a Git submodule at the exact commit referenced by the engine tag.
- Distribute a separately built binary and a matching tool bundle.

## Decision

The WebAssembly adapter lives in the web repository and uses a tagged Cargo
dependency on `clr`. The preparation tool stages source files from the tag
after matching the commit in `Cargo.lock`.

The Vita host uses a Git submodule at the same engine tag's exact commit.
It cross-builds the engine's static library with its locked workspace and
its specialized compilation profiles, including the optional translated
build. A fresh checkout verifies the tag against the gitlink.

Both ports use Git URL rewriting in a local test environment to validate
an unpublished engine tag. Publish the engine tag first when publication
is approved. Change a downstream pin in its own review.

## Consequences

Each platform can be cloned and built independently. One consistent engine
revision supplies both platform code and preparation tools. Vita retains
the nested, Git-managed source dependency used by its build.

## Revisit when

Consider a standalone Cargo static-library adapter for Vita when the native
build can use the public engine interface and packaged converter.
