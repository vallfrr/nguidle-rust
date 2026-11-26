"""Run Clippy without promoting the inherited warnings in the engine."""

import json
import subprocess
import sys


STRICT = "clr/tests/save_fixtures.rs"


def main():
    command = [
        "cargo", "clippy", "--locked", "--workspace",
        "--all-targets", "--features", "clr/legacy-save", "--no-deps",
        "--message-format=json", "--", "--cap-lints", "warn",
    ]
    new_warning = False
    with subprocess.Popen(command, stdout=subprocess.PIPE, text=True, encoding="utf-8") as run:
        for line in run.stdout:
            event = json.loads(line)
            if event.get("reason") != "compiler-message":
                continue
            diagnostic = event["message"]
            if diagnostic.get("rendered"):
                sys.stderr.write(diagnostic["rendered"])
            if diagnostic["level"] == "warning" and any(
                span["is_primary"]
                and ("/" + span["file_name"].replace("\\", "/")).endswith("/" + STRICT)
                for span in diagnostic["spans"]
            ):
                new_warning = True
        status = run.wait()
    if new_warning:
        print(f"Clippy warnings in {STRICT} are treated as errors.", file=sys.stderr)
    return status or int(new_warning)


if __name__ == "__main__":
    sys.exit(main())
