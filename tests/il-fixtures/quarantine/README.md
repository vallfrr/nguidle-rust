# Open generated-code fixtures

`StructProperty.cs` reproduces a generated-Rust compilation failure. With the
isolated project, the interpreter returned 7 for input 3 and `ngu-ilgen`
reported success, but `rustc` reported E0382 (the struct local was moved before reuse).
Adding enough string-switch cases to the fixture's dispatch also caused the
generator to reference an omitted `_PrivateImplementationDetails` hash method
(E0425). The string-switch issue still needs its own minimized reproduction. The
full original compiler output from the local investigation is recorded in
`work/traces/lot6/generated-struct-suite.log` outside the repository.

This fixture is not counted as passing, and the main suite does not exercise it.
Do not remove it from quarantine until the generated code compiles and its
result matches .NET and the interpreter.

Build the isolated fixture with `dotnet build tests/il-fixtures/quarantine/StructFixture.csproj --artifacts-path <temporary-directory>`.
Feed its DLL to `ngu-ilgen` selecting class `StructFixture` and entry
`StructFixture::StructProperty`, then compile the generated Rust with the
engine's `ngu-rt` and `ngu-host` crates. Run the .NET and interpreter versions
with identical inputs when validating a fix. Build outputs and traces must stay
outside Git.
