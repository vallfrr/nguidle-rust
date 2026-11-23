"""Inventory of the game's Unity objects (read-only)."""
import argparse
import collections
import UnityPy

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("game_data", help="path to the installation's NGUIdle_Data directory")
args = parser.parse_args()
env = UnityPy.load(args.game_data)
cnt = collections.Counter()
size = collections.Counter()
for o in env.objects:
    cnt[o.type.name] += 1
    size[o.type.name] += o.byte_size
print("files:", len(env.files))
for t, n in cnt.most_common():
    print(f"{t:28} {n:7} {size[t]/1024:10.0f} KB")
