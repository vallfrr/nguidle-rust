set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]

python := if os() == "windows" { "python" } else { "python3" }

check:
    {{python}} -B .github/repo-check.py
    rustfmt --check --edition 2021 clr/tests/save_fixtures.rs
    {{python}} -B tools/clippy-check.py
    cargo test --locked --workspace --exclude ngu-gen --features clr/legacy-vita-save
    {{python}} -B -m ruff check --select E9,F63,F7,F82 .github/repo-check.py tools
    {{python}} -B -m unittest discover -s tools -p test_verify_game_data.py

verify-game:
    bash tools/regress.sh

verify-fixtures:
    {{python}} -B tests/il-fixtures/run.py
