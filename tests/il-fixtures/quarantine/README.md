# Open generated-code fixtures

`StructProperty.cs` reproduces a generated-Rust compilation failure. With the
isolated project, the interpreter returned 7 for input 3 and `ngu-ilgen`
reported success, but `rustc` reported E0382 (the struct local was moved before reuse).
`StringSwitch.cs` independently reproduces a second failure: the interpreter
returns 1 for `alpha`, but the generated Rust refers to an omitted
`_PrivateImplementationDetails` hash method (E0425). The local compiler outputs
are `work/traces/lot6/isolated-struct-error.log` and `string-switch-error.log`,
outside the repository.

This fixture is not counted as passing, and the main suite does not exercise it.
Do not remove it from quarantine until the generated code compiles and its
result matches .NET and the interpreter.

Build the isolated fixture with `dotnet build tests/il-fixtures/quarantine/StructFixture.csproj --artifacts-path <temporary-directory>`.
Feed its DLL to `ngu-ilgen` selecting class `StructFixture` and entry
`StructFixture::StructProperty`, then compile the generated Rust with the
engine's `ngu-rt` and `ngu-host` crates. Run the .NET and interpreter versions
with identical inputs when validating a fix. Build outputs and traces must stay
outside Git.

For the other case, build `StringSwitch.csproj` into a temporary artifacts
directory and select class `SwitchFixture` and entry `SwitchFixture::Pick`.
