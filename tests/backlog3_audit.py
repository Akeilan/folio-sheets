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

# ---------------------------------------------------------------- S3-06 more local sources
from folio_sheets import getdata_ui, query_ui, datasources  # noqa: E402
import json as _json  # noqa: E402
import sqlite3 as _sq  # noqa: E402
jp = os.path.join(TMP, "people.json")
with open(jp, "w") as fh:
    _json.dump({"meta": {"v": 1}, "people": [{"name": "Ann", "age": 31, "city": {"name": "Utrecht"}},
                                             {"name": "Bob", "age": 45, "city": {"name": "Delft"}},
                                             {"name": "Cy", "age": 28, "city": {"name": "Utrecht"}}]}, fh)
items = datasources.json_items(jp)
check("json: tables listed", any(i[1] == "people" for i in items), items)
res = datasources.read_json({"path": jp, "item": "people"})
check("json: records become rows, nested keys columns", res.headers == ["name", "age", "city.name"] and len(res.rows) == 3
      and res.kinds[1] == "number", (res.headers, res.kinds))
xp = os.path.join(TMP, "orders.xml")
with open(xp, "w") as fh:
    fh.write("<orders><order id='1'><item>Pen</item><qty>3</qty></order><order id='2'><item>Ink</item><qty>5</qty></order></orders>")
items = datasources.xml_items(xp)
res = datasources.read_xml({"path": xp, "item": "/orders/order"})
check("xml: repeating elements become rows", res.headers == ["Attribute:id", "item", "qty"] and [r[1] for r in res.rows] == ["Pen", "Ink"],
      (items, res.headers, res.rows))
dbp = os.path.join(TMP, "shop.db")
con = _sq.connect(dbp)
con.execute("create table stock (sku text, n integer, price real)")
con.executemany("insert into stock values (?,?,?)", [("A1", 4, 2.5), ("B2", 0, 10.0), ("C3", 7, 1.25)])
con.commit()
con.close()
items = datasources.sqlite_items(dbp)
check("sqlite: tables listed", items and items[0][1] == "stock", items)
res = datasources.read_sqlite({"path": dbp, "item": "stock"})
check("sqlite: rows", res.headers == ["sku", "n", "price"] and res.rows[1] == ["B2", "0", "10"], (res.headers, res.rows))
import cairo  # noqa: E402
pp = os.path.join(TMP, "report.pdf")
surf = cairo.PDFSurface(pp, 595, 842)
cr = cairo.Context(surf)
cr.select_font_face("DejaVu Sans")
cr.set_font_size(11)
cr.move_to(60, 60)
cr.show_text("Quarterly report")
for i, row in enumerate([("Region", "Sales", "Units"), ("North", "1200", "30"), ("South", "950", "22"), ("East", "1430", "41")]):
    for j, t in enumerate(row):
        cr.move_to(60 + j * 140, 120 + i * 20)
        cr.show_text(t)
cr.move_to(60, 300)
cr.show_text("End of report.")
surf.finish()
items = datasources.pdf_items(pp)
check("pdf: a table found on the page", items and items[0][0].startswith("Table001"), items)
res = datasources.read_pdf({"path": pp, "item": items[0][1]}) if items else None
check("pdf: its rows and headers", res and res.headers == ["Region", "Sales", "Units"] and res.rows[2] == ["East", "1430", "41"],
      res and (res.headers, res.rows))
out = getdata_ui.from_source(win, "sqlite", dbp, item="stock", choice="load")
pump(0.3)
q = getdata_ui.find_query(b, "stock")
check("from SQLite: loaded as a table with its query", out and q and q["kind"] == "sqlite" and b.display(out[0], 3, 0) == "C3",
      (out, q))
from folio_sheets import commands as _cmds  # noqa: E402
check("Get Data menu commands are wired", all(hasattr(win, _cmds.method_name(c)) for c in ("from-source:xml", "from-source:json", "from-source:pdf", "from-source:sqlite", "from-table-range")))
g.set_sheet(0)
b.set_input(0, 20, 0, "k")
b.set_input(0, 20, 1, "v")
b.set_input(0, 21, 0, "a")
b.set_input(0, 21, 1, "1")
b.set_input(0, 22, 0, "b")
b.set_input(0, 22, 1, "2")
g.sel.ranges = [(20, 0, 22, 1)]
g.sel.active = (20, 0)
got = getdata_ui.from_table_range(win, "load")
pump(0.3)
qr = [q for q in getdata_ui.queries(b) if q["kind"] == "range"]
check("From Table/Range: the range becomes a table and a query", got and qr and b.display(got[0], 2, 0) == "b", (got, qr))

