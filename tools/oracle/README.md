# Unity RNG reference probe

`UnityRngProbe.cs` is a BepInEx 5 plugin that records Unity's random sequence and exits. It activates only when `NGU_RNG_PROBE_OUTPUT` points to an output file. It does not read or alter game state.

For the recorded vectors, copy a verified Windows 1.260 installation to an isolated local directory, install the official BepInEx 5.4.21 x64 release there, and compile the probe as a library against BepInEx and the copied Unity managed assemblies. Put the library in `BepInEx/plugins`. Set the output variable to a new local file, then run the isolated executable with `-batchmode -nographics`. Back up the original save directory before starting the executable. Keep the copied installation, build outputs and logs outside Git.

The file `clr/tests/fixtures/unity2019_rng.csv` contains the numeric output captured from Unity 2019.4.22f1 using seeds 0, 1, -1 and 123456789. For each seed, the probe resets the state before collecting 16 values from each of `value`, integer `Range(-100, 100)` and float `Range(-10, 30)`. The Rust unit test checks these exact vectors. Additional ranges, state serialization, System.Random and game-specific behavior require separate comparisons.

Run `just verify-game` with `NGU_GUARDS=on` and `NGU_GUARDS=off` to compare the interpreter paths with the generated game. The default is `off`. These comparisons use the shared compatibility layer.
