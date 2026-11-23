# ADR 0003 — Separate repository checks from game validation

Status — Accepted as the local workflow definition.

## Context

Each checkout needs repeatable checks. The real game, its extracted data
and its saves belong on the maintainer's machine. A hardware timing test
needs access to the Vita.

## Options

- Run public, synthetic checks on GitHub and keep real-game checks local.
- Run everything on the maintainer's machine.
- Expose the maintainer's real-game setup through a self-hosted runner.

## Decision

The independent repositories define read-only GitHub Actions workflows
for pull requests and pushes to `main`. The workflows run the same
`just check` recipe as contributors. Engine jobs run on Windows and Linux,
web builds the WASI runtime and tests saving with synthetic state, and Vita
builds the C host against a pinned SDK image with no device setup.

Real-game replays stay behind local commands using isolated profiles.
Review workflows use no Steam inputs, device credentials or derived
artifacts. Hardware and fidelity work retain their own validation.

## Consequences

Review jobs can reproduce basic build and parser behavior without private
data. Existing warnings and documentation debt remain visible and can be
retired incrementally. Publication and branch protection are separate from
the implementation and local validation of the workflows.

## Revisit when

Revisit if the checks cease to be reproducible on clean runners, take too
long for reviews or require dedicated hardware.