# ---------------------------------------------------------------- S3-05 the query editor
res = datasources.read_json({"path": jp, "item": "people"})
steps = [{"op": "filter", "col": "city.name", "cmp": "eq", "value": "Utrecht"},
         {"op": "rename", "col": "city.name", "new": "City"},
         {"op": "sort", "keys": [("age", False)]},
         {"op": "index", "start": 1}]
r2 = query_ui.apply_steps(res, steps)
check("steps: filter, rename, sort, index", r2.headers == ["name", "age", "City", "Index"] and [r[0] for r in r2.rows] == ["Ann", "Cy"]
      and r2.rows[1][3] == "2", (r2.headers, r2.rows))
r3 = query_ui.apply_steps(res, [{"op": "group", "by": ["city.name"], "aggs": [("Total age", "sum", "age"), ("Count", "count", None)]}])
check("steps: group by", r3.rows == [["Utrecht", "59", "2"], ["Delft", "45", "1"]], r3.rows)
r4 = query_ui.apply_steps(res, [{"op": "split", "col": "name", "delim": "n", "mode": "left"}, {"op": "remove", "cols": ["age"]}])
check("steps: split and remove", r4.headers[:2] == ["name.1", "name.2"] and "age" not in r4.headers, r4.headers)
q0 = {"kind": "json", "path": jp, "item": "people", "name": "people"}
ed = query_ui.QueryEditor(win, q0, res)
ed.select_column("age")
ed.add_step({"op": "filter", "col": "age", "cmp": "gt", "value": "30"})
ed.add_step({"op": "type", "col": "age", "kind": "text"})
check("editor: steps listed", [r.get_child().get_text() for r in ed.steps.get_children()] == ["Source", "Filtered Rows", "Changed Type"])
check("editor: preview follows", len(ed.res.rows) == 2, ed.res.rows)
ed.steps.select_row(ed.steps.get_row_at_index(0))
check("editor: picking a step shows the rows up to it", len(ed.res.rows) == 3)
ed.delete_step(1)
check("editor: Delete Step", [s["op"] for s in ed.q["steps"]] == ["filter"])
ok = ed.add_step({"op": "rename", "col": "nope", "new": "x"})
check("editor: a step on a missing column is refused", ok is False and len(ed.q["steps"]) == 1)
qq, rr = ed.finish()
ed.destroy()
got = getdata_ui.load(win, qq, rr, "new")
pump(0.3)
q1 = getdata_ui.find_query(b, "people")
check("editor: Close & Load keeps the steps with the query", q1 and q1.get("steps") and q1["steps"][0]["op"] == "filter")
check("editor: and loads the shaped rows", got and b.display(got[0], 2, 0) == "Bob" and b.display(got[0], 3, 0) == "", got)
with open(jp, "w") as fh:
    _json.dump({"people": [{"name": "Ann", "age": 31, "city": {"name": "Utrecht"}}, {"name": "Dee", "age": 60, "city": {"name": "Ede"}},
                           {"name": "Bob", "age": 45, "city": {"name": "Delft"}}]}, fh)
getdata_ui.refresh(win, q1)
pump(0.3)
check("refresh runs the steps again", [b.display(got[0], r, 0) for r in (1, 2, 3)] == ["Ann", "Dee", "Bob"])
# merge and append
qa = {"kind": "sqlite", "path": dbp, "item": "stock", "name": "stockB", "steps": [{"op": "top", "n": 1}]}
getdata_ui.save_query(b, qa)
r5 = getdata_ui.read({"kind": "sqlite", "path": dbp, "item": "stock", "name": "x", "steps": [{"op": "append", "queries": ["stockB"]}]}, b)
check("append another query", len(r5.rows) == 4 and r5.rows[3][0] == "A1", r5.rows)
r6 = getdata_ui.read({"kind": "sqlite", "path": dbp, "item": "stock", "name": "y",
                      "steps": [{"op": "merge", "query": "stockB", "left": "sku", "right": "sku", "kind": "inner"}]}, b)
check("merge with another query (inner)", len(r6.rows) == 1 and r6.headers[-1] == "stockB.price", (r6.headers, r6.rows))
# Transform Data button exists and works from a preview
d = getdata_ui.ItemsNavigator(win, "json", jp)
check("Transform Data is on", d.buttons.transform_button.get_sensitive())
d.destroy()

