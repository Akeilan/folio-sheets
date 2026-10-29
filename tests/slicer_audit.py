# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-20 slicers, timelines and PivotCharts — Insert
# Slicer / Insert Timeline, clicking items and periods on the sheet, Clear Filter, the Slicer and
# Timeline tabs, Report Connections, Slicer Settings, PivotCharts that follow their PivotTable,
# saving and opening.
import os
import sys
import tempfile
import time
import traceback
import zipfile

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


from folio_sheets.core import Book  # noqa: E402
from folio_sheets import pivot_ui as P  # noqa: E402
from folio_sheets import slicer_ui as S  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1400, 860)
pump(0.3)
g = win.grid
b = lambda: win.book  # noqa: E731


def tx(s, r, c):
    v = b().value(s, r, c)
    if isinstance(v, float) and v == int(v):
        v = int(v)
    return "" if v is None else str(v)


class Ev:
    """A button press as the grid gets it."""
    def __init__(self, x, y, state=0):
        self.x, self.y, self.button = float(x), float(y), 1
        self.type = Gdk.EventType.BUTTON_PRESS
        self._state = state

    def get_state(self):
        return self._state


rows = [["Region", "Date", "Sales"], ["East", "2024-01-10", "10"], ["West", "2024-02-11", "20"],
        ["East", "2024-03-12", "30"], ["North", "2024-04-13", "40"], ["West", "2025-01-14", "50"]]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b().set_input(0, r, c, v)
g.set_sheet(0)
g.select_cell(1, 0)
win.on_selection_changed()

# a PivotTable: Region down the rows, Sales summed
d = P.CreateDialog(win)
d.apply()
d.d.destroy()
pump(0.1)
PS = g.sheet
p = b().pivot(0)
p["rows"] = [0]
p["values"] = [{"field": 2, "agg": "sum", "name": "Sum of Sales", "show": "normal"}]
b().set_pivot(0, p)
g.select_cell(3, 0)
win.on_selection_changed()
pump(0.1)
total = lambda: tx(PS, [r for r in range(2, 12) if tx(PS, r, 0) == "Grand Total"][0], 1)  # noqa: E731
check("the PivotTable totals everything", total() == "150", total())

# ---- the ribbon
an = win.ribbon.tab_by_name("PivotTable Analyze")
check("Analyze has Insert Slicer, Insert Timeline, Filter Connections", an is not None)
check("the Slicer tab is hidden until a slicer is selected", not win.ribbon.tab_by_name("Slicer").button.get_visible())

# ---- Insert Slicer
dlg = S.InsertDialog(win, 0)
check("Insert Slicers lists the fields", [c.get_label() for c in dlg.checks] == ["Region", "Date", "Sales"], [c.get_label() for c in dlg.checks])
dlg.tick("Region")
dlg.apply()
dlg.d.destroy()
pump(0.1)
objs = b().objects(PS)
SI = dlg.made[0]
check("a slicer goes on the sheet", objs[SI]["kind"] == "slicer" and objs[SI]["name"] == "Region", objs)
check("selected, with the Slicer tab showing", g.sel_objects == [SI] and win.ribbon.tab_by_name("Slicer").button.get_visible())
check("its items, all chosen", b().slicer_items(PS, SI) == [("East", True), ("North", True), ("West", True)], b().slicer_items(PS, SI))

# ---- clicking items on the sheet
def item_point(i, k):
    x0, y0, w, h = b().object_screen_rect(PS, i, g.params())
    z = g.params()["zoom"]
    for xx in range(4, int(w), 6):
        for yy in range(4, int(h), 4):
            hit = b().slicer_hit(PS, i, float(xx), float(yy), float(w), float(h), float(z))
            if hit == ("item" if b().objects(PS)[i]["kind"] == "slicer" else "period", k):
                return x0 + xx, y0 + yy
    return None


