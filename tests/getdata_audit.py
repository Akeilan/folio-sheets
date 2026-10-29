# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): SH-25 Get Data from local sources — From Text/CSV,
# From Workbook and From Folder into tables; Refresh and Refresh All read the sources again; the
# queries travel with the file; web, database and the Power Query editor say they aren't available.
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


from folio_sheets import getdata_ui as G  # noqa: E402
from folio_sheets.core import Book  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
tmp = tempfile.mkdtemp()
win.resize(1280, 800)
pump(0.3)
b = lambda: win.book  # noqa: E731

# ---- From Text/CSV
csvp = os.path.join(tmp, "sales.csv")
with open(csvp, "w", encoding="utf-8") as f:
    f.write("Name,Qty,Price,When\nApples,3,1.25,2026-01-05\nPears,10,0.5,2026-01-06\n\"Figs, dried\",2,4,2026-01-07\n")
d = G.CsvDialog(win, csvp)
pump()
check("the preview reads the file: comma, headers from the first row",
      d.delim.get_active_id() == "," and d.result.headers == ["Name", "Qty", "Price", "When"] and len(d.result.rows) == 3,
      (d.delim.get_active_id(), d.result.headers))
check("and detects the column types", d.result.kinds == ["text", "number", "number", "date"], d.result.kinds)
check("Transform Data is there and available", d.buttons.transform_button.get_sensitive())
d.destroy()

CHOICE = {"v": "load"}


def fake_run(self):
    self.buttons.choice = CHOICE["v"]
    return Gtk.ResponseType.OK


for cls in (G.CsvDialog, G.NavigatorDialog, G.FolderDialog):
    cls.run = fake_run
n0 = b().sheet_count
res = G.from_text_csv(win, csvp)
pump(0.1)
check("Load puts it on a new sheet named after the file", res is not None and b().sheet_count == n0 + 1
      and b().sheet_info(res[0])["name"] == "sales" and win.grid.sheet == res[0], res)
s, i = res
t = b().tables(s)[i]
check("as a table called after the query", t["name"] == "sales" and t["ref"] == (0, 0, 3, 3) and t["header"], t)
check("numbers are numbers, text is text", b().value(s, 1, 1) == 3 and b().value(s, 3, 0) == "Figs, dried" and b().value(s, 2, 2) == 0.5,
      (b().value(s, 1, 1), b().value(s, 3, 0)))
check("dates are dates", isinstance(b().value(s, 1, 3), float) and b().display(s, 1, 3) == "2026-01-05", (b().value(s, 1, 3), b().display(s, 1, 3)))
qs = G.queries(b())
check("the query is kept in the workbook", len(qs) == 1 and qs[0]["path"] == csvp and qs[0]["table"] == "sales", qs)
check("in a hidden name (not in the Name Manager)", all(h for n, _t, _s, h in b().names() if n.startswith(G.PREFIX)))

# the source changes; Refresh All reads it again
with open(csvp, "a", encoding="utf-8") as f:
    f.write("Kiwis,7,0.8,2026-01-08\nPlums,1,2,2026-01-09\n")
win.run("refresh-all")
pump()
t = b().tables(s)[0]
check("Refresh All: the table grows with the file", t["ref"] == (0, 0, 5, 3) and b().value(s, 5, 0) == "Plums", t["ref"])
with open(csvp, "w", encoding="utf-8") as f:
    f.write("Name,Qty,Price,When\nOnly,1,1,2026-02-01\n")
win.grid.sel.set_active(1, 1) if hasattr(win.grid.sel, "set_active") else None
win.run("refresh")
pump()
t = b().tables(s)[0]
check("and shrinks, leaving nothing behind", t["ref"] == (0, 0, 1, 3) and b().value(s, 2, 0) is None and b().value(s, 5, 0) is None, t["ref"])
check("one query still", len(G.queries(b())) == 1 and G.queries(b())[0]["rows"] == 1)

# kept in the saved file
xl = os.path.join(tmp, "withquery.xlsx")
win.save_to(xl)
again = Book.open(xl)
q2 = G.queries(again)
check("saved and opened again, the query is still there", len(q2) == 1 and q2[0]["path"] == csvp and q2[0]["name"] == "sales", q2)

# other encodings and delimiters
semi = os.path.join(tmp, "liste.txt")
with open(semi, "w", encoding="cp1252") as f:
    f.write("Ville;Habitants\nGenève;203856\nZürich;421878\n")
d = G.CsvDialog(win, semi)
d.enc.set_active_id("cp1252")
pump()
check("semicolons found, encoding chosen", d.delim.get_active_id() == ";" and d.result.rows[0][0] == "Genève", (d.delim.get_active_id(), d.result.rows[:1]))
d.detect.set_active_id("none")
pump()
check("Do not detect data types: everything text", d.result.kinds == ["text", "text"])
d.destroy()

