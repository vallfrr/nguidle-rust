# Browser

The browser executes the engine through WebAssembly and renders with WebGL. The separate [web repository](https://github.com/vallfrr/nguidle-web) owns its adapter, save service, Docker setup, tests and engine pin. Its README documents deployment, controls, save synchronization and the supported game-data version.

A prepared game and isolated profile are required for the browser acceptance tests. Do not publish game data or saves.