pt = item_point(SI, 0)
check("the items are on the sheet", pt is not None)
g.on_press(g.area, Ev(*pt))
pump(0.1)
check("a click shows only that item", b().slicer_items(PS, SI) == [("East", True), ("North", False), ("West", False)], b().slicer_items(PS, SI))
check("and the PivotTable follows", total() == "40", total())
pt = item_point(SI, 2)
g.on_press(g.area, Ev(pt[0], pt[1], Gdk.ModifierType.CONTROL_MASK))
pump(0.1)
check("Ctrl+click adds one", [on for _t, on in b().slicer_items(PS, SI)] == [True, False, True] and total() == "110", (b().slicer_items(PS, SI), total()))
check("the move didn't start", g._drag is None or g._drag[0] != "obj")
b().undo()
pump(0.1)
check("Undo takes the click back", total() == "40", total())
S.clear(win, SI)
check("Clear Filter shows everything again", total() == "150" and all(on for _t, on in b().slicer_items(PS, SI)), total())
x0, y0, w, h = b().object_screen_rect(PS, SI, g.params())
hit = b().slicer_hit(PS, SI, float(w - 14), float(13), float(w), float(h), float(g.params()["zoom"]))
check("the header has the Clear Filter button", hit == ("clear", 0), hit)

# ---- the Slicer tab and Slicer Settings
g.sel_objects = [SI]
win.on_object_selection()
check("the tab shows the caption", win.slicer_caption.get_text() == "Region", win.slicer_caption.get_text())
S.set_caption(win, "Regions")
win.slicer_columns.set_value(2)
pump(0.05)
info = b().slicer_info(PS, SI)
check("caption and columns change", info["caption"] == "Regions" and info["columns"] == 2, info)
win.run("slicer-style:Dark3")
check("a style from the gallery", b().slicer_info(PS, SI)["style"] == "SlicerStyleDark3", b().slicer_info(PS, SI)["style"])
sd = S.SettingsDialog(win, SI)
sd.caption.set_text("Where")
sd.header.set_active(False)
sd.apply()
sd.d.destroy()
info = b().slicer_info(PS, SI)
check("Slicer Settings: caption and header", info["caption"] == "Where" and not info["header"], info)
b().set_slicer(PS, SI, header=True)

# ---- Insert Timeline
dlg = S.InsertDialog(win, 0, True)
check("Insert Timelines lists only date fields", [c.get_label() for c in dlg.checks] == ["Date"], [c.get_label() for c in dlg.checks])
dlg.tick("Date")
dlg.apply()
dlg.d.destroy()
pump(0.1)
TI = dlg.made[0]
rx = b().objects(PS)[TI]["rect"]
b().set_object_rect(PS, TI, rx[0], rx[1] + 300.0, rx[2], rx[3], "Move Timeline")
check("a timeline goes on the sheet", b().objects(PS)[TI]["kind"] == "timeline" and win.ribbon.tab_by_name("Timeline").button.get_visible())
per = b().timeline_periods(PS, TI)
check("months by default", per[0][:1] == ("JAN",) and per[0][3] == "2024" and len(per) == 13, per[:3])
pt = item_point(TI, 1)
check("the periods are on the sheet", pt is not None)
g.on_press(g.area, Ev(*pt))
pump(0.05)
pt2 = item_point(TI, 2)
g._drag_to(*pt2)
g.on_release(g.area, Ev(*pt2))
pump(0.1)
check("dragging across Feb-Mar 2024 chooses them", b().slicer_info(PS, TI)["span"] is not None and total() == "50", (b().slicer_info(PS, TI)["span"], total()))
win.run("slicer-level:0")
check("Time Level: years", [x[0] for x in b().timeline_periods(PS, TI)] == ["2024", "2025"])
S.clear(win, TI)
check("Clear Filter on the timeline", b().slicer_info(PS, TI)["span"] is None and total() == "150", total())

# ---- a second PivotTable on the same data
DS = [k for k in range(b().sheet_count) if b().sheet_info(k)["name"] == "Sheet1"][0]
q2 = b().create_pivot(DS, (0, 0, 5, 2), PS, (20, 0))
q = b().pivot(q2)
q["rows"] = [0]
q["values"] = [{"field": 2, "agg": "sum", "name": "Sum of Sales", "show": "normal"}]
b().set_pivot(q2, q)

