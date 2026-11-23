python := if os() == "windows" { "python" } else { "python3" }

check:
    cargo check --locked -p clr --lib
    {{python}} -B -m unittest discover -s tools -p test_verify_game_data.py
