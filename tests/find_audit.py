# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): Find and Replace, Go To Special.
import os
import sys
import time
import traceback

from gi.repository import Gtk

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


SEEN = []
SCRIPT = []


def fake_run(d):
    pump(0.02)
    SEEN.append((d.get_title(), getattr(d, "props", None) and d.props.text if isinstance(d, Gtk.MessageDialog) else ""))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    return fn(d)


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

g = win.grid
b = win.book
S = 0
vals = {(0, 0): "apple pie", (1, 0): "Apple", (2, 0): "banana", (3, 0): "=A1&\"!\"", (4, 1): "12", (5, 1): "=B5*2",
        (6, 2): "pineapple", (7, 0): "TRUE"}
for (r, c), v in vals.items():
    b.set_input(S, r, c, v)
b.insert_sheet(1, "Two")
b.set_input(1, 3, 3, "apple tart")
g.select_cell(0, 0)

win.run("find")
d = win.find_dialog
check("Find opens the dialog", d is not None and d.get_visible())
d.what.set_text("apple")
h = d.find_next()
check("Find Next goes to the next match", h == (0, 1, 0), h)
h = d.find_next()
check("then on, by rows (formulas are searched as typed)", h == (0, 6, 2), h)
h = d.find_next()
check("wraps around", h == (0, 0, 0), h)
d.case.set_active(True)
h = d.find_next()
check("match case skips Apple", h == (0, 6, 2), h)
d.case.set_active(False)
d.look.set_active(1)
hits = d.find_all()
check("Find All in values: A4 shows 'apple pie!'", (0, 3, 0) in hits and len(hits) == 4, hits)
check("results list filled", len(d.results) == 4)
d.whole.set_active(True)
hits = d.find_all()
check("match entire cell contents", hits == [(0, 1, 0)], hits)
d.whole.set_active(False)
d.what.set_text("app*e")
hits = d.find_all()
check("wildcards", [x[1:] for x in hits] == [(0, 0), (1, 0), (3, 0), (6, 2)], hits)
d.within.set_active(1)
d.what.set_text("tart")
h = d.find_next()
check("within workbook moves to the other sheet", h == (1, 3, 3) and g.sheet == 1, (h, g.sheet))
g.set_sheet(0)
d.within.set_active(0)
SCRIPT.append(lambda dd: Gtk.ResponseType.OK)
d.what.set_text("zzz")
h = d.find_next()
check("not found says so", h is None and "cannot find" in (SEEN[-1][1] or ""), SEEN[-1:])

# ---- replace
win.run("replace")
check("Replace tab", d.replace_mode and d.with_.get_visible())
d.what.set_text("apple")
d.with_.set_text("pear")
g.select_cell(0, 0)
d.replace_one()
check("Replace changes the active cell and finds the next", b.input(S, 0, 0) == "pear pie" and g.sel.active == (1, 0),
      (b.input(S, 0, 0), g.sel.active))
check("the formula that refers to it recalculates", b.value(S, 3, 0) == "pear pie!")
SCRIPT.append(lambda dd: Gtk.ResponseType.OK)
n = d.replace_all()
check("Replace All on the sheet", n == 2 and b.input(S, 1, 0) == "pear" and b.input(S, 6, 2) == "pinepear", (n, b.input(S, 1, 0)))
check("other sheet untouched", b.input(1, 3, 3) == "apple tart")
win.run("undo")
check("Replace All is one undo step", b.input(S, 1, 0) == "Apple" and b.input(S, 6, 2) == "pineapple" and b.input(S, 0, 0) == "pear pie")
d.what.set_text("2")
d.with_.set_text("3")
SCRIPT.append(lambda dd: Gtk.ResponseType.OK)
d.replace_all()
check("replace inside formulas", b.input(S, 5, 1) == "=B5*3" and b.input(S, 4, 1) == "13" and b.value(S, 5, 1) == 39.0,
      (b.input(S, 5, 1), b.value(S, 5, 1)))
d.hide()

# ---- Go To Special
g.select_cell(0, 0)
from folio_sheets import find_ui  # noqa: E402
out = find_ui.goto_special(win, "formulas")
check("Formulas", out == [(3, 0, 3, 0), (5, 1, 5, 1)], out)
g.select_cell(0, 0)
out = find_ui.goto_special(win, "constants", 1)
check("Constants: numbers only", out == [(4, 1, 4, 1)], out)
g.sel.ranges = [(0, 0, 2, 1)]
out = find_ui.goto_special(win, "blanks")
check("Blanks inside the selection", out == [(0, 1, 2, 1)], out)
g.select_cell(1, 0)
out = find_ui.goto_special(win, "region")
check("Current region", out and out[0][0:2] == (0, 0), out)
g.select_cell(0, 0)
out = find_ui.goto_special(win, "lastcell")
check("Last cell", out == [(7, 2, 7, 2)], out)


def special(dd):
    for w in dd.get_content_area().get_children():
        if isinstance(w, Gtk.RadioButton) and w.get_label() == "Constants":
            w.set_active(True)
    return Gtk.ResponseType.OK


g.select_cell(0, 0)
SCRIPT.append(special)
win.run("goto-special")
check("Go To Special dialog: constants", g.sel.ranges[0] == (0, 0, 2, 0), g.sel.ranges)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