# ---- Report Connections: one slicer filters both PivotTables
g.sel_objects = [SI]
win.on_object_selection()
cd = S.ConnectionsDialog(win, SI)
check("Report Connections lists the PivotTables on the same data", len(cd.checks) == 2, [c.get_label() for c in cd.checks])
for c in cd.checks:
    c.set_active(True)
cd.apply()
cd.d.destroy()
pt = item_point(SI, 1)
g.on_press(g.area, Ev(*pt))
pump(0.1)
other = [tx(PS, r, 1) for r in range(20, 30) if tx(PS, r, 0) == "Grand Total"]
check("a click filters both", total() == "40" and other == ["40"], (total(), other, b().slicer_info(PS, SI), b().slicer_items(PS, SI), pt, b().pivot(1)["name"], INFO[-2:]))
S.clear(win, SI)

# ---- PivotChart
g.sel_objects = []
g.select_cell(3, 0)
win.on_selection_changed()
win.run("pivot-chart")
pump(0.1)
ci = [k for k, o in enumerate(b().objects(PS)) if o["kind"] == "chart"]
check("PivotChart inserts a chart of the PivotTable", len(ci) == 1 and b().chart_pivot(PS, ci[0]) == b().pivot(0)["name"], ci)
CI = ci[0]
spec = b().chart_spec(PS, CI)
check("its series are the PivotTable's values, without the grand total", len(spec["series"]) == 1 and spec["series"][0]["val"].endswith("$B$6"), spec["series"])
g.sel_objects = [SI]
pt = item_point(SI, 0)
g.on_press(g.area, Ev(*pt))
pump(0.1)
spec = b().chart_spec(PS, CI)
check("the chart follows the slicer", spec["series"][0]["val"].endswith("$B$4"), spec["series"])
S.clear(win, SI)
INFO.clear()
g.sel_objects = []
g.select_cell(0, 10)
win.on_selection_changed()
win.run("pivot-chart")
check("outside a PivotTable it says what to do", INFO and "PivotTable" in INFO[-1][1], INFO)

# ---- the context menu
items = []
win._popup_menu = lambda it, ev: items.extend(it)
win.object_menu(None, SI)
labels = [x[0] for x in items if x]
check("the slicer's menu", "Clear Filter" in labels and "Slicer Settings..." in labels and "Report Connections..." in labels, labels)

# ---- saved and opened
S.clear(win, SI)
pt = item_point(SI, 2)
g.sel_objects = [SI]
g.on_press(g.area, Ev(*pt))
pump(0.1)
xfile = os.path.join(tempfile.mkdtemp(), "slicers.xlsx")
win.save_to(xfile)
z = zipfile.ZipFile(xfile)
names = z.namelist()
check("the file has the slicer, timeline and cache parts",
      all(any(k in n for n in names) for k in ("slicers/slicer", "slicerCaches/slicerCache", "timelines/timeline", "timelineCaches/timelineCache")), names)
wb = z.read("xl/workbook.xml").decode()
check("the workbook lists the caches and their names", "x14:slicerCaches" in wb and "Slicer_Region" in wb and "x15:timelineCacheRefs" in wb)
again = Book.open(xfile)
kinds = [o["kind"] for o in again.objects(PS)]
check("opened: slicer, timeline and PivotChart come back", sorted(kinds) == ["chart", "slicer", "timeline"], kinds)
k = kinds.index("slicer")
check("with the slicer's choice and settings", [on for _t, on in again.slicer_items(PS, k)] == [False, False, True]
      and again.slicer_info(PS, k)["caption"] == "Where" and again.slicer_info(PS, k)["columns"] == 2
      and len(again.slicer_info(PS, k)["pivots"]) == 2, (again.slicer_items(PS, k), again.slicer_info(PS, k)))
check("the PivotChart still plots its PivotTable", again.chart_pivot(PS, kinds.index("chart")) == "PivotTable1")

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