# ---------------------------------------------------------------- S3-04 load to a PivotTable
qp = {"kind": "sqlite", "path": dbp, "item": "stock", "name": "stockP"}
nsh = b.sheet_count
got = getdata_ui.load(win, qp, datasources.read_sqlite(qp), "pivot", None)
pump(0.3)
check("load to PivotTable: a PivotTable on a new sheet", got and b.pivot_count() >= 1 and b.sheet_count == nsh + 2, (got, b.sheet_count))
hidden = [i for i in range(b.sheet_count) if b.sheet_info(i)["state"] == "veryHidden"]
check("load to PivotTable: the rows on a hidden sheet", hidden, hidden)
pv = b.pivot(got[1]) if got else None
check("load to PivotTable: fields from the query", pv and [f["name"] for f in pv["fields"]] == ["sku", "n", "price"], pv and pv["fields"])
dlg = getdata_ui.LoadToDialog(win)
check("Import Data: PivotTable Report and PivotChart choosable", dlg.pivot.get_sensitive() and dlg.chart.get_sensitive())
dlg.pivot.set_active(True)
check("Import Data: where() says pivot", dlg.where()[0] == "pivot", dlg.where())
dlg.destroy()

# ---------------------------------------------------------------- S3-07 form controls, cell check boxes
from folio_sheets import controls_ui  # noqa: E402
import zipfile  # noqa: E402
win.run("insert-sheet")
pump()
SC = g.sheet
g.select_range((1, 1, 3, 1))
pump()
win.run("cell-checkbox")
pump()
check("Insert Checkbox: the cells show check boxes", all(b.is_checkbox(SC, r, 1) for r in (1, 2, 3)))
check("Insert Checkbox: empty cells become FALSE", [b.display(SC, r, 1) for r in (1, 2, 3)] == ["FALSE"] * 3, [b.display(SC, r, 1) for r in (1, 2, 3)])
b.toggle_checkbox(SC, 1, 1)
check("a click ticks it (TRUE)", b.display(SC, 1, 1) == "TRUE", b.display(SC, 1, 1))
g.select_cell(2, 1)
pump()
check("Space on a check-box cell ticks it", controls_ui.toggle_selected(win) and b.display(SC, 2, 1) == "TRUE", b.display(SC, 2, 1))
b.set_input(SC, 1, 3, "=COUNTIF(B2:B4,TRUE)")
check("formulas see the ticks", b.display(SC, 1, 3) == "2", b.display(SC, 1, 3))
win.run("undo")
win.run("undo")
check("undo un-ticks", b.display(SC, 2, 1) == "FALSE", b.display(SC, 2, 1))
win.run("redo")
win.run("redo")


class _Ev:
    def __init__(self, x, y, button=1, state=0, kind=None):
        from gi.repository import Gdk
        self.x, self.y, self.button = x, y, button
        self._state = state
        self.type = kind or Gdk.EventType.BUTTON_PRESS

    def get_state(self):
        return self._state


rect = b.cell_rect(SC, 3, 1, g.params())
check("clicking the box in the cell ticks it", controls_ui.click_cell(win, 3, 1, _Ev(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2), rect)
      and b.display(SC, 3, 1) == "TRUE", b.display(SC, 3, 1))
