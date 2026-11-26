set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]

python := if os() == "windows" { "python" } else { "python3" }
doc-env := if os() == "windows" { "$env:RUSTDOCFLAGS='-D warnings';" } else { "RUSTDOCFLAGS='-D warnings'" }

check:
    {{python}} -B .github/repo-check.py
    {{python}} -B tools/check_codemap.py
    {{python}} -B tools/check_skills.py
    rustfmt --check --edition 2021 clr/tests/save_fixtures.rs crates/save-format/tests/wire.rs crates/platform-api/src/lib.rs crates/platform-api/src/testing.rs crates/cil-interp/src/lib.rs clr/src/session.rs crates/ngu-headless/src/lib.rs crates/ngu-headless/src/main.rs crates/ngu-headless/tests/synthetic.rs crates/ngu-headless/tests/session.rs templates/port/src/lib.rs templates/port/src/main.rs templates/port/src/display.rs templates/port/src/input.rs templates/port/src/audio.rs templates/port/tests/contract.rs
    {{python}} -B tools/clippy-check.py
    cargo test --locked --workspace --features clr/legacy-save
    cargo check --locked --manifest-path tools/oracle/scenario-runner/Cargo.toml
    {{python}} -B -m ruff check --select E9,F63,F7,F82 .github/repo-check.py tools
    {{python}} -B -m unittest discover -s tools -p test_verify_game_data.py
    {{python}} -B -m unittest discover -s tools/oracle -p test_compare_scenario.py
    {{python}} -B -m unittest discover -s tools -p test_new_port.py
    {{python}} -B -m unittest discover -s tools -p test_pc_budget.py
    {{python}} -B tools/check_template.py
    just doc

# Reference documentation of the public crates; any warning is an error.
doc:
    {{doc-env}} cargo doc --locked --no-deps -p platform-api -p ngu-headless -p clr

# Create a port repository next to this checkout: just new-port nguidle-switch "Nintendo Switch"
# The optional third argument builds it against a local engine checkout: just new-port nguidle-test "Test" ../nguidle-rust
new-port name platform="a new platform" engine="":
    {{python}} -B tools/new_port.py {{name}} --platform "{{platform}}" {{ if engine == "" { "" } else { "--engine-path " + engine } }}

# The mechanical half of the porting-guide cold trial (clones this repository, follows the guide, runs the new port's checks).
cold-trial:
    {{python}} -B tools/cold_trial.py

verify-game mode="verify":
    bash tools/regress.sh {{mode}}

verify-fixtures:
    {{python}} -B tests/il-fixtures/run.py

check-components:
    cargo test --locked -p cil-interp
    cargo test --locked -p platform-api
    cargo test --locked -p save-format

verify-data directory:
    {{python}} -B tools/verify_game_data.py "{{directory}}"

prepare-data directory output: (verify-data directory)
    {{python}} -B tools/nguprep.py --game-data "{{directory}}" --out "{{output}}"

pack-data directory assembly output:
    {{python}} -B tools/pack.py "{{directory}}" "{{assembly}}" "{{output}}"

# Manual: the PC window against the budgets in budgets.json. Needs a display and prepared game data; build ngu-window first
# (cargo build --release -p ngu-desktop --bin ngu-window).
pc-perf window dll data:
    {{python}} -B tools/pc_budget.py "{{window}}" "{{dll}}" "{{data}}"
