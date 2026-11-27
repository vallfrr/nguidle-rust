# Unity RNG reference probe

`UnityRngProbe.cs` is a BepInEx 5 plugin that records Unity's random sequence and exits. It activates only when `NGU_RNG_PROBE_OUTPUT` points to an output file. It does not read or alter game state.

For the recorded vectors, copy a verified Windows 1.260 installation to an isolated local directory, install the official BepInEx 5.4.21 x64 release there, and compile the probe as a library against BepInEx and the copied Unity managed assemblies. Put the library in `BepInEx/plugins`. Set the output variable to a new local file, then run the isolated executable with `-batchmode -nographics`. Back up the original save directory before starting the executable. Keep the copied installation, build outputs and logs outside Git.

The original executable may ask Steam to start the installed game even when launched from a copy. In the recorded run this started a background Steam game process. It was closed and the backed-up save files matched afterward. Check for and account for this behavior before running additional scenarios.

The file `clr/tests/fixtures/unity2019_rng.csv` contains numeric output captured from Unity 2019.4.22f1 using seeds 0, 1, -1, 123456789 and the two 32-bit signed extremes. For each seed, the probe resets the state before collecting 16 values from each of `value`, integer `Range(-100, 100)` and float `Range(-10, 30)`. Set `NGU_RNG_STATE_PROBE_OUTPUT` in addition to the usual output variable to record the four state words and draws after state restoration; these vectors are in `unity2019_state.csv`. The Mono fixture records seeded `System.Random` values, bounded values, wide ranges and doubles. The Rust unit tests check these exact vectors. Additional ranges and game-specific behavior require separate comparisons.

The parameterless `System.Random` constructor currently retains the port's fixed seed of 12345. Original time-based seeding has not been reproduced or compared. The seeded generator follows the Mono reference algorithm and the recorded vectors. See [Mono's reference implementation](https://github.com/mono/mono/blob/2019-12/mcs/class/referencesource/mscorlib/system/random.cs).

The progression probe uses a preloader patch on the isolated copy to skip Steam initialization and autosave updates. It starts a fresh game state, bypasses the title menu and logs energy fields after 60 seconds of real time. It does not validate Steam integration or an existing save. Compile the probes with `build-probes.ps1 -GameCopy <isolated-copy> -BepInEx <extracted-bepinex-directory>`, set `NGU_PROGRESS_PROBE_OUTPUT` to a new local path, and launch the copied game with `-batchmode -nographics`. Install the BepInEx runtime in the copy before running. The probe and patcher affect the copied executable's in-memory assembly for that run. Back up and compare the original save files before and after the run.

After preparing the game's local data and compiling the interpreter, run `bash tools/oracle/replay-progress.sh` on WSL or Linux to replay the port's fresh-state measurement. It creates a separate local profile. Set `NGU_PROGRESS_PROBE_SAVE` to the path of a copied save in both environments to compare an existing save. Set `NGU_PROGRESS_REALTIME=1` to pace the 1,800 frames over 60 seconds and report elapsed wall time. Set `NGU_CLR_TARGET` if the release interpreter was built in a non-default target directory. The probe bypasses the original title menu; this replay calls the corresponding method after 90 warm-up frames, so the initial energy-bar phase can differ.

Run `just verify-game` with `NGU_GUARDS=on` and `NGU_GUARDS=off` to compare the interpreter paths with the generated game. The default is `off`. These comparisons use the shared compatibility layer.

`ScenarioProbe.cs` is an opt-in first recorder for a restricted subset of the autoplay format. Set `NGU_SCENARIO_PROBE_INPUT` to a text file and `NGU_SCENARIO_PROBE_OUTPUT` to a new local trace path; optionally set `NGU_PROGRESS_PROBE_SAVE` to an isolated save copy. It supports `tick menu N`, `tick mark NAME`, `tick check NAME` and `tick end`; unsupported commands are recorded as errors rather than silently ignored. `check` writes a private serialized save alongside the trace. Ticks are scheduled at 30 per second of wall time, not captured engine ticks. It seeds Unity RNG but does not fix the original's wall clock, does not replay pointer/controller input and does not yet compare traces automatically. Keep its outputs outside Git. A four-event smoke scenario (mark, menu, check, end) was exercised in an isolated copy with a copied save.

For that subset, `scenario-runner` replays the same file with the interpreter:

```sh
cargo run --locked --manifest-path tools/oracle/scenario-runner/Cargo.toml -- \
  "$DLL" "$ISOLATED_ASSETS" "$COPIED_SAVE" "$SCENARIO" "$PORT_TRACE"
python tools/oracle/compare-scenario.py "$ORIGINAL_TRACE" "$PORT_TRACE"
```

Use an isolated prepared-assets view, because the runner writes a profile below it. The comparator reports differing fields and returns a nonzero status if any differ; it does not normalize timing or compare the generated saves. In the first four-event run, menus and resource fields matched, while the energy-bar phase differed at three events. Original events were wall-clock scheduled and port events were simulation-tick scheduled, so this is not a tick-synchronized equivalence test. With `CLR_PROF=1`, the runner additionally logs the number of energy updates since startup.

Run `just verify-fixtures` with the .NET 8 SDK and Cargo installed to compare small independently written C# programs with the interpreter. Set `NGU_DOTNET` to the SDK executable when it is not on PATH. The fixture assembly is built in a temporary directory. See [Fidelity](../../docs/FIDELITY.md) for the current coverage.

## Offline progress and saves, original against the port

`OfflineProbe.cs` opens a save the way the game does when the player opens it (`getSaveDataFromString`, then `OpenFileDialog.loadintoGame`, so offline progress and version updates apply), then writes the whole game state as a save (`getBase64Data`) at the requested times. Environment: `NGU_OFFLINE_PROBE_SAVE` (the save to open), `NGU_OFFLINE_PROBE_OUTPUT` (output prefix: `<prefix>.info.txt`, which also records the clock the game read, and `<prefix>.<seconds>.sav`), `NGU_OFFLINE_PROBE_WAITS` (comma-separated seconds after the load, default `0`). Build it with `build-probes.ps1` and run the isolated executable with `-batchmode -nographics`, as above.

To compare, open the same save in the port with the clock the original read, then flatten both results and compare them field by field:

```sh
clr <dll> play <prepared copy> out.png "click,Create New Game" "click,Slot 1" "click,Create New Game" "w,30" "click,PLAY" "w,10"     "clock,<clock_before>" "open,<save>" "s,port.0.sav" "clock,real" "w,1800" "s,port.60.sav"   # with NGU_DESKTOP=1
NRBF_FLAT=1 clr <dll> nrbf original.0.sav > original.0.flat       # and the same for the port's file
python tools/oracle/compare_saves.py original.0.flat port.0.flat --tolerance 1e-6 --ignore _version --ignore 'manaCosts._items'
```

Ignore the list version counters and list capacities (`_version`, spare `_items` slots): they are internals of `List<T>` that no rule of the game reads. Make the test saves with the port (`s,<file>` after `set,Character.lastTime,<seconds>`): the original opens saves the port wrote, which also checks the save format in that direction. `play` runs with `NGU_DESKTOP=1` for the desktop screens of the applications. A fixed clock (`NGU_FIXED_NOW`) must not be left on while time passes: the game's energy does not grow with it.
