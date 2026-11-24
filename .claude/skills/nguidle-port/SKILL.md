---
name: nguidle-port
description: Investigate a new platform host for the NGU Idle Rust engine.
---

# Port the engine

Read the repository's [porting guide](../../../docs/PORTING.md), [engine interface](../../../docs/ENGINE-API.md) and [testing guide](../../../docs/TESTING.md). Pin the engine revision used by both the platform host and the game-data preparation tools.

Document the platform's renderer, input, storage and timing contracts. Keep engine calls on the engine thread and copy borrowed buffers before passing them elsewhere. Build with synthetic inputs where possible and keep game files and user profiles outside the repository.

Run the relevant checks and record the platform, revision, commands and coverage. A reusable platform template and a complete stable platform API remain future work.
