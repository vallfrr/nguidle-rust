# Open generated-code fixtures

`StructProperty.cs` reproduces a generated-Rust compilation failure. With the
isolated project, the interpreter returned 7 for input 3 and `ngu-ilgen`
reported success, but `rustc` reported E0382 (the struct local was moved before reuse).
An independently reproduced string-switch failure (E0425) was corrected by
including the compiler's `<PrivateImplementationDetails>` helper when selecting
classes. Five string-switch cases are now part of the passing main suite. The
local compiler outputs are `work/traces/lot6/isolated-struct-error.log` and
`string-switch-error.log`, outside the repository.

`DelegateGeneric.cs` covers a `Func<int, int>` lambda. The interpreter agrees
with .NET, but the generated path reports `generic delegate return` as
unimplemented. It remains open even if the simple structure example is fixed.

These fixtures are not counted as passing, and the main suite does not exercise
them. Do not remove one from quarantine until the generated code runs and its
result matches .NET and the interpreter.

Build the isolated fixture with `dotnet build tests/il-fixtures/quarantine/StructFixture.csproj --artifacts-path <temporary-directory>`.
Feed its DLL to `ngu-ilgen` selecting class `StructFixture` and entry
`StructFixture::StructProperty`, then compile the generated Rust with the
engine's `ngu-rt` and `ngu-host` crates. Run the .NET and interpreter versions
with identical inputs when validating a fix. Build outputs and traces must stay
outside Git.

For the delegate case, build `DelegateGeneric.csproj` into a temporary artifacts
directory and select class `DelegateFixture` and entry `DelegateFixture::Run`.
