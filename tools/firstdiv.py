"""firstdiv.py o_full n_full : first divergence between oracle and native (on engine calls + mutating UnityEngine calls),
then prints the [m] context (user methods) around the divergence in both traces."""
import re, sys
KEEP = re.compile(r'^\[call\]|^\[native\] UnityEngine[A-Za-z.]*::(set_|Set|Translate|Destroy|Instantiate|AddComponent|Invoke|InvokeRepeating|CancelInvoke|StartCoroutine|AddListener|RemoveListener|Play|Stop|Apply)')
def load(p):
    full = [l.rstrip('\n').replace(' ptr', ' struct') for l in open(p, errors='replace')]
    full = [l for l in full if (l.startswith('[m]') or l.startswith('[call]') or l.startswith('[native]')) and '::.ctor' not in l and '::.cctor' not in l]
    idx = [i for i, l in enumerate(full) if KEEP.match(l)]
    return full, idx
of, oi = load(sys.argv[1]); nf, ni = load(sys.argv[2])
n = min(len(oi), len(ni)); d = None
for k in range(n):
    if of[oi[k]] != nf[ni[k]]:
        d = k; break
if d is None:
    print("no divergence over", n, "evenements (lengths", len(oi), len(ni), ")"); sys.exit(0)
print(f"first divergence at event {d}/{n}")
w = int(sys.argv[3]) if len(sys.argv) > 3 else 14
for name, f, ix in (("ORACLE", of, oi), ("NATIF", nf, ni)):
    print("----", name)
    lo = ix[d - 1] if d > 0 else 0
    hi = ix[d] if d < len(ix) else len(f) - 1
    for l in f[max(lo, hi - w): hi + 4]:
        print(("  >> " if l is f[hi] else "     ") + l[:170])
