import csv
import struct
import sys

with open(sys.argv[1], newline='', encoding='utf-8') as source:
    original = list(csv.DictReader(source))
with open(sys.argv[2], newline='', encoding='utf-8') as source:
    port = list(csv.DictReader(source))
if len(original) != len(port):
    raise SystemExit(f'Row count differs: {len(original)} vs {len(port)}')
fields = ('scheduled_tick', 'action', 'menu', 'energy', 'idle', 'cap', 'gained', 'bar')
differences = []
for before, after in zip(original, port):
    for field in fields:
        # Compare the original and port values at the game's f32 precision.
        same = (struct.pack('<f', float(before[field])) == struct.pack('<f', float(after[field]))) if field == 'bar' else before[field] == after[field]
        if not same:
            differences.append((before['scheduled_tick'], field, before[field], after[field]))
for tick, field, before, after in differences:
    print(f'tick {tick}: {field}: original={before} port={after}')
print(f'{len(original)} events, {len(differences)} differing fields')
raise SystemExit(bool(differences))