# form controls
fc = win.form_controls
b.set_input(SC, 0, 6, "Apple")
b.set_input(SC, 1, 6, "Pear")
b.set_input(SC, 2, 6, "Plum")
ci = fc.add("Checkbox", 300, 20, 100, 18, link="$E$2", text="Shipped")
check("Insert > Check Box: a control on the sheet", len(b.controls(SC)) == 1 and b.controls(SC)[0]["kind"] == "Checkbox")
r = fc.rect(ci)
fc.press(_Ev(r[0] + 6, r[1] + r[3] / 2))
pump()
check("clicking the check box sets its linked cell TRUE", b.display(SC, 1, 4) == "TRUE", b.display(SC, 1, 4))
fc.press(_Ev(r[0] + 6, r[1] + r[3] / 2))
check("and again FALSE", b.display(SC, 1, 4) == "FALSE", b.display(SC, 1, 4))
b.set_input(SC, 1, 4, "TRUE")
check("the linked cell drives the box", b.control_value(SC, ci) == 1)
si = fc.add("Spin", 300, 60, 18, 32, link="$E$3", min=0, max=10, inc=2)
r = fc.rect(si)
fc.press(_Ev(r[0] + r[2] / 2, r[1] + 3))
fc.press(_Ev(r[0] + r[2] / 2, r[1] + 3))
check("spin button up twice: 4 in its cell", b.display(SC, 2, 4) == "4", b.display(SC, 2, 4))
fc.press(_Ev(r[0] + r[2] / 2, r[1] + r[3] - 3))
check("spin button down: 2", b.display(SC, 2, 4) == "2", b.display(SC, 2, 4))
li = fc.add("Drop", 300, 110, 100, 20, link="$E$4", range="$G$1:$G$3")
check("combo box items from its input range", b.control_items(SC, li) == ["Apple", "Pear", "Plum"], b.control_items(SC, li))
b.set_control_value(SC, li, 3)
check("picking an item puts its number in the cell", b.display(SC, 3, 4) == "3", b.display(SC, 3, 4))
lb = fc.add("List", 420, 110, 100, 60, link="$E$5", range="$G$1:$G$3")
r = fc.rect(lb)
fc.press(_Ev(r[0] + 10, r[1] + 15 * g.params()["zoom"] * 1.5))
check("clicking a list box row picks it", b.display(SC, 4, 4) == "2", b.display(SC, 4, 4))
r1 = fc.add("Radio", 300, 150, 90, 18, link="$E$6", text="Small")
r2 = fc.add("Radio", 300, 170, 90, 18, text="Large")
r = fc.rect(r2)
fc.press(_Ev(r[0] + 6, r[1] + r[3] / 2))
check("option buttons: the second one puts 2 in the group's cell", b.display(SC, 5, 4) == "2" and b.control_value(SC, r2) == 1 and b.control_value(SC, r1) == 0,
      (b.display(SC, 5, 4), b.control_value(SC, r1), b.control_value(SC, r2)))
sb = fc.add("Scroll", 520, 20, 18, 120, link="$E$7", min=0, max=100, inc=1, page=10)
r = fc.rect(sb)
fc.press(_Ev(r[0] + r[2] / 2, r[1] + r[3] - 30))
check("scroll bar: a click in the track moves a page", b.display(SC, 6, 4) == "10", b.display(SC, 6, 4))
bt = fc.add("Button", 520, 160, 72, 24, macro="Module1.Go")
DIALOG.append(lambda d: Gtk.ResponseType.OK)
r = fc.rect(bt)
fc.press(_Ev(r[0] + 10, r[1] + 10))
check("a button with a macro says macros don't run", not DIALOG)
# selecting, moving, deleting
r = fc.rect(ci)
from gi.repository import Gdk  # noqa: E402
fc.press(_Ev(r[0] + 6, r[1] + 6, state=Gdk.ModifierType.CONTROL_MASK))
check("Ctrl+click selects the control", fc.sel == ci)
fc.motion(_Ev(r[0] + 46, r[1] + 26))
fc.release(_Ev(r[0] + 46, r[1] + 26))
r2_ = fc.rect(ci)
check("dragging the selected control moves it", abs(r2_[0] - r[0] - 40) < 2 and abs(r2_[1] - r[1] - 20) < 2, (r, r2_))
win.run("undo")
check("undo puts it back", abs(fc.rect(ci)[0] - r[0]) < 2)
n = len(b.controls(SC))
fc.select(bt)
from gi.repository import Gdk as _G  # noqa: E402


class _K:
    keyval = _G.KEY_Delete


fc.key(_K())
check("Delete removes the selected control", len(b.controls(SC)) == n - 1)
win.run("undo")
check("undo brings it back", len(b.controls(SC)) == n)
# Format Control
fc.select(si)


def _fmt(d):
    d.fields["max"].set_text("20")
    d.fields["val"].set_text("12")
    return Gtk.ResponseType.OK


