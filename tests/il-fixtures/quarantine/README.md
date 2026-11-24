# Open generated-code fixtures

`StructProperty.cs` reproduces a generated-Rust compilation failure. With the
struct fixture included in the main synthetic project, `ngu-ilgen` reported
success, but `rustc` reported E0382 (the struct local was moved before reuse).
Adding enough string-switch cases to the fixture's dispatch also caused the
generator to reference an omitted `_PrivateImplementationDetails` hash method
(E0425). Both need independent minimized reproductions and correction. The
full original compiler output from the local investigation is recorded in
`work/traces/lot6/generated-struct-suite.log` outside the repository.

This fixture is not counted as passing, and the main suite does not exercise it.
Do not remove it from quarantine until the generated code compiles and its
result matches .NET and the interpreter.
