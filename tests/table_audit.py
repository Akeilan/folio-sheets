# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Format as Table, the Table Design tab,
# structured references, the total row, filters in a table, and the file.
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


def fake_run(d):
    pump(0.05)
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    return fn(d)


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run


def find(w, name):
    if w.get_name() == name:
        return w
    if isinstance(w, Gtk.Container):
        for c in w.get_children():
            r = find(c, name)
            if r is not None:
                return r
    return None


g = win.grid
b = win.book
S = 0
rows = [["Region", "Units", "Price"], ["East", "10", "2.5"], ["West", "20", "3"], ["North", "15", "4"], ["South", "5", "2"]]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b.set_input(S, r, c, v)
g.sel.ranges = [(2, 1, 2, 1)]
g.sel.active = (2, 1)


# the gallery builds
from folio_sheets import table_ui
pop = table_ui.gallery(win, lambda n: None)
check("the style gallery has 60 styles", len(pop.buttons) == 60, len(pop.buttons))


def create(d):
    check("Create Table guesses the range and headers", find(d, "range").get_text() == "=$A$1:$C$5" and find(d, "headers").get_active(),
          find(d, "range").get_text())
    return Gtk.ResponseType.OK


SCRIPT.append(create)
win.run("format-as-table:TableStyleMedium9")
ts = b.tables(S)
check("a table was made", len(ts) == 1 and ts[0]["ref"] == (0, 0, 4, 2) and ts[0]["style"] == "TableStyleMedium9", ts)
check("Table Design shows and is current", win.ribbon.tab_by_name("Table Design").button.get_visible()
      and win.ribbon.current is win.ribbon.tab_by_name("Table Design"))
check("its name shows", win.table_name.get_text() == "Table1")
check("one undo step", b.undo_labels()[0] in ("Format as Table", "Table"), b.undo_labels()[:2])

# a calculated column typed beside the table
g.sel.ranges = [(0, 3, 0, 3)]
g.sel.active = (0, 3)
b.set_input(S, 0, 3, "Amount")
b.set_input(S, 1, 3, "=[@Units]*[@Price]")
check("the table took in the new column", b.tables(S)[0]["ref"] == (0, 0, 4, 3))
check("the formula filled the column", b.value(S, 4, 3) == 10.0 and b.input(S, 3, 3) == "=[@Units]*[@Price]", (b.value(S, 4, 3), b.input(S, 3, 3)))

# Total Row from the tab's check box
g.sel.ranges = [(1, 1, 1, 1)]
g.sel.active = (1, 1)
win.on_selection_changed()
win.table_checks["totals"].set_active(True)
pump(0.02)
check("Total Row adds totals", b.tables(S)[0]["totals"] and b.value(S, 5, 3) == 25 + 60 + 60 + 10, b.value(S, 5, 3))
check("and the label", b.display(S, 5, 0) == "Total")
table_ui.set_total(win, 0, 1, "max")
check("the totals drop-down sets a function", b.value(S, 5, 1) == 20.0 and b.input(S, 5, 1) == "=SUBTOTAL(104,[Units])", b.input(S, 5, 1))
# the drop-down button beside the active total cell
g.sel.ranges = [(5, 1, 5, 1)]
g.sel.active = (5, 1)
x, y, w, h = b.cell_rect(S, 5, 1, g.params())
hit = b.hit(S, x + w + 8, y + h - 6, g.params())
check("a total cell has its drop-down", hit[0] == "totalsbtn", hit)
m = table_ui.totals_menu(win, 5, 1, (x, y, w, h))
check("the drop-down lists the functions", m is not None and len(m.get_children()) == 9)
m.destroy()

# options
win.table_checks["first_col"].set_active(True)
win.table_checks["row_stripes"].set_active(False)
t = b.tables(S)[0]
check("First Column and Banded Rows", t["first_col"] and not t["row_stripes"])
win.run("table-style:TableStyleLight9")
check("a new style from Quick Styles", b.tables(S)[0]["style"] == "TableStyleLight9")

# filter a table column
x, y, w, h = b.cell_rect(S, 0, 0, g.params())
hit = b.hit(S, x + w - 6, y + h - 6, g.params())
check("the header has filter buttons", hit[0] == "filterbtn", hit)
b.filter_column(S, 0, {"values": ["East", "North"], "blank": False}, (0, 0))
check("filtering the table hides rows", b.row_hidden(S, 2) and not b.row_hidden(S, 1) and not b.row_hidden(S, 3))
check("SUBTOTAL in the total row counts the shown rows", b.value(S, 5, 3) == 25 + 60, b.value(S, 5, 3))
b.clear_filters(S, (0, 0))
check("clearing the filter shows them", not b.row_hidden(S, 2))

# outside formulas, renames
b.set_input(S, 10, 0, "=SUM(Table1[Amount])")
check("a structured reference from outside", b.value(S, 10, 0) == 155.0, b.value(S, 10, 0))
win.table_name.set_text("Sales")
win.table_name.activate()
check("renaming in the tab", b.tables(S)[0]["name"] == "Sales" and b.input(S, 10, 0) == "=SUM(Sales[Amount])", b.input(S, 10, 0))
b.set_input(S, 0, 3, "Revenue")
check("renaming a header follows into formulas", b.input(S, 10, 0) == "=SUM(Sales[Revenue])" and b.input(S, 1, 3) == "=[@Units]*[@Price]",
      (b.input(S, 10, 0), b.input(S, 1, 3)))

# saved and opened again
path = "/tmp/table_audit.xlsx"
win.save_to(path)
from folio_sheets import core
b2 = core.Book.open(path)
t2 = b2.tables(0)
check("the table is saved", len(t2) == 1 and t2[0]["name"] == "Sales" and t2[0]["totals"] and t2[0]["style"] == "TableStyleLight9", t2)
check("its formulas work after opening", b2.value(0, 10, 0) == 155.0 and b2.value(0, 3, 3) == 60.0)

# Convert to Range, then undo
g.sel.ranges = [(1, 1, 1, 1)]
g.sel.active = (1, 1)
win.run("table-convert")
check("Convert to Range", b.tables(S) == [] and b.input(S, 10, 0) == "=SUM($D$2:$D$5)" and b.value(S, 10, 0) == 155.0, b.input(S, 10, 0))
check("the tab goes", not win.ribbon.tab_by_name("Table Design").button.get_visible())
win.run("undo")
check("undo brings the table back", len(b.tables(S)) == 1)

pump(0.1)
shot = os.environ.get("FOLIO_SHOT", "/tmp/table_audit.png")
b.render_png(S, shot, g.params({"width": 700.0, "height": 300.0}))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
