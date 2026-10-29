# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): the Backlog 3 commands (S3-01 to S3-13).
# Formulas tab's function menus, Advanced Filter, Consolidate, the Review tools, Custom Views,
import os
import sys
import tempfile
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


# dialogs: a queue of callbacks that fill the dialog and return the response
DIALOG = []


def fake_run(d):
    pump(0.05)
    if DIALOG:
        return DIALOG.pop(0)(d)
    return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = lambda d: (DIALOG.pop(0)(d) if DIALOG else Gtk.ResponseType.OK)


def widgets(w, kind):
    out = []

    def walk(x):
        if isinstance(x, kind):
            out.append(x)
        if isinstance(x, Gtk.Container):
            for c in x.get_children():
                walk(c)
    walk(w)
    return out


g = win.grid
b = win.book
TMP = tempfile.mkdtemp()
from folio_sheets.core import Book  # noqa: E402
from folio_sheets import data_ui, clip_ui  # noqa: E402

# ---------------------------------------------------------------- S3-01 copy sheets between workbooks
for r in range(4):
    b.set_input(0, r, 0, str((r + 1) * 10))
b.restyle(0, [(0, 0, 0, 0)], [{"font": {"bold": True, "color": 0xC00000}, "fill": {"pattern": "solid", "fg": 0xFFFF00}}], "Fmt")
win.run("insert-sheet")
pump()
S2 = g.sheet
b.set_input(S2, 0, 0, "7")
g.set_sheet(0)
b.set_input(0, 0, 1, "=A1+%s!A1" % b.sheet_info(S2)["name"])
b.set_input(0, 1, 1, "=SUM(A1:A4)")
pump()
w2 = win.app.new_window()
pump(0.3)
n2 = w2.book.sheet_count
dest = win.act_move_copy_sheet(to_book=w2, before=n2, copy=True)
pump(0.3)
B2 = w2.book
check("xcopy: sheet added to the other workbook", B2.sheet_count == n2 + 1, B2.sheet_count)
j = B2.sheet_count - 1
check("xcopy: values", [B2.input(j, r, 0) for r in range(4)] == ["10", "20", "30", "40"], [B2.input(j, r, 0) for r in range(4)])
check("xcopy: own-sheet formula", B2.input(j, 1, 1) == "=SUM(A1:A4)" and B2.display(j, 1, 1) == "100", (B2.input(j, 1, 1), B2.display(j, 1, 1)))
f = B2.input(j, 0, 1)
check("xcopy: reference to the source's other sheet becomes a link", "[1]" in f, f)
check("xcopy: and keeps its value", B2.display(j, 0, 1) == "17", B2.display(j, 0, 1))
fmt = B2.shown_colors(j, 0, 0)
check("xcopy: formats come along", fmt[0] == 0xFFFF00 and fmt[1] == 0xC00000, fmt)
check("xcopy: source untouched", b.sheet_count == 2)
p = os.path.join(TMP, "dest.xlsx")
B2.save(p)
bb = Book.open(p)
check("xcopy: saved with its link", bb.sheet_count == n2 + 1 and len(bb.ext_links()) == 1 and bb.display(j, 0, 1) == "17",
      (bb.ext_links(), bb.display(j, 0, 1)))
# move to a new book
w3 = win.act_move_copy_sheet(to_book="new", before=0, copy=False)
pump(0.3)
check("move to new book: one sheet, the moved one", w3 is not None and w3.book.sheet_count == 1 and w3.book.input(0, 0, 0) == "10",
      w3 and (w3.book.sheet_count, w3.book.input(0, 0, 0)))
check("move to new book: gone from the source", b.sheet_count == 1, b.sheet_count)
w3.book.mark_saved()
w3.destroy()
pump(0.2)
win.run("undo")
pump(0.2)

# ---------------------------------------------------------------- S3-03 paste link between workbooks
S = g.sheet
for r in range(3):
    b.set_input(S, r, 3, str(r + 5))
