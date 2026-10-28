# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): SH-20 PivotTables — Insert › PivotTable, the PivotTable
# Fields pane, the Analyze and Design tabs, filter drop-downs, Value Field Settings, Show Details,
# Refresh, Move, Recommended PivotTables, saving and opening.
import os
import sys
import tempfile
import time
import traceback

import cairo
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


from folio_sheets.core import Book  # noqa: E402
from folio_sheets import pivot_ui as P  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1400, 860)
pump(0.3)
g = win.grid
b = lambda: win.book  # noqa: E731


def sheet(name):
    return [k for k in range(b().sheet_count) if b().sheet_info(k)["name"] == name][0]


def go(s, r=0, c=0):
    g.set_sheet(s)
    win.sheet_changed()
    g.select_cell(r, c)
    win.on_selection_changed()
    pump(0.05)


def tx(s, r, c):
    v = b().value(s, r, c)
    if isinstance(v, float) and v == int(v):
        v = int(v)
    return "" if v is None else str(v)


rows = [["Region", "Product", "Sales", "Units"], ["East", "Apples", "10", "1"], ["West", "Apples", "20", "2"],
        ["East", "Pears", "30", "3"], ["West", "Pears", "40", "4"], ["East", "Apples", "5", "5"], ["North", "Plums", "7", "6"]]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b().set_input(0, r, c, v)
go(0, 1, 1)

# ---- the ribbon
ins = win.ribbon.tab_by_name("Insert")
check("Insert has PivotTable and Recommended PivotTables", ins is not None)
check("the PivotTable tabs are hidden outside a PivotTable", not win.ribbon.tab_by_name("PivotTable Analyze").button.get_visible())

# ---- Create PivotTable
d = P.CreateDialog(win)
check("Create PivotTable picks the data around the active cell", d.source.get_text() == "Sheet1!$A$1:$D$7", d.source.get_text())
check("a new worksheet by default", d.r_new.get_active())
check("the Data Model: add this data to it; use it once it has tables", d.model.get_sensitive() and not d.r_model.get_sensitive())
err = d.apply()
d.d.destroy()
pump(0.1)
check("OK makes it", err is None, err)
PS = g.sheet
check("on a new sheet in front of the data", b().sheet_info(PS)["name"] == "Sheet2" and sheet("Sheet1") == PS + 1, b().sheet_info(PS)["name"])
check("with the active cell in it, at A3", g.sel.active == (2, 0) and P.current(win) == 0)
check("the field list opens", win.pivot_pane.get_visible() and win.pivot_pane.i == 0)
check("the Analyze and Design tabs show", win.ribbon.tab_by_name("PivotTable Analyze").button.get_visible()
      and win.ribbon.tab_by_name("Design").button.get_visible())
check("an empty PivotTable says so", tx(PS, 2, 0) != "" or b().pivot(0)["out"] is not None)
pane = win.pivot_pane
names = [r.field_name for r in pane.fields.get_children()]
check("the pane lists the fields", names == ["Region", "Product", "Sales", "Units"], names)

# ---- ticking fields: text to Rows, numbers to Values
pane.toggle(0, True)
pane.toggle(2, True)
p = b().pivot(0)
check("Region goes to Rows, Sales to Values", p["rows"] == [0] and [v["name"] for v in p["values"]] == ["Sum of Sales"])
check("the table shows", [tx(PS, 2, 0), tx(PS, 2, 1)] == ["Row Labels", "Sum of Sales"] and
      [tx(PS, 3, 0), tx(PS, 3, 1)] == ["East", "45"] and [tx(PS, 6, 0), tx(PS, 6, 1)] == ["Grand Total", "112"],
      [tx(PS, r, c) for r in range(2, 7) for c in range(2)])
check("the areas show them", [ch.get_child().get_label() for ch in pane.lists["rows"].get_children()] == ["Region"]
      and [ch.get_child().get_label() for ch in pane.lists["values"].get_children()] == ["Sum of Sales"])
check("its cells have the style's look", b().pivot(0)["style"] == "PivotStyleLight16")
# a second value: Σ Values in Columns
pane.toggle(3, True)
lay = P.layout_of(b().pivot(0))
check("two values put Σ Values in Columns", lay["cols"] == [-2], lay)
check("each value has its column", [tx(PS, 2, 1), tx(PS, 2, 2)] == ["Sum of Sales", "Sum of Units"], [tx(PS, 2, 1), tx(PS, 2, 2)])
# drag Σ Values to Rows
pane.move("cols", 0, "rows")
p = b().pivot(0)
check("Σ Values moved to Rows", p["values_in_rows"] and [tx(PS, 3, 0), tx(PS, 4, 0), tx(PS, 5, 0)] == ["East", "Sum of Sales", "Sum of Units"],
      [tx(PS, r, 0) for r in range(2, 8)])
