"""Shared workspace locations for the setup and doctor commands."""
from pathlib import Path
import os


REPO = Path(__file__).resolve().parents[1]
DEFAULT_HOME = (
    REPO.parent.parent
    if REPO.parent.name == "repos"
    else REPO.parent if (REPO.parent / "NGU IDLE" / "NGUIdle_Data").is_dir() else REPO
)


class Workspace:
    def __init__(self, home=None, game=None):
        self.home = Path(home or os.getenv("NGUIDLE_HOME") or DEFAULT_HOME).resolve()
        self.work = Path(os.getenv("NGU_WORK") or self.home / "work").resolve()
        self.build = Path(os.getenv("NGU_BUILD") or self.home / "build").resolve()
        installation = self.home / "game" / "NGU IDLE" / "NGUIdle_Data"
        legacy = self.home / "NGU IDLE" / "NGUIdle_Data"
        self.game = Path(
            game or os.getenv("NGU_GAME_DIR") or (legacy if not installation.is_dir() and legacy.is_dir() else installation)
        ).resolve()
        windows = os.name == "nt"
        self.prep = self.home / "venvs" / ("prep-win" if windows else "prep-linux")
        self.python = self.prep / ("Scripts/python.exe" if windows else "bin/python")
        self.vita = Path(os.getenv("NGU_VITA_REPO") or self.home / "repos" / "nguidle-vita").resolve()
        if not self.vita.is_dir():
            self.vita = REPO / "nguidle-vita"
        local_sdk = self.home / "toolchains" / "vitasdk"
        self.sdk = Path(os.getenv("VITASDK") or (local_sdk if local_sdk.is_dir() else Path.home() / "vitasdk")).resolve()