g.sel.ranges = [(0, 3, 2, 3)]
g.sel.active = (0, 3)
clip_ui.copy(win)
w2.grid.set_sheet(0)
w2.grid.sel.ranges = [(5, 5, 5, 5)]
w2.grid.sel.active = (5, 5)
ok = clip_ui.paste(w2, {"link": True})
pump(0.2)
f = B2.input(0, 5, 5)
check("paste link: formulas to the other workbook", ok and f.startswith("=[") and "$D$1" in f, (ok, f))
check("paste link: values", [B2.display(0, 5 + r, 5) for r in range(3)] == ["5", "6", "7"], [B2.display(0, 5 + r, 5) for r in range(3)])
links = B2.ext_links()
check("paste link: one link to the source", len(links) == 1, links)
b.set_input(S, 0, 3, "50")
lk = links[0]["index"] if links and "index" in links[0] else 1
win.save_to(os.path.join(TMP, "src.xlsx")) if hasattr(win, "save_to") else b.save(os.path.join(TMP, "src.xlsx"))

# ---------------------------------------------------------------- S3-02 sort and filter by colour
win.run("insert-sheet")
pump()
S = g.sheet
b.set_input(S, 0, 0, "Name")
cols = [None, 0xFF0000, None, 0x00B050, 0xFF0000, None]
for r, c in enumerate(cols):
    b.set_input(S, r + 1, 0, "n%d" % r)
    if c is not None:
        b.restyle(S, [(r + 1, 0, r + 1, 0)], [{"fill": {"pattern": "solid", "fg": c}}], "Fill")
b.restyle(S, [(3, 0, 3, 0)], [{"font": {"color": 0x0070C0}}], "Font")
seen = b.colors_in(S, (1, 0, 6, 0), "cell")
check("colours in a column", [x[0] for x in seen] == [None, 0xFF0000, 0x00B050], seen)
b.sort_range(S, (0, 0, 6, 0), [(0, True, "cell", 0xFF0000, "", 0)], True)
order = [b.input(S, r, 0) for r in range(1, 7)]
check("sort by cell colour: red on top, rest in order", order == ["n1", "n4", "n0", "n2", "n3", "n5"], order)
b.sort_range(S, (0, 0, 6, 0), [(0, False, "font", 0x0070C0, "", 0)], True)
order = [b.input(S, r, 0) for r in range(1, 7)]
check("sort by font colour on bottom", order[-1] == "n2", order)
# the Sort dialog
g.sel.ranges = [(0, 0, 6, 0)]
g.sel.active = (1, 0)


def sort_green(d):
    d.header.set_active(True)
    d.set_level(0, col=0, on=1, color=0x00B050, top=True)
    return Gtk.ResponseType.OK


DIALOG.append(sort_green)
data_ui.sort_custom(win)
check("Sort dialog: Sort On Cell Color", b.input(S, 1, 0) == "n3", [b.input(S, r, 0) for r in range(1, 7)])
# Filter by Color
b.set_autofilter(S, (0, 0, 6, 0))
b.filter_column(S, 0, {"color": (True, 0xFF0000)})
vis = sorted(b.input(S, r, 0) for r in range(1, 7) if not b.row_hidden(S, r))
check("filter by colour: only the red rows show", vis == ["n1", "n4"], vis)
af = b.autofilter(S)
check("filter by colour: kept as a colour filter", af["columns"].get(0, {}).get("color") == (True, 0xFF0000), af)
p = os.path.join(TMP, "colors.xlsx")
b.save(p)
bb = Book.open(p)
af2 = bb.autofilter(S)
check("filter by colour: saved and read back", af2 and af2["columns"].get(0, {}).get("color") == (True, 0xFF0000), af2)
fm = data_ui.FilterMenu(win, 0)
names = [x[0] for x in fm.column_colors("cell")]
check("filter menu lists the column's fills", 0xFF0000 in names and 0x00B050 in names, names)
fm.filter_by_color("cell", 0x00B050)
pump()
af = b.autofilter(S)
check("filter menu: Filter by Color applies", af["columns"].get(0, {}).get("color") == (True, 0x00B050), af)
b.filter_column(S, 0, None)

# ---------------------------------------------------------------- end
w2.book.mark_saved()
for p in tuple(globals().get("EXTRA", ())):
    p()

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
