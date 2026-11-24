"""Compare synthetic .NET fixtures with the interpreter and generated Rust."""

import os
import json
import re
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[2]
PROJECT = ROOT / "tests" / "il-fixtures" / "Fixtures.csproj"
DOTNET = os.environ.get("NGU_DOTNET", "dotnet")
CARGO = os.environ.get("NGU_CARGO", "cargo")


def main():
    subprocess.run([CARGO, "build", "--locked", "-p", "clr", "-p", "ngu-ilgen"], cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    executable = target / "debug" / ("clr.exe" if os.name == "nt" else "clr")
    with TemporaryDirectory(prefix="ngu-il-fixture-") as temporary:
        output = Path(temporary)
        subprocess.run(
            [DOTNET, "build", str(PROJECT), "--artifacts-path", str(output), "--nologo"],
            cwd=ROOT,
            check=True,
        )
        candidates = list((output / "bin").rglob("Fixtures.dll"))
        if len(candidates) != 1:
            raise RuntimeError(f"Expected one fixture assembly, found {len(candidates)}")
        generator = target / "debug" / ("ngu-ilgen.exe" if os.name == "nt" else "ngu-ilgen")
        generated = output / "generated"
        src = generated / "src"
        src.mkdir(parents=True)
        subprocess.run([str(generator), str(candidates[0]), str(src), "Fixture", "Fixture::Branch"], cwd=ROOT, check=True)
        source = (src / "gen.rs").read_text(encoding="utf-8")
        names = {}
        for method in ("Branch", "Loop", "Choice", "SeededRandom", "ByRef", "Exception", "StringSwitch", "DelegateLambda", "DelegateBool"):
            matches = re.findall(r"pub fn (m\d+_Fixture_" + method + r")\(", source)
            if len(matches) != 1:
                raise RuntimeError(f"Expected one generated {method}, found {matches}")
            names[method] = matches[0]
        (generated / "Cargo.toml").write_text(
            '[package]\nname="ngu-fixture-runner"\nversion="0.0.0"\nedition="2021"\n'
            '[workspace]\n[dependencies]\n'
            f'ngu-rt={{path={json.dumps(str(ROOT / "crates" / "ngu-rt"))}}}\n'
            f'ngu-host={{path={json.dumps(str(ROOT / "crates" / "ngu-host"))}}}\n', encoding="utf-8")
        arms = "\n".join(f'"{method}" => gen::{name}(value),' for method, name in names.items())
        (src / "main.rs").write_text(
            '#![allow(non_camel_case_types,non_snake_case,dead_code,unused_imports,unused_variables,unused_mut,unreachable_code,unused_parens,unused_assignments,static_mut_refs)]\n'
            'mod ext { pub use ngu_rt::ext::*; include!("ext_stubs.rs"); }\nmod gen;\n'
            'fn main() { let dll=std::env::var("NGU_DLL").unwrap(); let vm=Box::leak(Box::new(ngu_host::clr::Vm::new(&dll))); vm.native_mode=true; vm.native_inv=Some(gen::hook); ngu_host::set_vm(vm as *mut _); let a:Vec<String>=std::env::args().collect(); let value:i32=a[2].parse().unwrap(); '
            f'let result=match a[1].as_str() {{ {arms} _ => panic!("unknown fixture") }}; '
            'println!("=> {}",result); }\n', encoding="utf-8")
        subprocess.run([CARGO, "build", "--manifest-path", str(generated / "Cargo.toml"),
                        "--target-dir", str(target), "--offline"], cwd=ROOT, check=True)
        runner = target / "debug" / ("ngu-fixture-runner.exe" if os.name == "nt" else "ngu-fixture-runner")
        subprocess.run(
            [
                sys.executable,
                str(PROJECT.parent / "compare.py"),
                "--dotnet", DOTNET,
                "--clr", str(executable),
                "--dll", str(candidates[0]),
                "--generated", str(runner),
            ],
            cwd=ROOT,
            check=True,
        )


if __name__ == "__main__":
    main()
