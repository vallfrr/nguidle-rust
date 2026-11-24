"""Create a new port repository from templates/port: `just new-port nguidle-<platform>`.

The new folder holds a Rust crate that builds on this engine, with CI, hygiene check, agent instructions and a contract
test. It is not a Git repository yet: review it, then `git init` and commit.

    python tools/new_port.py nguidle-switch --platform "Nintendo Switch"
    python tools/new_port.py nguidle-test --out ../nguidle-test --engine-path ../nguidle-rust   # local engine checkout
"""

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ENGINE = Path(__file__).resolve().parent.parent
TEMPLATE = ENGINE / "templates" / "port"
SHARED = [".github/repo-check.py", ".github/check-baseline", "LICENSE"]
NAME = re.compile(r"^[a-z][a-z0-9]*(-[a-z0-9]+)+$")


def git(*args):
    return subprocess.run(["git", "-C", str(ENGINE), *args], capture_output=True, text=True)


def engine_pin():
    """The newest `v0.x.y` tag of this checkout and the revision it names."""
    tags = [t for t in git("tag", "--list", "v0.*", "--sort=-v:refname").stdout.split() if re.fullmatch(r"v0\.\d+\.\d+", t)]
    if not tags:
        raise SystemExit("This engine checkout has no v0.x.y tag: tag a revision first, or pass --tag and --revision.")
    return tags[0], git("rev-list", "-n1", tags[0]).stdout.strip()


def render(text, values):
    for key, value in values.items():
        text = text.replace(key, value)
    return text


def create(name, out, platform, engine_path, tag, revision, url):
    """Write the port into `out`. `revision` is only reported: Cargo.lock records it at the first build."""
    if not NAME.match(name):
        raise SystemExit(f"`{name}` is not a valid port name: use lowercase words joined by hyphens, like nguidle-switch.")
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"{out} exists and is not empty: choose another --out or remove it.")
    if engine_path:
        dependency = f'{{ path = "{Path(engine_path).as_posix()}/clr" }}'
    else:
        dependency = f'{{ git = "{url}", tag = "{tag}" }}'
    values = {
        "__PORT_NAME__": name,
        "__PORT_LIB__": name.replace("-", "_"),
        "__PORT_PLATFORM__": platform,
        "__ENGINE_DEPENDENCY__": dependency,
        "__ENGINE_TAG__": tag,
    }
    for source in sorted(p for p in TEMPLATE.rglob("*") if p.is_file()):
        target = out / source.relative_to(TEMPLATE)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(render(source.read_text(encoding="utf-8"), values), encoding="utf-8", newline="\n")
    for name_ in SHARED:
        target = out / name_
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ENGINE / name_, target)
    return out


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("name", help="repository and crate name, for example nguidle-switch")
    parser.add_argument("--platform", default="a new platform", help="human name of the platform, used in the text")
    parser.add_argument("--out", type=Path, help="where to create it (default: next to the engine checkout)")
    parser.add_argument("--engine-path", help="use a local engine checkout (path to its root) instead of the tagged git dependency")
    parser.add_argument("--tag", help="engine tag to pin (default: newest v0.x.y tag)")
    parser.add_argument("--revision", help="engine revision of that tag (default: read from git)")
    options = parser.parse_args(argv)
    tag, revision = options.tag, options.revision
    if not tag:
        tag, revision = engine_pin()
    elif not revision:
        revision = git("rev-list", "-n1", tag).stdout.strip()
        if not revision:
            raise SystemExit(f"Tag {tag} does not exist in this checkout: pass --revision, or tag it first.")
    remote = git("remote", "get-url", "origin").stdout.strip() or "https://github.com/vallfrr/nguidle-rust.git"
    out = options.out or ENGINE.parent / options.name
    create(options.name, out, options.platform, options.engine_path, tag, revision, remote)
    print(f"Created {out} (engine {tag}, revision {revision[:12] or 'unknown'}).")
    print("Next: cd there, `just check`, then read docs/PORTING.md of the engine and replace the PORT: stand-ins.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
