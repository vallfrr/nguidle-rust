"""nguprep: exports NGU Idle's Unity data (read-only on the game).

Output (default J:/NGUvita/nguvita/out/ngu/):
  scene.json     objects (GameObject/RectTransform/Canvas/MonoBehaviour/Sprite/Font...) with PPtrs normalised to "file:pathid"
  tex/<id>.png   textures (Texture2D) as RGBA
  fonts/<id>.ttf fonts
  text/<id>.txt  TextAssets
Usage: python nguprep.py [--no-tex]
"""
import json, os, struct, sys, collections
import UnityPy
from UnityPy.helpers.TypeTreeGenerator import TypeTreeGenerator

GAME = r"J:\NGUvita\NGU IDLE"
D = GAME + r"\NGUIdle_Data"
OUT = r"J:\NGUvita\nguvita\out\ngu"
NO_TEX = "--no-tex" in sys.argv
os.makedirs(OUT + r"\tex", exist_ok=True)
os.makedirs(OUT + r"\fonts", exist_ok=True)
os.makedirs(OUT + r"\text", exist_ok=True)

env = UnityPy.load(D)
g = TypeTreeGenerator("2019.4.22f1")
g.load_local_game(GAME)
env.typetree_generator = g

# --- project tags: custom tag ids are 20000 + index into TagManager.tags (the game compares tag names, e.g. "Menu")
for _o in env.objects:
    if _o.type.name == "TagManager":
        json.dump({"tags": list(_o.read_typetree()["tags"])}, open(OUT + r"	ags.json", "w"))
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
                p = OUT + "\\tex\\" + key.replace(":", "_") + ".png"
                if not os.path.exists(p):
                    try:
                        d.image.save(p)
                    except Exception as e:
                        errors["tex:" + repr(e)[:50]] += 1
        elif t == "Font":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
            data = bytes(d.m_FontData) if d.m_FontData else b""
            if data:
                open(OUT + "\\fonts\\" + key.replace(":", "_") + ".ttf", "wb").write(data)
                rec["file"] = key.replace(":", "_") + ".ttf"
            rec["d"] = norm(af, o.read_typetree()) if False else {}
        elif t == "TextAsset":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
            raw = d.m_Script
            raw = raw.encode("utf-8", "surrogateescape") if isinstance(raw, str) else bytes(raw)
            open(OUT + "\\text\\" + key.replace(":", "_") + ".bin", "wb").write(raw)
        elif t == "AudioClip":
            d = o.read()
            rec = {"t": t, "name": d.m_Name}
        else:
            rec = {"t": t, "d": norm(af, o.read_typetree())}
        objs[key] = rec
    except Exception as e:
        errors[t + ":" + repr(e)[:60]] += 1

json.dump({"objects": objs, "scripts": scripts}, open(OUT + r"\scene.json", "w"), separators=(",", ":"))
print("exported objects:", len(objs))
print("types:", collections.Counter(v["t"] for v in objs.values()).most_common())
print("errors:", errors.most_common(10))
