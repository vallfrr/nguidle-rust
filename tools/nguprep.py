"""nguprep: exports NGU Idle's Unity data (read-only on the game).

Output (default out/ngu/ in this repository):
  scene.json     objects and Unity resource paths with PPtrs normalised to "file:pathid"
  tex/<id>.png   textures (Texture2D) as RGBA
  fonts/<id>.ttf fonts
  text/<id>.bin  TextAssets
  audio/<name>.wav AudioClips
Usage: python nguprep.py [--game-data PATH] [--out PATH] [--no-tex]
"""
import argparse, json, os, struct, collections
from pathlib import Path
import UnityPy
from UnityPy.helpers.TypeTreeGenerator import TypeTreeGenerator

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--game-data", type=Path,
                    default=Path(__file__).resolve().parents[2] / "NGU IDLE" / "NGUIdle_Data")
parser.add_argument("--out", type=Path, default=Path(__file__).resolve().parents[1] / "out" / "ngu")
parser.add_argument("--no-tex", action="store_true")
args = parser.parse_args()
data_dir = args.game_data.resolve()
if not (data_dir / "Managed" / "Assembly-CSharp.dll").is_file():
    parser.error("--game-data must point to the NGUIdle_Data directory")
OUT = args.out.resolve()
NO_TEX = args.no_tex
for folder in ("tex", "fonts", "text", "audio"):
    (OUT / folder).mkdir(parents=True, exist_ok=True)

env = UnityPy.load(str(data_dir))
g = TypeTreeGenerator("2019.4.22f1")
g.load_local_dll_folder(str(data_dir / "Managed"))
env.typetree_generator = g

# --- project tags: custom tag ids are 20000 + index into TagManager.tags (the game compares tag names, e.g. "Menu")
for _o in env.objects:
    if _o.type.name == "TagManager":
        (OUT / "tags.json").write_text(json.dumps({"tags": list(_o.read_typetree()["tags"])}), encoding="utf-8")
        break

# --- fix: the generator types string[] fields as "string" (should be "vector")
_orig_get_nodes = g.get_nodes
def _patched_get_nodes(assembly, fullname):
    nodes = _orig_get_nodes(assembly, fullname)
    for i, n in enumerate(nodes):
        if n.m_Type == "string" and i + 3 < len(nodes) and nodes[i + 1].m_Type == "Array"                 and nodes[i + 3].m_Name == "data" and nodes[i + 3].m_Type != "char":
            n.m_Type = "vector"
    return nodes
g.get_nodes = _patched_get_nodes


def base(n):
    return os.path.basename(n.replace("\\", "/"))


def fname(af, fid):
    if fid == 0:
        return base(af.name)
    ex = af.externals
    return base(ex[fid - 1].path) if 0 < fid <= len(ex) else f"?{fid}"


def norm(af, v):
    """Replaces PPtrs {m_FileID,m_PathID} with the string 'file:pathid'."""
    if isinstance(v, dict):
        if set(v.keys()) == {"m_FileID", "m_PathID"}:
            if v["m_PathID"] == 0:
                return None
            return f"{fname(af, v['m_FileID'])}:{v['m_PathID']}"
        return {k: norm(af, x) for k, x in v.items()}
    if isinstance(v, list):
        return [norm(af, x) for x in v]
    if isinstance(v, (bytes, bytearray)):
        return {"$bytes": len(v)}
    return v


# Preserve duplicate paths: Unity can index a texture and a sprite under the same name.
resources = None
for o in env.objects:
    if o.type.name == "ResourceManager":
        resources = [(path, norm(o.assets_file, ref)) for path, ref in o.read_typetree()["m_Container"] if ref["m_PathID"]]
        break


# MonoScript: (file, pathid) -> class
scripts = {}
for o in env.objects:
    if o.type.name == "MonoScript":
        d = o.read()
        scripts[f"{base(o.assets_file.name)}:{o.path_id}"] = {
            "ns": d.m_Namespace, "cls": d.m_ClassName, "asm": d.m_AssemblyName}

WANT = {"GameObject", "Transform", "RectTransform", "Canvas", "CanvasRenderer", "MonoBehaviour",
        "Sprite", "Texture2D", "Font", "TextAsset", "AudioClip", "Camera", "Material"}
