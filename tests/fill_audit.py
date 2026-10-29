# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): the fill handle, Fill Series, Flash Fill, Justify.
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
for (r, c), v in {(0, 0): "1", (1, 0): "2", (0, 1): "Jan", (0, 2): "Q1", (0, 3): "=A1+10", (0, 4): "7"}.items():
    b.set_input(S, r, c, v)


def Ev(x, y, kind=Gdk.EventType.BUTTON_PRESS):
    e = Gdk.Event.new(kind)
    e.x, e.y, e.button = x, y, 1
    e.window = g.area.get_window()
    return e


def center(r, c):
    x, y, w, h = b.cell_rect(S, r, c, g.params())
    return x + w / 2, y + h / 2


# a real drag of the fill handle: press on the handle, move, release
g.sel.ranges = [(0, 0, 1, 3)]
g.sel.active = (0, 0)
g.refresh()
pump(0.1)
x, y, w, h = b.cell_rect(S, 1, 3, g.params())
hx, hy = x + w - 1, y + h - 1
check("the fill handle is under the corner", b.hit(S, hx, hy, g.params())[0] == "fill", b.hit(S, hx, hy, g.params()))
h = b.hit(S, hx, hy, g.params())
g._drag = ("fill", tuple(g.sel.ranges[-1]))   # what pressing on the handle starts
tx, ty = center(5, 2)
g._drag_to(tx, ty)
check("dragging previews the target", tuple(g.fill_target) == (0, 0, 5, 3), g.fill_target)
class Rel:
    x, y, state = tx, ty, 0


g.on_release(g.area, Rel())
pump(0.05)
check("numbers continue the series", [b.value(S, r, 0) for r in range(2, 6)] == [3.0, 4.0, 5.0, 6.0])
# B2:D2 are blank, so the pattern (value, blank) repeats
check("months continue", [b.value(S, r, 1) for r in range(2, 6)] == ["Feb", None, "Mar", None], [b.value(S, r, 1) for r in range(2, 6)])
check("text with a number counts", b.value(S, 2, 2) == "Q2" and b.value(S, 4, 2) == "Q3", (b.value(S, 2, 2), b.value(S, 4, 2)))
check("formulas copy relatively", b.input(S, 4, 3) == "=A5+10" and b.input(S, 5, 3) == "", b.input(S, 4, 3))
check("selection becomes the filled range", tuple(g.sel.ranges[-1]) == (0, 0, 5, 3))
win.run("undo")
check("one undo step", b.value(S, 2, 0) is None)
win.fill_handle((0, 4, 0, 4), (0, 4, 3, 4))
check("a single number is copied", b.value(S, 3, 4) == 7.0)
win.fill_handle((0, 4, 0, 4), (0, 4, 3, 4), ctrl=True)
check("Ctrl+drag counts a single number up", b.value(S, 3, 4) == 10.0, b.value(S, 3, 4))
win.fill_handle((0, 4, 3, 4), (0, 4, 1, 4))
check("dragging back inside clears", b.value(S, 2, 4) is None and b.value(S, 1, 4) == 8.0)

# Fill Series dialog
b.set_input(S, 10, 0, "2")
g.sel.ranges = [(10, 0, 20, 0)]
g.sel.active = (10, 0)


def series(d):
    ents = []

    def walk(w):
        if isinstance(w, Gtk.Entry):
            ents.append(w)
        if isinstance(w, Gtk.RadioButton) and w.get_label() == "Growth":
            w.set_active(True)
        if isinstance(w, Gtk.Container):
            for c in w.get_children():
                walk(c)
    walk(d)
    [e for e in ents if e.get_name() == "step"][0].set_text("2")
    [e for e in ents if e.get_name() == "stop"][0].set_text("100")
    return Gtk.ResponseType.OK


SCRIPT.append(series)
win.run("fill-series")
check("Series: growth x2 up to 100", [b.value(S, r, 0) for r in range(10, 17)] == [2.0, 4.0, 8.0, 16.0, 32.0, 64.0, None],
      [b.value(S, r, 0) for r in range(10, 17)])

# Flash Fill
for i, name in enumerate(["Grace Hopper", "Alan Turing", "Ada Lovelace"]):
    b.set_input(S, 30 + i, 0, name)
b.set_input(S, 30, 1, "HOPPER, G")
g.select_cell(30, 1)
win.run("flash-fill")
check("Flash Fill", b.value(S, 31, 1) == "TURING, A" and b.value(S, 32, 1) == "LOVELACE, A", b.value(S, 31, 1))

# Justify
b.set_input(S, 40, 5, "one two three four five six seven eight nine ten eleven twelve")
g.sel.ranges = [(40, 5, 45, 5)]
g.sel.active = (40, 5)
win.run("fill-justify")
check("Justify splits the text over cells", b.display(S, 41, 5) != "" and " ".join(b.display(S, r, 5) for r in range(40, 52)).split()[-1] == "twelve" and b.display(S, 40, 5) == "one two")

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
