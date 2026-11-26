"""Write the layouts of engine value types and event base classes for the Rust exporter.

Usage: python dump_engine_types.py GAME_DATA_DIR OUTPUT.json
Each entry is read from the type layout of a script field that uses the type, so
the file holds engine metadata only.
"""
import json
import sys

from UnityPy.helpers.TypeTreeGenerator import TypeTreeGenerator

# managed type -> (assembly, class, field that has that type)
TYPES = {
    "UnityEngine.Color": ("UnityEngine.UI.dll", "UnityEngine.UI.Image", "m_Color"),
    "UnityEngine.Vector2": ("UnityEngine.UI.dll", "UnityEngine.UI.CanvasScaler", "m_ReferenceResolution"),
    "UnityEngine.LayerMask": ("UnityEngine.UI.dll", "UnityEngine.UI.GraphicRaycaster", "m_BlockingMask"),
    "UnityEngine.RectOffset": ("UnityEngine.UI.dll", "UnityEngine.UI.GridLayoutGroup", "m_Padding"),
    "UnityEngine.Random/State": ("Assembly-CSharp.dll", "Inventory", "boostCombineState"),
    "UnityEngine.GUIStyle": ("UnityEngine.IMGUIModule.dll", "UnityEngine.GUISkin", "m_box"),
}
# event base class -> a class whose event field has no fields of its own
BASES = {
    "UnityEngine.Events.UnityEvent": ("UnityEngine.UI.dll", "UnityEngine.UI.Button", "m_OnClick"),
    "UnityEngine.Events.UnityEvent`1": ("UnityEngine.UI.dll", "UnityEngine.UI.ScrollRect", "m_OnValueChanged"),
}


def rows(generator, assembly, cls, field):
    nodes = generator.get_nodes(assembly, cls)
    start = next(i for i, n in enumerate(nodes) if n.m_Name == field and n.m_Level == 1)
    end = next((i for i in range(start + 1, len(nodes)) if nodes[i].m_Level <= 1), len(nodes))
    base = nodes[start].m_Level
    return [[n.m_Level - base, n.m_Type, n.m_Name, n.m_MetaFlag or 0] for n in nodes[start:end]]


generator = TypeTreeGenerator("2019.4.22f1")
generator.load_local_dll_folder(sys.argv[1] + "/Managed")
out = {"types": {}, "bases": {}}
for name, spec in TYPES.items():
    out["types"][name] = rows(generator, *spec)
for name, spec in BASES.items():
    # the children of the event field are the fields the base class brings
    out["bases"][name] = rows(generator, *spec)[1:]
with open(sys.argv[2], "w", encoding="utf-8") as file:
    json.dump(out, file, separators=(",", ":"))
print(len(out["types"]), "types,", len(out["bases"]), "bases")
