"""Cache the Steam Community achievement icons for a prepared desktop profile."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
import json
from pathlib import Path
import re
from tempfile import NamedTemporaryFile
from urllib.request import Request, urlopen

from PIL import Image


APP_ID = 1147690
PAGE = f"https://steamcommunity.com/stats/{APP_ID}/achievements/?l=english"
CDN = f"https://shared.fastly.steamstatic.com/community_assets/images/apps/{APP_ID}"
KEY = re.compile(r"[a-z0-9_]{1,64}")
FILENAME = re.compile(r"[a-f0-9]{40}\.jpg")


def fetch(url, limit):
    with urlopen(Request(url, headers={"User-Agent": "Mozilla/5.0"}), timeout=15) as response:
        contents = response.read(limit + 1)
    if len(contents) > limit:
        raise ValueError("Unexpected response size")
    return contents


def icon_catalog(page):
    script = re.search(r'<script[^>]+id="valve-ssr-data"[^>]*>(.*?)</script>', page, re.S)
    if not script:
        raise ValueError("Steam did not return an achievement catalog")
    context = json.loads(script[1])["renderContext"]
    for query in json.loads(context["queryData"])["queries"]:
        if query["queryKey"] == ["achievements", APP_ID, "schema_v2"]:
            groups = query["state"]["data"]["groups"]
            icons = {
                a["api_name"]: a["icon_achieved"]
                for group in groups
                for a in group["achievements"]
                if not a.get("archived")
            }
            if icons and all(KEY.fullmatch(key) and FILENAME.fullmatch(name) for key, name in icons.items()):
                return icons
    raise ValueError("Steam returned an unrecognized achievement catalog")


def cache_icon(folder, entry):
    key, filename = entry
    destination = folder / f"{key}.png"
    if destination.is_file():
        return
    raw = fetch(f"{CDN}/{filename}", 256 * 1024)
    with Image.open(BytesIO(raw)) as source:
        if source.format != "JPEG" or source.width != source.height or source.width > 256:
            raise ValueError(f"Unexpected icon format for {key}")
        icon = source.convert("RGBA").resize((64, 64), Image.Resampling.LANCZOS)
    with NamedTemporaryFile(dir=folder, delete=False) as output:
        path = Path(output.name)
    try:
        icon.save(path, format="PNG")
        path.replace(destination)
    finally:
        path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("data", type=Path, help="prepared game data directory")
    args = parser.parse_args()
    folder = args.data / "tex" / "steam-achievements"
    folder.mkdir(parents=True, exist_ok=True)
    index = folder / "index.json"
    if index.is_file():
        try:
            keys = json.loads(index.read_text(encoding="utf-8"))
        except ValueError:
            keys = []
        if isinstance(keys, list) and keys and all(
            isinstance(key, str) and KEY.fullmatch(key) and (folder / f"{key}.png").is_file()
            for key in keys
        ):
            print(f"{len(keys)} achievement icons already cached.")
            return
    icons = icon_catalog(fetch(PAGE, 2 * 1024 * 1024).decode("utf-8"))
    with ThreadPoolExecutor(max_workers=8) as pool:
        for _ in pool.map(lambda entry: cache_icon(folder, entry), icons.items()):
            pass
    index.write_text(json.dumps(sorted(icons)), encoding="utf-8")
    print(f"Cached {len(icons)} Steam achievement icons.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, TypeError, KeyError) as error:
        raise SystemExit(f"Achievement icons could not be prepared: {error}") from error
