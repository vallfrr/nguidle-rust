# Agent instructions

This is an NGU Idle port for __PORT_PLATFORM__ on the nguidle-rust engine. Read `README.md`, then the engine's
`docs/PORTING.md` (the complete guide) and `docs/ENGINE-API.md` (what is public).

- Run `just check` before and after each change. Work on a topic branch.
- `PORT:` comments in `src/` mark the only places that need platform code. The order of the steps in
  `run_frame` is part of the contract: input, game time, draw, then textures, text and sound.
- Call the engine from one thread only; copy any borrowed texture or draw data before using it elsewhere.
- Never commit the game's files, extracted assets, saves or machine-specific paths. Test with a throw-away profile folder.
- Update the engine pin together: edit `Cargo.toml` and `engine.tag` (both hold the tag; `Cargo.lock` records the exact revision), then `cargo update -p clr`.
- Keep code, comments and documentation in English. Report what was tested and what was not.
