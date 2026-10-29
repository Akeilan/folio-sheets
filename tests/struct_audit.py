# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Insert/Delete rows, columns and cells through
# the window commands, checked against the engine, undo/redo and a saved file.
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


CHOICE = []


def fake_run(d):
    pump(0.05)
    want = CHOICE.pop(0) if CHOICE else None
    if want is None:
        return Gtk.ResponseType.CANCEL
    radios = []

    def walk(w):
        if isinstance(w, Gtk.RadioButton):
            radios.append(w)
        if isinstance(w, Gtk.Container):
            for c in w.get_children():
                walk(c)
    walk(d)
    for r in radios:
        if r.get_label().replace("_", "").lower().startswith(want):
            r.set_active(True)
    return Gtk.ResponseType.OK


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = lambda d: Gtk.ResponseType.OK

from folio_sheets.core import MAX_COLS, MAX_ROWS  # noqa: E402

g = win.grid
b = win.book
S = 0


def sel(*rgs):
    g.sel.ranges = [tuple(r) for r in rgs]
    g.sel.active = rgs[0][:2]


def inp(r, c):
    return b.input(S, r, c)


for r in range(6):
    b.set_input(S, r, 0, str(r + 1))
    b.set_input(S, r, 1, "=A%d*10" % (r + 1))
b.set_input(S, 0, 3, "=SUM(A1:A6)")
b.insert_sheet(1, "Other")
b.set_input(1, 0, 0, "=Sheet1!A4")
b.define_name("Top", "=Sheet1!$A$1:$A$3")
b.set_input(S, 8, 0, "=SUM(Top)")
b.restyle(S, [(2, 0, 2, 1)], [{"fill": {"pattern": "solid", "fg": 0xFFFF00}}], "Fill")
b.set_merges(S, [(10, 4, 10, 5)])
n0 = len(b.undo_labels())

sel((2, 0, 3, MAX_COLS - 1))
win.run("insert-rows")
check("two rows inserted above row 3", inp(4, 0) == "3" and inp(0, 3) == "=SUM(A1:A8)", (inp(4, 0), inp(0, 3)))
check("formulas that moved still point at their row", inp(4, 1) == "=A5*10" and b.value(S, 4, 1) == 30.0)
check("other sheets follow", b.input(1, 0, 0) == "=Sheet1!A6" and b.value(1, 0, 0) == 4.0, b.input(1, 0, 0))
check("names follow", [n for n in b.names() if n[0] == "Top"][0][1] == "Sheet1!$A$1:$A$5", b.names())
check("inserted rows take the format of the row above", b.cell_style(S, 2, 0)["fill"] is None and b.cell_style(S, 4, 0)["fill"] == 0xFFFF00)
check("merges move down", tuple(b.merges(S)[0]) == (12, 4, 12, 5), b.merges(S))
check("one undo step", len(b.undo_labels()) == n0 + 1, b.undo_labels()[:3])

sel((0, 1, MAX_ROWS - 1, 1))
win.run("insert-columns")
check("column inserted before B", inp(4, 2) == "=A5*10" and inp(0, 4) == "=SUM(A1:A8)", (inp(4, 2), inp(0, 4)))
win.run("delete-columns")
check("column deleted again", inp(4, 1) == "=A5*10")

sel((4, 0, 4, 0))
CHOICE.append("shift cells down")
win.run("insert-cells")
check("insert cells shift down: only column A moves", inp(5, 0) == "3" and inp(4, 1) == "=A6*10" and b.value(S, 4, 1) == 30.0,
      (inp(5, 0), inp(4, 1)))
CHOICE.append("shift cells up")
win.run("delete-cells")
check("delete cells shift up puts it back", inp(4, 0) == "3" and inp(4, 1) == "=A5*10")
CHOICE.append("shift cells right")
win.run("insert-cells")
check("insert cells shift right", inp(4, 1) == "3" and inp(4, 2) == "=B5*10", (inp(4, 1), inp(4, 2)))
CHOICE.append("shift cells left")
win.run("delete-cells")
check("delete cells shift left", inp(4, 0) == "3" and inp(4, 1) == "=A5*10")
CHOICE.append(None)
win.run("insert-cells")
check("cancel changes nothing", inp(4, 0) == "3")

sel((12, 5, 12, 5))
CHOICE.append("shift cells down")
win.run("insert-cells")
check("shifting part of a merge is refused", tuple(b.merges(S)[0]) == (12, 4, 12, 5))

sel((5, 0, 5, MAX_COLS - 1))
win.run("delete-rows")
check("deleting a referenced row gives #REF!", b.input(1, 0, 0) == "=Sheet1!#REF!" and b.value(1, 0, 0) == ("error", "#REF!"), b.input(1, 0, 0))
check("sums shrink", inp(0, 3) == "=SUM(A1:A7)" and b.value(S, 0, 3) == 17.0, (inp(0, 3), b.value(S, 0, 3)))

while len(b.undo_labels()) > n0:
    win.run("undo")
check("undo all returns everything", inp(2, 0) == "3" and inp(0, 3) == "=SUM(A1:A6)" and b.input(1, 0, 0) == "=Sheet1!A4"
      and tuple(b.merges(S)[0]) == (10, 4, 10, 5) and b.cell_style(S, 2, 0)["fill"] == 0xFFFF00)
while b.can_redo:
    win.run("redo")
check("redo all", b.input(1, 0, 0) == "=Sheet1!#REF!" and inp(0, 3) == "=SUM(A1:A7)")

path = "/tmp/struct_audit.xlsx"
b.save(path)
from folio_sheets.core import Book  # noqa: E402
c = Book.open(path)
check("saved file agrees", c.input(S, 0, 3) == "=SUM(A1:A7)" and c.value(S, 0, 3) == 17.0 and c.input(1, 0, 0) == "=Sheet1!#REF!")

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