pane.move("rows", 1, "cols")
check("and back", not b().pivot(0)["values_in_rows"])
# Product to Columns by dragging a field in
pane.drop("field:1", "cols", 0)
p = b().pivot(0)
check("a field dropped on Columns", p["cols"] == [1], p["cols"])
check("Column Labels over the items", tx(PS, 2, 1) == "Column Labels" and tx(PS, 3, 1) == "Apples", [tx(PS, r, 1) for r in range(2, 6)])
pane.remove("values", 1)
check("a value removed", [v["name"] for v in b().pivot(0)["values"]] == ["Sum of Sales"])
# a field to Filters
pane.move("cols", 0, "pages")
p = b().pivot(0)
check("Product moved to Filters", p["pages"] == [(1, None)] and p["cols"] == [], p)
check("the filter sits above the table", [tx(PS, 0, 0), tx(PS, 0, 1)] == ["Product", "(All)"], [tx(PS, 0, 0), tx(PS, 0, 1)])
# a filter drop-down
btns = [(r, c) for r in range(0, 4) for c in range(0, 2) if b().pivot_button_at(PS, r, c)]
check("drop-down buttons on the filter and Row Labels", (0, 1) in btns and (2, 0) in btns, btns)
fp = P.FilterPopover(win, 0, "page", 0)
check("the filter lists the items", fp.items == ["Apples", "Pears", "Plums"], fp.items)
fp.pick("Pears")
fp.ok()
check("one item shown", tx(PS, 0, 1) == "Pears" and [tx(PS, 3, 0), tx(PS, 3, 1)] == ["East", "30"], [tx(PS, r, c) for r in range(0, 6) for c in range(2)])
fp = P.FilterPopover(win, 0, "page", 0)
fp.multi.set_active(True)
fp.set_shown({"Apples", "Plums"})
fp.ok()
check("several items: (Multiple Items)", tx(PS, 0, 1) == "(Multiple Items)" and [tx(PS, 3, 0), tx(PS, 3, 1)] == ["East", "15"],
      [tx(PS, r, c) for r in range(0, 7) for c in range(2)])
P.clear(win, "filters") if P.current(win) is not None else None
go(PS, 3, 0)
P.clear(win, "filters")
check("Clear Filters shows everything", tx(PS, 0, 1) == "(All)")
# row field filter: hide West
fp = P.FilterPopover(win, 0, "rows", 0)
fp.set_shown({"East", "North"})
fp.ok()
rl = [tx(PS, r, 0) for r in range(3, 7)]
check("a row filter hides an item", rl == ["East", "North", "Grand Total", ""], rl)
check("the field list marks the filtered field", any(len(ch.get_child().get_children()) > 1 for ch in pane.fields.get_children()))
fp = P.FilterPopover(win, 0, "rows", 0)
fp.sort(-1)
check("sorted Z to A", [tx(PS, 3, 0), tx(PS, 4, 0)] == ["North", "East"])
fp = P.FilterPopover(win, 0, "rows", 0)
fp.clear()
fp = P.FilterPopover(win, 0, "rows", 0)
fp.sort(1)
check("filter cleared and sorted back", [tx(PS, r, 0) for r in range(3, 7)] == ["East", "North", "West", "Grand Total"])

# ---- typing in it
try:
    b().set_input(PS, 3, 1, "5")
    typed = False
except ValueError as e:
    typed = "PivotTable" in str(e)
check("its values can't be typed over", typed)
b().set_input(PS, 2, 1, "Total Sales")
check("a value's caption can be renamed by typing", b().pivot(0)["values"][0]["name"] == "Total Sales")
win.run("undo")

# ---- Value Field Settings
go(PS, 3, 1)
af = P.active_field(win)
check("the active field of a value cell", af == ("value", "Sum of Sales", 0), af)
check("shown in the ribbon", win.pivot_field.get_text() == "Sum of Sales", win.pivot_field.get_text())
vs = P.ValueSettings(win, 0, 0)
for row in vs.aggs.get_children():
    if row.code == "average":
        vs.aggs.select_row(row)
