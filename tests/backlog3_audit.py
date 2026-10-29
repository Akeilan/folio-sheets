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

# ---------------------------------------------------------------- end
w2.book.mark_saved()
for p in tuple(globals().get("EXTRA", ())):
    p()

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
