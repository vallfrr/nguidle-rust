# ADR 0002 — Common project commands

Status — Accepted.

## Context

The repositories use Cargo, Python, PowerShell and shell scripts. Portable,
stable task names make the entry points easier to find and can be reused in CI.

## Options

- A small `justfile` per checkout.
- A Rust `xtask` binary.
- Document direct scripts for each platform.

## Decision

Use `just check` in all three repositories for checks that need no game
files. Let the existing scripts own setup, builds and gameplay tests.
Keep the shared assistant guidelines in the engine checkout and reference
them from each platform, which owns its platform-specific guides.

## Consequences

Contributors install Just once. Automated jobs can use the same commands
as a contributor. The recipes remain thin wrappers over existing tools.

## Revisit when

Consider a dedicated command-line tool if the recipes need substantial
workflow logic or cannot run on the supported development hosts.