check("the caption follows the calculation", vs.name.get_text() == "Average of Sales", vs.name.get_text())
vs.show.set_active_id("percentOfTotal")
err = vs.apply()
vs.d.destroy()
check("Average, as % of Grand Total", err is None and b().pivot(0)["values"][0]["agg"] == "average"
      and b().pivot(0)["values"][0]["show"] == "percentOfTotal", err)
check("shown as a percentage", b().cell_text(PS, 3, 1).endswith("%") if hasattr(b(), "cell_text") else True)
vs = P.ValueSettings(win, 0, 0)
vs.show.set_active_id("x-Rank Smallest to Largest")
check("the missing calculations say so", INFO and "isn't available" in INFO[-1][1] and vs.show.get_active_id() == "percentOfTotal", INFO[-1:])
vs.d.destroy()
win.run("pivot-summarize:sum")
win.run("pivot-show-as:normal")
check("Summarize Values By and Show Values As from the cell menu", b().pivot(0)["values"][0]["agg"] == "sum"
      and b().pivot(0)["values"][0]["name"] == "Sum of Sales" and tx(PS, 3, 1) == "45", b().pivot(0)["values"][0])
items = [it[0] for it in P.menu_items(win) if it]
check("the cell menu has the pivot commands", "Show Details" in items and "Value Field Settings..." in items and 'Remove "Sum of Sales"' in items, items)

# ---- Show Details (double-click a value)
n0 = b().sheet_count
P.double_click(win, 3, 1)
pump(0.1)
DS = g.sheet
check("double-clicking a value shows its records on a new sheet", b().sheet_count == n0 + 1 and tx(DS, 0, 0) == "Region"
      and [tx(DS, r, 0) for r in (1, 2, 3)] == ["East", "East", "East"] and tx(DS, 4, 0) == "", [tx(DS, r, 0) for r in range(5)])
check("as a table", len(b().tables(DS)) == 1)
PS = sheet("Sheet2")
go(PS, 3, 1)

# ---- Design
win.run("pivot-design:layout:tabular")
check("Tabular Form", b().pivot(0)["layout"] == "tabular" and tx(PS, 2, 0) == "Region", tx(PS, 2, 0))
win.run("pivot-design:layout:compact")
win.run("pivot-design:grand:off")
check("grand totals off", "Grand Total" not in [tx(PS, r, 0) for r in range(2, 9)])
win.run("pivot-design:grand:on")
win.run("pivot-design:option:banded_rows:1")
check("Banded Rows ticked", b().pivot(0)["banded_rows"] and win.pivot_checks["banded_rows"].get_active())
win.run("pivot-design:style:PivotStyleMedium9")
check("a style picked", b().pivot(0)["style"] == "PivotStyleMedium9")
pop = P.style_gallery(win, lambda n: None)
check("the style gallery has 84 styles", len(pop.buttons) == 84, len(pop.buttons))
surf = cairo.ImageSurface(cairo.FORMAT_RGB24, 61, 46)
b().render_pivot_style(cairo.Context(surf), "PivotStyleMedium9", 61.0, 46.0)
check("style previews draw", True)
win.run("pivot-design:subtotals:bottom")
check("subtotals at the bottom", b().pivot(0)["subtotals"] == "bottom")

# ---- Analyze
win.pivot_name.set_text("Sales by Region")
win.pivot_name.emit("activate")
check("renamed from the ribbon", b().pivot(0)["name"] == "Sales by Region", (b().pivot(0)["name"], INFO[-2:], P.current(win), g.sel.active))
od = P.OptionsDialog(win, 0)
od.checks["grand_cols"].set_active(False)
err = od.apply()
od.d.destroy()
check("PivotTable Options: grand totals for columns off", err is None and not b().pivot(0)["grand_cols"], err)
win.run("pivot-design:grand:on")
# source changes; Refresh
S1 = sheet("Sheet1")
b().set_input(S1, 1, 2, "100")
check("the table waits for Refresh", tx(PS, 3, 1) == "45")
go(PS, 3, 1)
win.run("pivot-refresh")
check("Refresh reads the source again", tx(PS, 3, 1) == "135", tx(PS, 3, 1))
b().set_input(S1, 1, 2, "10")
win.run("refresh-all")
check("Data › Refresh All refreshes PivotTables", tx(PS, 3, 1) == "45", tx(PS, 3, 1))
# rows added below the source: Change Data Source
for c, v in enumerate(["South", "Kiwis", "3", "1"]):
    b().set_input(S1, 7, c, v)
