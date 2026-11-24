# ADR 0004 — Skill sources

Status — Accepted for current project guidance.

## Context

Project guidance and third-party assistant skills need a discoverable location and traceable changes. Existing third-party copies include their licenses but no reliable upstream commit record.

## Decision

The engine repository's `.claude/skills` directory is the canonical source for common project guides. Platform repositories keep their platform-specific guides. Assistants that do not automatically discover that directory can read the same files directly.

Bundled third-party files remain available during the transition. `skills.lock` records their source, license and normalized SHA-256 content hashes. An unknown upstream revision is recorded as null. `just check` detects changes to the locked tracked files.

## Consequences

Content changes are reviewable without inventing provenance. Upstream revisions and reproducible installation remain open work. Project guides do not authorize pushes or replace source-level verification. User-installed hooks are optional.

## Revisit when

Replace bundled third-party copies with a reproducible installation once their upstream revisions and consumer discovery mechanisms have been established.