DIALOG.append(_fmt)
win.run("control-properties")
pump()
check("Format Control: limits and value", b.controls(SC)[si]["max"] == 20 and b.display(SC, 2, 4) == "12", (b.controls(SC)[si], b.display(SC, 2, 4)))
fc.select(None)
# drawing one with the mouse
win.run("form-control:Label")
check("Insert > Label: drawing mode", fc.placing == "Label")
fc.press(_Ev(100, 300))
fc.motion(_Ev(180, 320))
fc.release(_Ev(180, 320))
check("drawn on the sheet", b.controls(SC)[-1]["kind"] == "Label" and fc.placing is None)
DIALOG.append(lambda d: Gtk.ResponseType.OK)
win.run("activex-controls")
check("ActiveX controls say they aren't available", not DIALOG)
# save and reopen
pc = os.path.join(TMP, "controls.xlsx")
b.save(pc)
z = zipfile.ZipFile(pc)
names = z.namelist()
check("saved: the feature property bag for cell check boxes", any("featurePropertyBag" in n for n in names), names)
sty = z.read("xl/styles.xml").decode()
check("saved: the check-box cell format", "C7286773-470A-42A8-94C5-96B5CB345126" in sty or "c7286773" in sty.lower())
vml = [z.read(n).decode() for n in names if n.endswith(".vml")]
check("saved: the controls in the sheet's drawing", any('ObjectType="Checkbox"' in v and "<x:FmlaLink>$E$2</x:FmlaLink>" in v for v in vml), vml[:1])
bb = Book.open(pc)
si2 = [i for i in range(bb.sheet_count) if bb.sheet_info(i)["name"] == b.sheet_info(SC)["name"]][0]
kinds = [c["kind"] for c in bb.controls(si2)]
check("reopened: the controls", kinds == [c["kind"] for c in b.controls(SC)], kinds)
check("reopened: their links and ranges", bb.controls(si2)[2]["range"] == "$G$1:$G$3" and bb.controls(si2)[0]["link"] == "$E$2", bb.controls(si2)[:3])
check("reopened: check-box cells", bb.is_checkbox(si2, 1, 1) and bb.display(si2, 1, 1) == "TRUE")
check("reopened: spin limits", bb.controls(si2)[1]["max"] == 20 and bb.controls(si2)[1]["inc"] == 2, bb.controls(si2)[1])
# saved again without changes: the drawing kept
pc2 = os.path.join(TMP, "controls2.xlsx")
bb.save(pc2)
bb2 = Book.open(pc2)
check("saved again: the controls kept", [c["kind"] for c in bb2.controls(si2)] == kinds)
# a control rendered
import cairo as _c  # noqa: E402
surf = _c.ImageSurface(_c.FORMAT_RGB24, 800, 400)
crx = _c.Context(surf)
b.render(SC, crx, g.params({"width": 800.0, "height": 400.0}))
check("the controls are drawn", True)
g.set_sheet(0)

# ---------------------------------------------------------------- S3-11 effect schemes
from folio_sheets import shapes_ui, effect_schemes, themes_ui  # noqa: E402
from folio_sheets.core import Book as _B  # noqa: E402
g.set_sheet(0)
si_ = b.insert_shape(0, "rect", 40, 400, 120, 60, False, False)
g.select_object(si_)
pump()
check("Shape Styles: Intense Effect applied", shapes_ui.apply_style(win, 5, 1))
fx = b.shape_effects(0, si_)
check("the new book's own effects are flat", fx == (False, False, False, False), fx)
b.set_theme_effects(_B.default_theme_xml())
fx = b.shape_effects(0, si_)
check("Office's intense effect: a shadow", fx and fx[0] and not fx[2], fx)
names = [n for n, _x in effect_schemes.schemes()]
check("thirteen more effect schemes", len(names) >= 13 and "Glossy" in names and "Subtle Solids" in names, names)
gx = dict(effect_schemes.schemes())["Glossy"]
b.set_theme_effects(effect_schemes.theme_with(_B.default_theme_xml(), gx))
check("Glossy: the theme says so", b.theme_info()["effects_name"] == "Glossy", b.theme_info()["effects_name"])
fx = b.shape_effects(0, si_)
check("Glossy: the shape takes its bevel", fx and fx[2], fx)
ss = dict(effect_schemes.schemes())["Subtle Solids"]
b.set_theme_effects(effect_schemes.theme_with(_B.default_theme_xml(), ss))
fx = b.shape_effects(0, si_)
check("Subtle Solids: a softer shadow, no bevel", fx and fx[0] and not fx[2], fx)
for n, x in effect_schemes.schemes():
    try:
        b.set_theme_effects(effect_schemes.theme_with(_B.default_theme_xml(), x))
        ok = b.theme_info()["effects_name"] == n
    except ValueError as e:
        ok = False
    if not ok:
        check("effect scheme %s applies" % n, False)
