exec(open("/tmp/b6/uiharn.py").read())
focus(); S = g.sheet
at(0, 0); typ("=IMSUM(9.99e307,9.99e307)"); key("Return"); pump(0.3)
info("after typing a formula that panics the engine", dict(editing=bool(g.editing()), A1=(I(0, 0), V(0, 0)), msg=MSG[-1:], errors=len(ERRORS)))
if g.editing(): key("Escape")
at(1, 0); typ("5"); key("Return"); ck("the app still takes input afterwards", V(1, 0), 5.0)
b.set_input(S, 2, 0, "0.5"); at(2, 0); n0 = len(ERRORS)
try: win.apply_number_format(".00"); r = "applied"
except BaseException as e: r = "raised %s: %s" % (type(e).__name__, str(e)[:60])
pump(0.5); info("number format .00 applied through the window", r)
try: d = D(2, 0)
except BaseException as e: d = "display raised " + str(e)[:50]
info("cell shows", d); g.refresh(); pump(0.5); info("errors in callbacks while drawing", len(ERRORS) - n0)
import subprocess; subprocess.run(["import", "-window", "root", "/tmp/b6/panic.png"])
at(3, 0); typ("7"); key("Return"); info("typing after the bad format", (V(3, 0), len(ERRORS)))
ok = None
try: ok = win.save_to("/tmp/b6/panic.xlsx")
except BaseException as e: ok = "save raised " + str(e)[:60]
info("save with the bad format in the sheet", ok)
for e in ERRORS[:3]: print(e[-300:])
ERRORS.clear(); done()
