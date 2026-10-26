# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): Group and Outline — Group / Ungroup, the
# outline bars and their buttons, Show / Hide Detail, Clear Outline, Auto Outline, Subtotal.
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
SEEN = []


def fake_run(d):
    pump(0.02)
    text = d.get_property("text") if isinstance(d, Gtk.MessageDialog) else None
    SEEN.append((d.get_title(), text))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

win.resize(1280, 800)
pump(0.4)
g = win.grid
b = win.book
S = 0


def sel(r, c, r2=None, c2=None):
    g.select_range((r, c, r if r2 is None else r2, c if c2 is None else c2), (r, c))
    pump(0.01)


class Ev:
    def __init__(self, x, y):
        self.x, self.y, self.button, self.type, self.state = x, y, 1, Gdk.EventType.BUTTON_PRESS, 0

    def get_state(self):
        return 0


def hidden(r):
    return b.row_hidden(S, r)


# ---- Group rows
check("no outline: no outline bars", not g.row_gutter.get_visible() and not g.col_gutter.get_visible())
sel(1, 0, 3, 16383)
win.run("group")
pump(0.1)
check("Group on whole rows groups them", [b.outline_level(S, True, r) for r in range(5)] == [0, 1, 1, 1, 0])
check("and the row outline bar shows", g.row_gutter.get_visible() and g.row_gutter.get_allocated_width() == 2 * 16 + 4,
      g.row_gutter.get_allocated_width())
sel(2, 1, 3, 1)
SCRIPT.append(lambda d: Gtk.ResponseType.OK)       # Rows
win.run("group")
check("Group on cells asks rows or columns (rows)", SEEN[-1][0] == "Group" and b.outline_level(S, True, 2) == 2)
# the + / - buttons
pump(0.1)
tg = [t for t in g.row_gutter.targets if t[4][0] == "detail"]
check("each group has its button at its summary row", sorted(t[4][1] for t in tg) == [4, 4], tg)
lv = [t for t in g.row_gutter.targets if t[4][0] == "level"]
check("and the level buttons 1-3", [t[4][1] for t in lv] == [1, 2, 3])
inner = [t for t in tg if t[4][5] == 2][0]
g.row_gutter.on_press(g.row_gutter, Ev(inner[0] + 3, inner[1] + 3))
pump(0.05)
check("the - button hides the inner group", hidden(2) and hidden(3) and not hidden(1))
win.run("undo")
check("undo shows it again", not hidden(2))
x, y = lv[0][0], lv[0][1]
g.row_gutter.on_press(g.row_gutter, Ev(x + 3, y + 3))
pump(0.05)
check("level 1 shows only the summary rows", hidden(1) and hidden(2) and hidden(3) and not hidden(4))
g.row_gutter.on_press(g.row_gutter, Ev(lv[2][0] + 3, lv[2][1] + 3))
pump(0.05)
check("level 3 shows everything", not any(hidden(r) for r in range(1, 5)))
sel(2, 0)
win.run("hide-detail")
check("Hide Detail on a detail row hides its group", hidden(2) and hidden(3) and not hidden(1))
sel(4, 0)
win.run("show-detail")
check("Show Detail on the summary row shows it", not hidden(2) and not hidden(3))

# ---- columns
sel(0, 2, 1048575, 4)
win.run("group")
pump(0.1)
check("Group on whole columns", [b.outline_level(S, False, c) for c in range(6)] == [0, 0, 1, 1, 1, 0])
check("the column outline bar shows", g.col_gutter.get_visible())
ct = [t for t in g.col_gutter.targets if t[4][0] == "detail"]
g.col_gutter.on_press(g.col_gutter, Ev(ct[0][0] + 3, ct[0][1] + 3))
pump(0.05)
check("its - hides the columns", all(b.col_width_px(S, c) == 0 for c in range(2, 5)) if hasattr(b, "col_width_px")
      else b.col_px(S, 3) == 0, b.col_px(S, 3))
sel(0, 2, 1048575, 4)
win.run("ungroup")
check("Ungroup takes the columns out (and shows them)", b.outline_level(S, False, 3) == 0 and b.col_px(S, 3) > 0)

# ---- clear, auto outline
win.run("ungroup:clear")
pump(0.05)
check("Clear Outline", b.max_outline_level(S, True) == 0 and not g.row_gutter.get_visible())
for r, v in enumerate(["Item", "1", "2", "3", "=SUM(A22:A24)", "5", "6", "=SUM(A26:A27)"]):
    b.set_input(S, 20 + r, 0, v)
win.run("group:auto")
check("Auto Outline groups the rows each SUM adds up",
      [b.outline_level(S, True, r) for r in range(20, 28)] == [0, 1, 1, 1, 0, 1, 1, 0],
      [b.outline_level(S, True, r) for r in range(20, 28)])
win.run("ungroup:clear")

# ---- Subtotal
data = [("Region", "Rep", "Sales"), ("East", "Ann", 10), ("East", "Bo", 20), ("West", "Cy", 5), ("West", "Di", 7),
        ("West", "Ed", 8), ("North", "Fa", 1)]
for r, row in enumerate(data):
    for c, v in enumerate(row):
        b.set_input(S, 40 + r, c, str(v))
sel(41, 1)


def st(d):
    d.at.set_active(0)
    d.fn.set_active(0)
    return Gtk.ResponseType.OK


SCRIPT.append(st)
win.run("subtotal")
col0 = [b.display(S, r, 0) for r in range(40, 51)]
check("Subtotal adds a total row per region and a grand total",
      col0 == ["Region", "East", "East", "East Total", "West", "West", "West", "West Total", "North", "North Total",
               "Grand Total"], col0)
check("the totals add up", [b.display(S, r, 2) for r in (43, 47, 49, 50)] == ["30", "20", "1", "51"],
      [b.display(S, r, 2) for r in (43, 47, 49, 50)])
check("with SUBTOTAL formulas", b.input(S, 43, 2) == "=SUBTOTAL(9,C42:C43)", b.input(S, 43, 2))
check("and a 3-level outline", [b.outline_level(S, True, r) for r in (41, 43, 49, 50)] == [2, 1, 1, 0])
win.run("undo")
check("one undo removes it all", b.display(S, 43, 0) == "West" and b.max_outline_level(S, True) == 0,
      b.display(S, 43, 0))
win.run("redo")
sel(41, 1)
SCRIPT.append(lambda d: (d.fn.set_active(1), Gtk.ResponseType.OK)[1])
win.run("subtotal")
check("again with Replace: counts instead of sums, no doubles", b.display(S, 43, 0) == "East Count" and
      b.display(S, 43, 2) == "2" and b.display(S, 50, 0) == "Grand Count", (b.display(S, 43, 0), b.display(S, 50, 0)))
sel(41, 1)
SCRIPT.append(lambda d: 3)
win.run("subtotal")
check("Remove All takes the subtotals away", [b.display(S, r, 0) for r in range(40, 48)] ==
      ["Region", "East", "East", "West", "West", "West", "North", ""] and b.max_outline_level(S, True) == 0,
      [b.display(S, r, 0) for r in range(40, 48)])

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
