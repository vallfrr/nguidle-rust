# Audit of the C# -> Rust generator (ngu-ilgen) and the bridge (ngu-host)

Goal: make sure the generated Rust behaves like the original C#. Method: operation-by-operation review of the generator, then **differential tests** against the interpreter (oracle, `clr`) running the same IL. This document lists what was verified, fixed, and what remains a known gap.

## 1. Evidence (reproducible)
| test | command | result (2026-11-20) |
|---|---|---|
| `NumberOutput::suffixFormat` | `tools/ildiff-number.sh` | 84 cases, 0 differences |
| visible state after N frames (6,729 GameObjects: text, fill, colour, position) | `tools/statediff.sh 30` / `300` | 0 differing lines |
| same after 96 grid clicks + 1,500 idle frames | `NGU_STEPS=$(tools/grid-steps.sh);w,1500 tools/statediff.sh 600` | 0 differing lines |
| sequence of engine->game calls and mutating UnityEngine calls | `tools/divtrace.sh 30` | identical (only difference: selection order of an explicit-interface method, drawn from a hash table) |
| complete save, load, re-save | `tools/savediff.sh`, `tools/loaddiff.sh` | identical |
| static methods with primitive signatures, boundary arguments | `ngu-gen fuzz <dll> N` | see section 5 |

The whole battery: `tools/regress.sh` (must end with `REGRESSION: ALL OK`). Fixed clock (`NGU_FIXED_NOW`) and `NGU_NO_GUARD=1` on the oracle side (the oracle deliberately skips `CookingController.updateDishUI` when the menu is hidden; native does not).

## 2. Defects found by the audit and fixed
1. **Unsigned 64-bit comparisons**: `cgt.un/clt.un/bge.un...` truncated to `u32` even for `i64` -> `u64` when an operand is 64-bit.
2. **Float -> integer conversions**: Rust saturates, x86-64 (Mono/.NET) returns `MIN` (out of range or NaN). Added `f2i32`/`f2i64` (x86 behaviour).
3. **Evaluation stacks between blocks**: a non-empty stack at a join with different types read the wrong carry variable silently. Now detected (`diverging stacks`): the method is refused instead of mis-compiled (6 menu sort methods).
4. **Non-`Copy` carry values** moved in a loop: read by `clone()`.
5. **Live pointers between blocks** (`ref/out` in a ternary): carried in a `*mut T` variable.
6. **Generic external function names**: all instances of `GetComponent<T>` shared one function; the name now contains the type.
7. **Native type ids**: stable for game types (< n_user), **unstable across processes** for external types (created on demand). All type tests on native classes go through the name (`cid_of`).
8. **Dictionary/list created without generic parameters** (`new List<T>{...}`): type inferred by rustc; calls go through the runtime's generic functions (otherwise a silent copy through the bridge gave an empty list).
9. **Boxed objects**: an `Obj` boxed by generated code was unreadable by the engine. Boxing now always produces an engine `Value` (`boxv`).
10. **Engine boxed primitives** (`Ext::Boxed`) read as 0: unboxed in `FromV` (all bonus dictionary enum keys equal to 0).
11. **Scene import** overwrote constructor-initialised state (dictionaries) with null shell values; only populated fields are imported.
12. **`unbox.any`** lost its type in lowering; the type is kept (`Op::Unbox(ty)`).
13. **External constructors**: classes = pre-allocated object passed first; structs = return value.
14. **Exceptions**: `catch`/`finally`/`fault` by stack unwinding (`catch_unwind`), Rust exceptions (`expect`, indices, division) translated to usual CLR exceptions; `panic = "unwind"` required.
15. **Deserialized objects** ran constructors: `blank()`/`import(all)` for save loading; `List` vs array distinguished (`GList`).

## 3. Known gaps (not fixed)
- **Collection copies across the bridge**: arrays/lists/dictionaries passed to an engine native are copied (no alias). No effect observed; watch any engine API that *modifies* a received collection.
- **Boxed user classes in a non-convertible generic `Obj`**: message `[host] boxed object...` (null value). Never seen after the `boxv` unification.
- **Inheritance / interfaces** between game classes: refused (`class conversion`), concerns only file-dialog code (platform excluded). User `callvirt`: 6 methods, same area.
- **User structs** (value types defined by the game) are treated as classes (copy semantics lost). No case in the verified methods; list to be established.
- **Untranslated methods** (26-32): Steam, Kartridge, Windows file dialogs, `order*Cost` sorts (diverging stacks) -> `unimplemented!`; relayed as CLR exceptions by the bridge (the engine logs them as the interpreter does).
- **Integer division by zero**: panics; .NET throws `DivideByZeroException` (translated to a CLR exception in the bridge).
- **Concurrency**: global static state (`static mut`), single-threaded, like the game.

## 4. Operation-by-operation review (summary)
Integer arithmetic (`wrapping_*`, shifts masked like .NET), floating point (`f32` stays `f32`), comparisons (NaN unordered), conversions (see 2.2), arrays (`Vec` + `RefCell`, out-of-range = panic -> `IndexOutOfRangeException`), references (`ceq` = `Rc::ptr_eq`), fields (`clr` slots), statics (initialised on demand with the `.cctor`), virtual calls (unused by the game except the excluded area), delegates (target + method id, like the engine), coroutines (iterator classes run by the engine via `invoke`).

## 5. Differential fuzz
`ngu-gen fuzz <dll> 40`: 480 calls over 12 static methods with primitive signatures (excluding clock/random/platform), boundary arguments: **0 differences** (results and exceptions). Low coverage by construction: the game logic lives in instance methods that depend on state; it is covered by the state comparisons (section 1).