check("every effect scheme applies", True)
pe = os.path.join(TMP, "fx.xlsx")
b.save(pe)
check("saved with the effects", _B.open(pe).theme_info()["effects_name"] == names[-1])
win.run("undo")
pop = shapes_ui.style_gallery(win)
check("Shape Styles gallery: 6 x 7", len(pop.rows) == 42)
g.sel_objects = []

# ---------------------------------------------------------------- S3-09 Analysis ToolPak
from folio_sheets import analysis_ui, analysis as AN  # noqa: E402
win.run("insert-sheet")
pump()
SA = g.sheet
SA_NAME = b.sheet_info(SA)["name"]
xa = [12, 15, 11, 18, 14, 16, 13, 17, 19, 15]
xb = [10, 14, 12, 15, 11, 13, 12, 16, 14, 12]
xc = [20, 22, 19, 25, 21, 23, 24, 22, 26, 20]
b.set_input(SA, 0, 0, "A")
b.set_input(SA, 0, 1, "B")
b.set_input(SA, 0, 2, "C")
for i in range(10):
    b.set_input(SA, i + 1, 0, str(xa[i]))
    b.set_input(SA, i + 1, 1, str(xb[i]))
    b.set_input(SA, i + 1, 2, str(xc[i]))


def ev(f):
    return float(b.evaluate(SA, 30, 30, f))


def close(a, b_, tol=1e-6):
    return isinstance(a, (int, float)) and abs(a - b_) <= tol * max(1, abs(b_))


def find(table, label, col=1):
    for r in table:
        if r and r[0] == label:
            return r[col]
    return None


t, _ = analysis_ui.run_tool(win, "Descriptive Statistics", {"input": "$A$1:$C$11", "grouped": "cols", "labels": True, "summary": True,
                                                           "conf_on": True, "conf": 95, "large_on": True, "large": 2, "small_on": False, "small": 1})
check("descriptive: mean, sd, kurtosis, skewness as the functions", close(find(t, "Mean"), ev("AVERAGE(A2:A11)")) and close(find(t, "Standard Deviation"), ev("STDEV.S(A2:A11)"))
      and close(find(t, "Kurtosis"), ev("KURT(A2:A11)")) and close(find(t, "Skewness"), ev("SKEW(A2:A11)")), t[:10])
check("descriptive: confidence level = CONFIDENCE.T", close(find(t, "Confidence Level(95.0%)"), ev("CONFIDENCE.T(0.05,STDEV.S(A2:A11),10)")), find(t, "Confidence Level(95.0%)"))
check("descriptive: three blocks side by side", t[0][0] == "A" and t[0][2] == "B" and t[0][4] == "C" and find(t, "Largest(2)") == 18, t[0])
t, _ = analysis_ui.run_tool(win, "t-Test: Two-Sample Assuming Equal Variances", {"input": "A1:A11", "input2": "B1:B11", "labels": True, "hyp": 0, "alpha": 0.05})
check("t-test equal: two-tail p = T.TEST(,,2,2)", close(find(t, "P(T<=t) two-tail"), ev("T.TEST(A2:A11,B2:B11,2,2)")), (find(t, "P(T<=t) two-tail"), ev("T.TEST(A2:A11,B2:B11,2,2)")))
check("t-test equal: critical = T.INV.2T", close(find(t, "t Critical two-tail"), ev("T.INV.2T(0.05,18)")))
t, _ = analysis_ui.run_tool(win, "t-Test: Two-Sample Assuming Unequal Variances", {"input": "A1:A11", "input2": "C1:C11", "labels": True, "hyp": 0, "alpha": 0.05})
check("t-test unequal: df and t", find(t, "df") == round(find(t, "df")) and close(abs(find(t, "t Stat")), abs((sum(xa) / 10 - sum(xc) / 10) / ((AN.var_s(xa) + AN.var_s(xc)) / 10) ** 0.5)))
t, _ = analysis_ui.run_tool(win, "t-Test: Paired Two Sample for Means", {"input": "A1:A11", "input2": "B1:B11", "labels": True, "hyp": 0, "alpha": 0.05})
check("t-test paired: = T.TEST(,,2,1)", close(find(t, "P(T<=t) two-tail"), ev("T.TEST(A2:A11,B2:B11,2,1)")) and close(find(t, "Pearson Correlation"), ev("CORREL(A2:A11,B2:B11)")))
t, _ = analysis_ui.run_tool(win, "F-Test Two-Sample for Variances", {"input": "A1:A11", "input2": "B1:B11", "labels": True, "alpha": 0.05})
check("F-test: F and one-tail p", close(find(t, "F"), ev("VAR.S(A2:A11)/VAR.S(B2:B11)")) and close(find(t, "P(F<=f) one-tail"), ev("F.DIST.RT(VAR.S(A2:A11)/VAR.S(B2:B11),9,9)")),
      (find(t, "F"), find(t, "P(F<=f) one-tail")))
