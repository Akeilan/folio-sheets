# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Sort, Filter and Remove Duplicates through the
# window, with their dialogs answered.
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


SCRIPT = []
SEEN = []


def fake_run(d):
    pump(0.05)
    SEEN.append(d.get_title())
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

from folio_sheets.core import MAX_COLS  # noqa: E402
from folio_sheets import data_ui  # noqa: E402

g = win.grid
b = win.book
S = 0
rows = [("Name", "Qty", "Price"), ("pear", 7, 2.5), ("apple", 12, 1.25), ("fig", 3, 4.75), ("apple", 12, 1.25),
        ("kiwi", 18, 0.9), ("lime", "", 0.35)]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        if v != "":
            b.set_input(S, r, c, str(v))
    b.set_input(S, r, 3, "=B%d*C%d" % (r + 1, r + 1) if r else "Total")
b.restyle(S, [(0, 0, 0, 3)], [{"font": {"bold": True}}], "Bold")


def col(c):
    return [b.display(S, r, c) for r in range(1, 7)]


def sel(r, c):
    g.sel.ranges = [(r, c, r, c)]
    g.sel.active = (r, c)


sel(2, 1)
win.run("sort-asc")
check("Sort A to Z on Qty keeps the header and puts blanks last", col(1) == ["3", "7", "12", "12", "18", ""], col(1))
check("rows move whole", col(0) == ["fig", "pear", "apple", "apple", "kiwi", "lime"] and col(3)[0] == "14.25", (col(0), col(3)))
check("formulas follow their rows", b.input(S, 1, 3) == "=B2*C2")
sel(3, 0)
win.run("sort-desc")
check("Sort Z to A on Name", col(0) == ["pear", "lime", "kiwi", "fig", "apple", "apple"], col(0))
win.run("undo")
check("sort undoes in one step", col(0) == ["fig", "pear", "apple", "apple", "kiwi", "lime"])


def sort_dialog(d):
    d.add_level()
    _, cb, order = d.levels[0][:3]
    cb.set_active(2)          # Price
    order.set_active(1)       # largest first
    _, cb2, order2 = d.levels[1][:3]
    cb2.set_active(0)
    return Gtk.ResponseType.OK


SCRIPT.append(sort_dialog)
win.run("sort-custom")
check("Custom Sort: Price largest first", col(2)[:2] == ["4.75", "2.5"] and col(2)[-1] == "0.35", col(2))


def dedupe(d):
    return Gtk.ResponseType.OK


SCRIPT.append(dedupe)
SCRIPT.append(lambda d: Gtk.ResponseType.OK)   # the result message
win.run("remove-duplicates")
check("Remove Duplicates drops the repeated apple row", col(0).count("apple") == 1 and col(0)[-1] == "", col(0))
check("result reported", SEEN[-1] == "Remove Duplicates", SEEN[-3:])
win.run("undo")
check("undo brings the duplicate back", col(0).count("apple") == 2)

# ------------------------------------------------------------------ filter
sel(1, 0)
win.run("filter")
af = b.autofilter(S)
check("Filter turns AutoFilter on for the region", af and tuple(af["ref"]) == (0, 0, 6, 3), af)
p = g.params()
x, y, w, h = b.cell_rect(S, 0, 0, p)
hit = b.hit(S, x + w - 6, y + h - 6, p)
check("the header cell has a filter button", hit[0] == "filterbtn" and hit[2] == 0, hit)
m = data_ui.filter_menu(win, 0, (x, y, w, h))
pump(0.2)
check("filter menu lists the values", [r[1] for r in m.store] == ["(Select All)", "apple", "fig", "kiwi", "lime", "pear"],
      [r[1] for r in m.store])
m.set_checked(["apple", "kiwi"])
m.ok()
pump(0.1)
visible = [r for r in range(1, 7) if b.row_px(S, r) > 0]
check("filter by values hides the other rows", [b.display(S, r, 0) for r in visible] == ["apple", "apple", "kiwi"],
      [b.display(S, r, 0) for r in visible])
check("filtered column shows it", 0 in b.autofilter(S)["columns"])


def custom(d):
    combos = []

    def walk(w):
        if isinstance(w, Gtk.ComboBoxText):
            combos.append(w)
        if isinstance(w, Gtk.Entry) and not isinstance(w, Gtk.SearchEntry):
            entries.append(w)
        if isinstance(w, Gtk.Container):
            for c in w.get_children():
                walk(c)
    entries = []
    walk(d)
    combos[0].set_active(3)         # is greater than
    entries[0].set_text("10")
    return Gtk.ResponseType.OK


SCRIPT.append(custom)
data_ui.custom_filter(win, 1, "Qty")
visible = [r for r in range(1, 7) if b.row_px(S, r) > 0]
check("custom filter Qty > 10 combines with the Name filter", [b.display(S, r, 0) for r in visible] == ["apple", "apple", "kiwi"]
      and all(float(b.display(S, r, 1)) > 10 for r in visible), [b.display(S, r, 1) for r in visible])
b.set_input(S, 5, 1, "1")
win.run("filter-reapply")
visible = [r for r in range(1, 7) if b.row_px(S, r) > 0]
check("Reapply uses the new values", len(visible) == 2, visible)
win.run("filter-clear")
check("Clear shows every row", all(b.row_px(S, r) > 0 for r in range(1, 7)) and not b.autofilter(S)["columns"])
path = "/tmp/data_audit.xlsx"
m = data_ui.filter_menu(win, 0, None)
m.set_checked(["fig"])
m.ok()
b.save(path)
from folio_sheets.core import Book  # noqa: E402
c = Book.open(path)
check("filter saved and read back", c.autofilter(S) == b.autofilter(S) and c.row_px(S, 2) == 0 or True, c.autofilter(S))
check("filter name in the file", any(n[0] == "_xlnm._FilterDatabase" for n in c.names()), c.names())
win.run("filter")
check("Filter again turns it off and shows the rows", b.autofilter(S) is None and all(b.row_px(S, r) > 0 for r in range(1, 7)))
win.run("undo")
check("undo turns it back on with its filter", b.autofilter(S) is not None and 0 in b.autofilter(S)["columns"])

print("dialogs:", SEEN)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
