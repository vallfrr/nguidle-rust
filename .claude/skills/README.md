# Skills for assistants (Claude Code, Codex, others)

Loaded on demand by Claude Code (`.claude/skills/`). Other assistants: read the `SKILL.md` of the relevant skill.

| skill | source | license | role |
|---|---|---|---|
| `ngu-verify` | this project | GPL-3.0 | prove that a change keeps the game identical to the original |
| `ngu-devmode` | this project | GPL-3.0 | drive the game by script (unlock, menus, screen as text) |
| `rust-patterns`, `rust-testing` | [everything-claude-code](https://github.com/affaan-m/everything-claude-code) | MIT (`LICENSE` in the folder) | idiomatic Rust, Rust tests |
| `ponytail`, `ponytail-review`, `ponytail-audit`, `ponytail-debt` | [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | MIT (`LICENSE` in the folder) | minimal code: simplest thing that works, over-engineering review/audit, ledger of `ponytail:` shortcuts |

The full ponytail plugin (with the hooks that activate it each session) is installed at user level: see its README.
