import sys
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2]); b.set_input(0, 0, 0, "1")
def ok(f):
    b.set_input(0, 5, 5, "=" + f); x = b.value(0, 5, 5); return not isinstance(x, (list, tuple))
for name, mk in (("A1+A1+... terms", lambda n: "+".join(["A1"] * n)), ("A1&A1&... terms", lambda n: "&".join(["A1"] * n)), ("A1*A1*...", lambda n: "*".join(["A1"] * n)), ("SUM(A1,A1,...) args", lambda n: "SUM(" + ",".join(["A1"] * n) + ")"), ("nested brackets", lambda n: "(" * n + "1" + ")" * n), ("nested IF", lambda n: "IF(1," * n + "1" + ",0)" * n), ("nested SUM(", lambda n: "SUM(" * n + "1" + ")" * n), ("IF chain in else", lambda n: "IF(A1=0,0," * n + "9" + ")" * n), ("recursive LAMBDA depth", lambda n: "LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,%d))" % n)):
    lo, hi = 1, 20000
    if ok(mk(hi)): print("%-28s fine up to %d" % (name, hi)); continue
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if ok(mk(mid)): lo = mid
        else: hi = mid
    b.set_input(0, 5, 5, "=" + mk(hi)); print("%-28s works up to %d, then %r (formula length %d)" % (name, lo, tuple(b.value(0, 5, 5)), len(mk(hi)) + 1))