objs = {}
errors = collections.Counter()
for o in env.objects:
    t = o.type.name
    if t not in WANT:
        continue
    af = o.assets_file
    key = f"{base(af.name)}:{o.path_id}"
    try:
        if t == "MonoBehaviour":
            raw = o.get_raw_data()
            sf, sp = struct.unpack_from("<iq", raw, 16)   # PPtr<MonoScript> apres m_Enabled+pad
            sk = f"{fname(af, sf)}:{sp}"
            try:
                tt = o.read_typetree()
            except Exception as e:
                errors["typetree:" + repr(e)[:50]] += 1
                tt = {}
            tt.pop("m_Script", None)
            rec = {"t": t, "script": scripts.get(sk), "d": norm(af, tt)}
        elif t == "Texture2D":
            d = o.read()
            rec = {"t": t, "name": d.m_Name, "w": d.m_Width, "h": d.m_Height, "fmt": int(d.m_TextureFormat)}
            if not NO_TEX:
                p = OUT / "tex" / (key.replace(":", "_") + ".png")
                if not p.exists() and d.m_Width > 0 and d.m_Height > 0:
                    try:
                        d.image.save(p)
                    except Exception as e:
                        errors["tex:" + repr(e)[:50]] += 1
        elif t == "Font":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
            data = bytes(d.m_FontData) if d.m_FontData else b""
            if data:
                (OUT / "fonts" / (key.replace(":", "_") + ".ttf")).write_bytes(data)
                rec["file"] = key.replace(":", "_") + ".ttf"
            rec["d"] = norm(af, o.read_typetree()) if False else {}
        elif t == "TextAsset":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
            raw = d.m_Script
            raw = raw.encode("utf-8", "surrogateescape") if isinstance(raw, str) else bytes(raw)
            (OUT / "text" / (key.replace(":", "_") + ".bin")).write_bytes(raw)
        elif t == "AudioClip":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
            try:
                samples = d.samples
                filename = d.m_Name + ".wav"
                sample = samples.get(filename) or next(iter(samples.values()))
                (OUT / "audio" / filename).write_bytes(sample)
            except Exception as e:
                errors["audio:" + repr(e)[:50]] += 1
        else:
            rec = {"t": t, "d": norm(af, o.read_typetree())}
        objs[key] = rec
    except Exception as e:
        errors[t + ":" + repr(e)[:60]] += 1


def park_card_preview(objects):
    # The card drag preview starts partly over the sidebar. DeckPodUI positions
    # it at the pointer during a drag and parks it offscreen when the drag ends.
    for record in objects.values():
        if record["t"] != "RectTransform":
            continue
        rect = record["d"]
        go = objects.get(rect.get("m_GameObject"), {}).get("d", {})
        if go.get("m_Name") != "Ghost":
            continue
        parent = objects.get(rect.get("m_Father"), {}).get("d", {})
        parent_go = objects.get(parent.get("m_GameObject"), {}).get("d", {})
        if parent_go.get("m_Name") == "Cards Menu":
            shift = -6000 - rect["m_AnchoredPosition"]["y"]
            rect["m_AnchoredPosition"]["y"] += shift
            rect["m_LocalPosition"]["y"] += shift


park_card_preview(objs)


def fit_card_labels(objects):
    labels = {"Total Mana Text", "Mayo Gen Text"}
    for record in objects.values():
        if (record.get("script") or {}).get("cls") != "Text":
            continue
        text = record["d"]
        go = objects.get(text.get("m_GameObject"), {}).get("d", {})
        font = text.get("m_FontData")
        if go.get("m_Name") in labels and font:
            font["m_BestFit"] = True
            font["m_MinSize"] = min(10, font["m_FontSize"])
            font["m_MaxSize"] = font["m_FontSize"]


fit_card_labels(objs)


def hide_unavailable_online_ui(objects):
    for record in objects.values():
        if record["t"] != "RectTransform":
            continue
        rect = record["d"]
        go = objects.get(rect.get("m_GameObject"), {}).get("d", {})
        parent = objects.get(rect.get("m_Father"), {}).get("d", {})
        parent_name = objects.get(parent.get("m_GameObject"), {}).get("d", {}).get("m_Name")
        name = go.get("m_Name", "")
        hidden = (
            parent_name == "Canvas" and name in {"Sellout Kreds Canvas", "Sellout Kreds 2 Canvas", "Itopod Name List Canvas"}
            or parent_name == "Title Panel" and (
                name.startswith("Buy AP Wth Kred") or name in {"Special Packs", "Special News", "Special News 2"}
            )
            or parent_name == "Info Menu + Controller" and name in {"Discord", "Kart Promo Button", "SteamMtx", "Itopod Names List"}
            or parent_name == "Settings Panel 2" and name == "Check For Updates"
            or parent_name == "Augment Settings 1" and name == "Submit Highscores"
            or parent_name in {"Main Menu Screen", "Splash Screen"} and (
                name.startswith("Discord Button") or name in {"Kartridge Button", "Kong Button", "Cloud Save Border"}
            )
            or parent_name == "Main Menu Screen" and name == "Text"
        )
        if hidden:
            go["m_PortHidden"] = True


hide_unavailable_online_ui(objs)
scene = OUT / "scene.json"
temp = scene.with_suffix(".json.tmp")
with temp.open("w", encoding="utf-8") as f:
    json.dump({"objects": objs, "scripts": scripts, "resources": resources}, f, separators=(",", ":"))
temp.replace(scene)
print("exported objects:", len(objs))
print("resource references:", len(resources or []))
print("types:", collections.Counter(v["t"] for v in objs.values()).most_common())
print("errors:", errors.most_common(10))
