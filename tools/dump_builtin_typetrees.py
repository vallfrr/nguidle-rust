"""Write Unity's built-in type layouts (from UnityPy's type database) for the Rust exporter.

Usage: python dump_builtin_typetrees.py GAME_DATA_DIR OUTPUT.json
Only engine metadata is written, nothing from the game files themselves.
"""
import json
import sys

import UnityPy
from UnityPy.helpers.TypeTreeGenerator import TypeTreeGenerator  # noqa: F401

WANT = {1, 4, 20, 21, 28, 49, 78, 83, 115, 128, 147, 213, 222, 223, 224}


def pack(node):
    return [node.m_Type, node.m_Name, node.m_MetaFlag or 0, [pack(c) for c in node.m_Children]]


env = UnityPy.load(sys.argv[1])
trees = {}
for obj in env.objects:
    if obj.type.value in WANT and obj.type.value not in trees:
        trees[obj.type.value] = pack(obj._get_typetree_node(None))
missing = WANT - trees.keys()
if missing:
    sys.exit(f"No layout found for classes {sorted(missing)}")
with open(sys.argv[2], "w", encoding="utf-8") as out:
    json.dump({str(k): trees[k] for k in sorted(trees)}, out, separators=(",", ":"))
print(f"{len(trees)} classes written")
