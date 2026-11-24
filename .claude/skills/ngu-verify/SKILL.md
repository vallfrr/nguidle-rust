---
name: ngu-verify
description: Verify engine, interface and platform changes using the project's documented checks.
---

# Verify a change

Use [the test guide](../../../docs/TESTING.md) to select checks appropriate to the change. Run `just check` for repository changes. For game behavior, prepare the game data and run `just verify-game` with isolated profiles.

The generated game and reference interpreter share runtime components. Their agreement covers only the exercised scenarios. A claim about the original game requires a separate measurement against the original. See [fidelity](../../../docs/FIDELITY.md).

For visual changes, follow [console mode](../../../docs/CONSOLE-MODE.md) and inspect the affected screens and configurations. Keep game assets, generated files, screenshots of the original, saves and private data outside Git. Build generated game code under the workspace's memory cap.

Report the commands, results, limitations and remaining work. Follow the maintainer's instructions for review and publication.