# Load To: only a connection
CHOICE["v"] = "load-to"
G.LoadToDialog.run = lambda self: (self.conn.set_active(True), Gtk.ResponseType.OK)[1]
n1 = b().sheet_count
G.from_text_csv(win, semi)
pump()
qs = G.queries(b())
conn = [q for q in qs if q["name"] == "liste"]
check("Load To › Only Create Connection: no sheet, a query", b().sheet_count == n1 and conn and conn[0]["table"] is None, qs)
# Load To an existing sheet at a cell
G.LoadToDialog.run = lambda self: (self.existing.set_active(True), self.ref.set_text("=$C$10"), Gtk.ResponseType.OK)[2]
win._after_structure(0)
G.from_text_csv(win, semi)
pump()
loc = G.locate(b(), "liste_2")
check("Load To › Existing worksheet at C10", loc and loc[0] == 0 and loc[2]["ref"][:2] == (9, 2), loc)

# ---- From Workbook
srcp = os.path.join(tmp, "source.xlsx")
sb = Book.open(win.app.blank_template())
for r, row in enumerate([["Region", "Total"], ["North", "10"], ["South", "20"]]):
    for c, v in enumerate(row):
        sb.set_input(0, r, c, v)
sb.create_table(0, (0, 0, 2, 1), True, "TableStyleLight1")
sb.rename_table(0, 0, "Totals")
sb.insert_sheet(1, "Notes")
sb.set_input(1, 0, 0, "Item")
sb.set_input(1, 1, 0, "one")
sb.save(srcp)
CHOICE["v"] = "load"
d = G.NavigatorDialog(win, srcp)
pump()
labels = [it[0] for it in d.items]
check("Navigator lists the tables and sheets", "Totals" in labels and "Notes" in labels and "Sheet1" in labels, labels)
d.destroy()
res = G.from_workbook(win, srcp)
pump()
check("From Workbook loads the first item (the table)", res and b().tables(res[0])[res[1]]["name"] == "Totals"
      and b().value(res[0], 2, 1) == 20, res)
sb.set_input(0, 3, 0, "East")
sb.set_input(0, 3, 1, "5")
sb.resize_table(0, 0, (0, 0, 3, 1))
sb.save(srcp)
win.run("refresh-all")
pump()
loc = G.locate(b(), "Totals")
check("refreshing reads the workbook again", loc and loc[2]["ref"] == (0, 0, 3, 1) and b().value(loc[0], 3, 0) == "East", loc and loc[2]["ref"])

# ---- From Folder
fold = os.path.join(tmp, "monthly")
os.makedirs(fold)
for m, rows in (("jan", "A,1\nB,2\n"), ("feb", "C,3\n")):
    with open(os.path.join(fold, m + ".csv"), "w") as f:
        f.write("Item,Count\n" + rows)
d = G.FolderDialog(win, fold)
pump()
check("From Folder lists the files", sorted(r[0] for r in d.result.rows) == ["feb.csv", "jan.csv"] and d.result.headers[0] == "Name")
check("Combine is offered", d.buttons.combine_button is not None)
d.destroy()
CHOICE["v"] = "combine"
res = G.from_folder(win, fold)
pump()
t = b().tables(res[0])[res[1]]
check("Combine & Load: the files one under the other, with Source.Name", t["columns"] == ["Source.Name", "Item", "Count"]
      and t["ref"][2] == 3, (t["columns"], t["ref"]))
vals = sorted((b().value(res[0], r, 0), b().value(res[0], r, 1)) for r in range(1, 4))
check("every row from every file", vals == [("feb.csv", "C"), ("jan.csv", "A"), ("jan.csv", "B")], vals)
with open(os.path.join(fold, "mar.csv"), "w") as f:
    f.write("Item,Count\nD,4\n")
win.run("refresh-all")
pump()
loc = G.locate(b(), "monthly")
check("a new file in the folder arrives on refresh", loc and loc[2]["ref"][2] == 4, loc and loc[2]["ref"])

# ---- Queries & Connections pane
win.run("queries-connections")
pump()
pane = win.queries_pane
rows = pane.list.get_children()
check("Queries & Connections lists every query", pane.get_visible() and len(rows) == len(G.queries(b())), (len(rows), len(G.queries(b()))))
check("with what each one loaded", any("Connection only." in [w.get_text() for w in r.get_child().get_children()[0].get_children()] for r in rows))
before = len(G.queries(b()))
G._pane_delete(win, G.find_query(b(), "liste"))
check("deleting a query", len(G.queries(b())) == before - 1)
win.run("queries-connections")

# ---- undo takes a load back
n2 = b().sheet_count
q_before = len(G.queries(b()))
CHOICE["v"] = "load"
G.from_text_csv(win, semi)
pump()
win.run("undo")
pump()
check("undo takes a load back: sheet and query", b().sheet_count == n2 and len(G.queries(b())) == q_before, (b().sheet_count, n2))

# ---- what is out
for cmd in ("get-data-unavailable:Getting data from the web", "get-data-unavailable:Getting data from a database"):
    k = len(INFO)
    win.run(cmd)
    check("%s says it isn't available" % cmd.split(":")[1], len(INFO) == k + 1 and "isn't available" in INFO[-1][1], INFO[-1:])

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
