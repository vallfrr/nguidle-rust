# ADR 0005 — Reference tools and porting method

## Decision

Use the record/replay and incremental fallback ideas described by rust-anything, but do not add it as a build dependency. Its documented adapters target other languages and do not supply a Unity/.NET game recorder. Our Unity probes and IL fixtures remain specific, separately tested tools. Measure fallback as part of the later native-port work; the present experiments do not establish a fallback rate.

Keep REA as an optional local investigation reference, not a required dependency or an automated source of truth. Its tool adapters may help inspect .NET, but no evaluation here establishes that installing it improves the existing evidence. Adopt a pinned version only after a reproducible case demonstrates that benefit. Neither external tool is needed for `just check`.

## Evidence and consequences

Reviewed the local snapshots of [rust-anything](https://github.com/erkamyaman/rust-anything) and [REA](https://github.com/morluto/rea), especially their READMEs. These are methodology references, not code copied into this project. Our BepInEx probe records only the documented fields and scenarios; passing it does not establish whole-game parity. The future front-door and fallback work remains on the roadmap.