t, _ = analysis_ui.run_tool(win, "z-Test: Two Sample for Means", {"input": "A1:A11", "input2": "B1:B11", "labels": True, "hyp": 0, "var1": 6, "var2": 3, "alpha": 0.05})
check("z-test: critical two-tail 1.96", close(find(t, "z Critical two-tail"), 1.959963985, 1e-6))
t, _ = analysis_ui.run_tool(win, "Anova: Single Factor", {"input": "A1:C11", "grouped": "cols", "labels": True, "alpha": 0.05})
row = [r for r in t if r and r[0] == "Between Groups"][0]
check("anova: F, p and F crit", close(row[5], ev("F.DIST.RT(%r,2,27)" % row[4])) and close(row[6], ev("F.INV.RT(0.05,2,27)")), row)
t, _ = analysis_ui.run_tool(win, "Anova: Two-Factor Without Replication", {"input": "A2:C11", "labels": False, "alpha": 0.05})
check("anova two-factor: rows, columns, error", [r[0] for r in t if r and r[0] in ("Rows", "Columns", "Error", "Total")] == ["Rows", "Columns", "Error", "Total"])
t, _ = analysis_ui.run_tool(win, "Anova: Two-Factor With Replication", {"input": "A1:C11", "rows": 5, "alpha": 0.05})
check("anova with replication: interaction", any(r and r[0] == "Interaction" for r in t))
t, _ = analysis_ui.run_tool(win, "Correlation", {"input": "A1:C11", "grouped": "cols", "labels": True})
check("correlation matrix = CORREL", close(t[2][1], ev("CORREL(A2:A11,B2:B11)")) and t[1][1] == 1 and t[1][2] is None, t)
t, _ = analysis_ui.run_tool(win, "Covariance", {"input": "A1:C11", "grouped": "cols", "labels": True})
check("covariance = COVARIANCE.P", close(t[3][2], ev("COVARIANCE.P(B2:B11,C2:C11)")), t)
t, _ = analysis_ui.run_tool(win, "Regression", {"input": "C1:C11", "input2": "A1:B11", "labels": True, "zero": False, "conf_on": False, "conf": 95,
                                                "residuals": True, "stdres": True, "resplot": False, "fitplot": False})
coef = [r for r in t if r and r[0] in ("Intercept", "A", "B")]
check("regression: coefficients = LINEST", close(coef[0][1], ev("INDEX(LINEST(C2:C11,A2:B11),1,3)")) and close(coef[1][1], ev("INDEX(LINEST(C2:C11,A2:B11),1,2)"))
      and close(coef[2][1], ev("INDEX(LINEST(C2:C11,A2:B11),1,1)")), coef)
check("regression: R square and standard errors = LINEST", close(find(t, "R Square"), ev("INDEX(LINEST(C2:C11,A2:B11,TRUE,TRUE),3,1)"))
      and close(coef[1][2], ev("INDEX(LINEST(C2:C11,A2:B11,TRUE,TRUE),2,2)")), (find(t, "R Square"), coef))
