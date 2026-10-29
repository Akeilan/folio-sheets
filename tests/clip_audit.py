# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Copy, Cut, Paste and Paste Special, within a
# workbook, between windows, and from other programs (text).
import os
import sys
import time
import traceback

from gi.repository import Gdk, Gtk

checks = []
ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))


def check(name, cond, detail=""):
    checks.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  " + repr(detail)), flush=True)


def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.005)


SCRIPT = []


def fake_run(d):
    pump(0.02)
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    return fn(d)


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

g = win.grid
b = win.book
S = 0
b.set_input(S, 0, 0, "1")
b.set_input(S, 1, 0, "2")
b.set_input(S, 0, 1, "=A1*10")
b.set_input(S, 1, 1, "=A2*10")
b.set_input(S, 5, 5, "=B1+1")
b.restyle(S, [(0, 0, 1, 1)], [{"font": {"bold": True}, "fill": {"pattern": "solid", "fg": 0xFFFF00}}], "Fmt")
b.set_col_width(S, 1, 1, 20.0)


def sel(rg):
    g.sel.ranges = [tuple(rg)]
    g.sel.active = tuple(rg[0:2])


sel((0, 0, 1, 1))
win.run("copy")
check("copy shows the marquee", g.copy_range == (0, 0, 1, 1))
sel((3, 3, 3, 3))
win.run("paste")
check("paste: formulas adjust", b.input(S, 3, 4) == "=D4*10" and b.value(S, 4, 4) == 20.0, b.input(S, 3, 4))
check("paste: formats come too", b.cell_style(S, 4, 4)["bold"] and b.cell_style(S, 4, 4)["fill"] == 0xFFFF00)
check("pasted area selected", tuple(g.sel.ranges[0]) == (3, 3, 4, 4), g.sel.ranges)
check("one undo step", b.undo_labels()[0] == "Paste")
sel((10, 0, 10, 0))
win.run("paste-values")
check("paste values", b.input(S, 10, 1) == "10" and b.input(S, 11, 1) == "20" and not b.cell_style(S, 10, 1)["bold"])
sel((20, 0, 23, 1))
win.run("paste")
check("tiles into a selection twice its size", b.input(S, 22, 1) == "=A23*10" and b.input(S, 23, 0) == "2", b.input(S, 22, 1))
sel((30, 0, 30, 0))
win.run("paste-transpose")
check("transpose", b.input(S, 30, 1) == "2" and b.cell_style(S, 30, 1)["bold"], b.input(S, 30, 1))
sel((40, 5, 40, 5))
win.run("paste-keep-widths")
check("keep source column widths", b.col_width(S, 6) == 20.0, b.col_width(S, 6))
sel((50, 0, 50, 0))
win.run("paste-link")
check("paste link", b.input(S, 50, 0) == "=A1" and b.input(S, 51, 1) == "=B2" and b.value(S, 51, 1) == 20.0)
# Paste Special: multiply
b.set_input(S, 60, 0, "5")
b.set_input(S, 61, 0, "=3")
sel((0, 0, 1, 0))
win.run("copy")


def mult(d):
    for w in d.get_content_area().get_children():
        if isinstance(w, Gtk.Grid):
            for x in w.get_children():
                if isinstance(x, Gtk.RadioButton) and x.get_label() in ("Values", "Multiply"):
                    x.set_active(True)
    return Gtk.ResponseType.OK


SCRIPT.append(mult)
sel((60, 0, 60, 0))
win.run("paste-special")
check("Paste Special values x multiply", b.value(S, 60, 0) == 5.0 and b.input(S, 61, 0) == "=(3)*2", (b.value(S, 60, 0), b.input(S, 61, 0)))
# cut: the cells move and references follow
sel((0, 1, 0, 1))
win.run("cut")
sel((70, 2, 70, 2))
win.run("paste")
check("cut moves the cell", b.input(S, 70, 2) == "=A1*10" and b.input(S, 0, 1) == "")
check("references to it follow", b.input(S, 5, 5) == "=C71+1" and b.value(S, 5, 5) == 11.0, b.input(S, 5, 5))
check("marquee gone after a cut paste", g.copy_range is None)
win.run("undo")
check("undo puts it back", b.input(S, 0, 1) == "=A1*10" and b.input(S, 5, 5) == "=B1+1")

# ---- to another window
w2 = win.app.new_window()
pump(0.3)
sel((0, 0, 1, 1))
win.run("copy")
w2.grid.sel.ranges = [(2, 2, 2, 2)]
w2.grid.sel.active = (2, 2)
w2.run("paste")
b2 = w2.book
check("paste into another workbook: formulas and formats", b2.input(0, 2, 3) == "=C3*10" and b2.cell_style(0, 3, 3)["fill"] == 0xFFFF00,
      (b2.input(0, 2, 3), b2.cell_style(0, 3, 3)))
w2.grid.sel.ranges = [(8, 0, 8, 0)]
w2.grid.sel.active = (8, 0)
w2.run("paste-values")
check("values into another workbook", b2.input(0, 8, 1) == "10" and b2.input(0, 9, 1) == "20", b2.input(0, 8, 1))

# ---- from another program
cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
cb.set_text("x\t1\r\ny\t=1+1\r\n", -1)
pump(0.1)
sel((80, 0, 80, 0))
win.run("paste")
check("tab-separated text from elsewhere", b.input(S, 80, 0) == "x" and b.value(S, 81, 1) == 2.0, (b.input(S, 80, 0), b.value(S, 81, 1)))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