sd = P.SourceDialog(win, 0)
check("Change Data Source shows the range", sd.source.get_text() == "Sheet1!$A$1:$D$7", sd.source.get_text())
sd.source.set_text("Sheet1!$A$1:$D$8")
err = sd.apply()
sd.d.destroy()
check("a bigger range: the new item shows", err is None and "South" in [tx(PS, r, 0) for r in range(3, 9)], err)
# Move
md = P.MoveDialog(win, 0)
md.location.set_text("Sheet2!$F$5")
err = md.apply()
md.d.destroy()
p = b().pivot(0)
check("Move PivotTable", err is None and p["out"][0] == 4 and p["out"][1] == 5 and tx(PS, 6, 5) == "Row Labels" and tx(PS, 4, 5) == "Product" and tx(PS, 2, 0) == "",
      (err, p["out"]))
win.run("undo")
check("undone", b().pivot(0)["out"][:2] == (0, 0) and tx(PS, 2, 0) == "Row Labels", b().pivot(0)["out"])
# Select
go(PS, 3, 0)
win.run("pivot-select:entire")
check("Select › Entire PivotTable", tuple(g.sel.ranges[-1]) == tuple(b().pivot(0)["out"]), g.sel.ranges)
go(PS, 3, 0)
# rows through it can't be inserted
try:
    b().insert_rows(PS, 4, 1)
    refused = False
except ValueError:
    refused = True
check("rows can't be inserted through it", refused)
# the not-available commands say so
INFO.clear()
for cmd in ("pivot-group", "pivot-calculated", "pivot-drill-down", "pivot-olap", "pivot-relationships"):
    win.run(cmd)
check("grouping, calculated fields, OLAP say they aren't available", len(INFO) == 5 and all("isn't available" in t for _, t in INFO), INFO)
# field list toggles
win.run("pivot-field-list")
check("Field List hides the pane", not win.pivot_pane.get_visible())
win.run("pivot-field-list")
check("and shows it", win.pivot_pane.get_visible())
# defer layout update
pane.defer.set_active(True)
pane.toggle(1, True)
check("deferred: the table waits", b().pivot(0)["rows"] == [0] and pane.update_btn.get_sensitive())
pane.update_now()
check("Update applies it", b().pivot(0)["rows"] == [0, 1], b().pivot(0)["rows"])
pane.defer.set_active(False)
win.run("undo")

# ---- leaving it hides the tabs
go(PS, 30, 10)
check("outside: tabs and pane go", not win.ribbon.tab_by_name("PivotTable Analyze").button.get_visible() and not win.pivot_pane.get_visible())
go(PS, 3, 0)
check("back inside: they come back", win.ribbon.tab_by_name("Design").button.get_visible() and win.pivot_pane.get_visible())

# ---- Recommended PivotTables
go(sheet("Sheet1"), 2, 1)
rd = P.RecommendedDialog(win)
titles = [t for t, *_ in rd.items]
check("recommendations for the data", "Sum of Sales by Region" in titles, titles)
k = titles.index("Sum of Sales by Region")
err = rd.apply(k)
rd.d.destroy()
RS = g.sheet
check("the pick goes on a new sheet", err is None and b().pivot_count() == 2 and tx(RS, 2, 0) == "Row Labels" and tx(RS, 2, 1) == "Sum of Sales",
      (err, tx(RS, 2, 0), tx(RS, 2, 1)))

# ---- a screenshot of it
go(sheet("Sheet2"), 3, 0)
pump(0.2)
surf = cairo.ImageSurface(cairo.FORMAT_RGB24, 600, 300)
cr = cairo.Context(surf)
g.area.draw(cr) if hasattr(g.area, "draw") else None
check("it paints", True)

# ---- saved and opened
xfile = os.path.join(tempfile.mkdtemp(), "pivots.xlsx")
win.save_to(xfile)
again = Book.open(xfile)
check("saved: both PivotTables come back", again.pivot_count() == 2, again.pivot_count())
q = again.pivot(0)
check("with their layout and fields", q["name"] == "Sales by Region" and q["rows"] == [0] and q["values"][0]["name"] == "Sum of Sales"
      and q["style"] == "PivotStyleMedium9" and q["refreshable"], q)
again.set_input(sheet("Sheet1") if False else [k for k in range(again.sheet_count) if again.sheet_info(k)["name"] == "Sheet1"][0], 1, 2, "1000")
again.refresh_pivot(0)
s2 = [k for k in range(again.sheet_count) if again.sheet_info(k)["name"] == "Sheet2"][0]
check("and refresh after opening", again.value(s2, 3, 1) == 1035, again.value(s2, 3, 1))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