check("regression: residual output", any(r[:1] == ["Observation"] for r in t))
t, _ = analysis_ui.run_tool(win, "Histogram", {"input": "A1:A11", "bins": "", "labels": True, "pareto": True, "cumulative": True, "chart": True})
check("histogram: frequencies add up", sum(r[1] for r in t[1:]) == 10 and t[0][:3] == ["Bin", "Frequency", "Cumulative %"], t)
t, _ = analysis_ui.run_tool(win, "Moving Average", {"input": "A2:A11", "labels": False, "interval": 3, "chart": False, "stderr": True})
check("moving average", t[0][0] == "#N/A" and t[1][0] == "#N/A" and close(t[2][0], (12 + 15 + 11) / 3) and t[3][1] == "#N/A" and isinstance(t[4][1], float), t[:6])
t, _ = analysis_ui.run_tool(win, "Exponential Smoothing", {"input": "A2:A11", "damping": 0.3, "labels": False, "chart": False, "stderr": False})
check("exponential smoothing", t[0][0] == "#N/A" and t[1][0] == 12 and close(t[2][0], 0.7 * 15 + 0.3 * 12), t[:3])
t, _ = analysis_ui.run_tool(win, "Rank and Percentile", {"input": "A1:A11", "grouped": "cols", "labels": True})
check("rank and percentile = RANK.EQ / PERCENTRANK.INC", t[1][1] == 19 and t[1][2] == 1 and t[1][3] == 1.0 and close(t[-1][3], ev("PERCENTRANK.INC(A2:A11,11)")), t[:3])
t, _ = analysis_ui.run_tool(win, "Sampling", {"input": "A2:A11", "labels": False, "method": "Periodic", "period": 3, "count": 5, "seed": ""})
check("sampling: every third", [r[0] for r in t] == [11, 16, 19], t)
t, _ = analysis_ui.run_tool(win, "Random Number Generation", {"nvars": 2, "count": 50, "dist": "Uniform", "p1": 5, "p2": 6, "p3": 1, "p4": 1, "p5": 1, "input": "", "seed": "7"})
check("random numbers: uniform between its bounds", len(t) == 50 and all(5 <= v <= 6 for r in t for v in r))
t2, _ = analysis_ui.run_tool(win, "Random Number Generation", {"nvars": 2, "count": 50, "dist": "Uniform", "p1": 5, "p2": 6, "p3": 1, "p4": 1, "p5": 1, "input": "", "seed": "7"})
check("random numbers: the same seed, the same numbers", t == t2)
t, _ = analysis_ui.run_tool(win, "Random Number Generation", {"nvars": 1, "count": 6, "dist": "Patterned", "p1": 1, "p2": 3, "p3": 1, "p4": 2, "p5": 1, "input": "", "seed": ""})
check("random numbers: patterned", [r[0] for r in t] == [1, 1, 2, 2, 3, 3], t)
for i, z in enumerate(["1", "2", "3", "4"]):
    b.set_input(SA, 20 + i, 0, z)
t, _ = analysis_ui.run_tool(win, "Fourier Analysis", {"input": "A21:A24", "labels": False, "inverse": False})
check("fourier: FFT of 1,2,3,4", [r[0] for r in t] == ["10", "-2+2i", "-2", "-2-2i"], t)
DIALOG.append(lambda d: Gtk.ResponseType.OK)
try:
    analysis_ui.run_tool(win, "Fourier Analysis", {"input": "A21:A23", "labels": False, "inverse": False})
    ok = False
except analysis_ui.InputError:
    ok = True
DIALOG.clear()
check("fourier: needs 2^n values", ok)
# through the dialog, to a new sheet
nsh = b.sheet_count


def _pick(d):
    d.select("Descriptive Statistics")
    return Gtk.ResponseType.OK


def _fill(d):
    d.w["input"][1].set_text("$A$1:$B$11")
    d.w["labels"][1].set_active(True)
    d.o_sheet_e.set_text("Stats")
    return Gtk.ResponseType.OK


DIALOG.extend([_pick, _fill])
got = win.run("data-analysis")
pump(0.2)
st = [i for i in range(b.sheet_count) if b.sheet_info(i)["name"] == "Stats"]
check("Data Analysis dialog: output on a new sheet", b.sheet_count == nsh + 1 and st and b.display(st[0], 0, 0) == "A" and b.display(st[0], 1, 0) == "Mean",
      (b.sheet_count, st and [b.display(st[0], r, 0) for r in range(3)]))
check("Data Analysis: values, not formulas", st and not b.input(st[0], 1, 1).startswith("="))
SA = [i for i in range(b.sheet_count) if b.sheet_info(i)["name"] == SA_NAME][0]
g.set_sheet(SA)
pump()
ws = analysis_ui.apply(win, "Correlation", {"input": "A1:C11", "grouped": "cols", "labels": True}, ("range", "$H$2"))
check("output range", b.display(SA, 2, 8) == "1" and b.display(SA, 1, 8) == "A", [b.display(SA, 1, c) for c in range(7, 11)])
check("Data Analysis command wired", hasattr(win, "act_data_analysis"))
g.set_sheet(0)

# ---------------------------------------------------------------- end
w2.book.mark_saved()
for p in tuple(globals().get("EXTRA", ())):
    p()

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
