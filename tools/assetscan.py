"""Inventory of the game's Unity objects (read-only). Usage: python assetscan.py"""
import collections, sys, UnityPy

D = r"J:\NGUvita\NGU IDLE\NGUIdle_Data"
env = UnityPy.load(D)
cnt = collections.Counter()
size = collections.Counter()
for o in env.objects:
    cnt[o.type.name] += 1
    size[o.type.name] += o.byte_size
print("files:", len(env.files))
for t, n in cnt.most_common():
    print(f"{t:28} {n:7} {size[t]/1024:10.0f} KB")
