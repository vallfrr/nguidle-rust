---
name: nguidle-port
description: Create a new platform port using the public engine interface and the port template.
---

Read `docs/VISION.md`, then follow `docs/PORTING.md` and `docs/ENGINE-API.md`.
Start with `just new-port` and use `ngu-headless` as the reference port.
Pin the engine version and keep host-specific code in the platform repository.
Run the generated port's `just check` and verify the target's input, output
and save paths. Follow the engine's cold trial when changing the template.
