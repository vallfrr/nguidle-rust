#!/usr/bin/env python3
"""Compares two saves field by field from their flattened dumps (`NRBF_FLAT=1 clr <dll> nrbf <save>`).

Usage: compare_saves.py <original.flat> <port.flat> [--tolerance 1e-9] [--ignore REGEX ...]
Prints the fields that differ (relative tolerance for numbers) and exits 1 if any does."""
import argparse
import math
import re
import sys

LINE = re.compile(r"^(\S+) = (\w+)\((.*)\)$")


def load(path):
    fields = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            match = LINE.match(line.rstrip("\n"))
            if match:
                fields[match.group(1)] = (match.group(2), match.group(3))
    return fields


def same(left, right, tolerance):
    if left == right:
        return True
    try:
        a, b = float(left[1]), float(right[1])
    except ValueError:
        return False
    if math.isnan(a) and math.isnan(b):
        return True
    return abs(a - b) <= tolerance * max(abs(a), abs(b))


def compare(original, port, tolerance=1e-9, ignore=()):
    skipped = [re.compile(pattern) for pattern in ignore]
    differences = []
    for name in sorted(set(original) | set(port)):
        if any(pattern.search(name) for pattern in skipped):
            continue
        if name not in original or name not in port:
            differences.append((name, original.get(name), port.get(name)))
        elif not same(original[name], port[name], tolerance):
            differences.append((name, original[name], port[name]))
    return differences


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("original")
    parser.add_argument("port")
    parser.add_argument("--tolerance", type=float, default=1e-9)
    parser.add_argument("--ignore", action="append", default=[])
    arguments = parser.parse_args()
    original, port = load(arguments.original), load(arguments.port)
    differences = compare(original, port, arguments.tolerance, arguments.ignore)
    for name, left, right in differences:
        print(f"{name}: original {left} / port {right}")
    print(f"{len(original)} fields in the original, {len(port)} in the port, {len(differences)} differ")
    return 1 if differences else 0


if __name__ == "__main__":
    sys.exit(main())
