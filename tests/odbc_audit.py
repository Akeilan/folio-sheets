# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): S4-01 Get Data from a database over ODBC — the
# sources this computer is set up for, the tables of one, and loading a table as a query. Without
# python3-pyodbc it checks the message that says how to install it instead.
import os
import sqlite3
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


SEEN = []


def fake_run(d):
    pump(0.05)
    SEEN.append(d.get_title())
    return Gtk.ResponseType.OK


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

from folio_sheets import datasources, getdata_ui  # noqa: E402
from folio_sheets import commands as cmds  # noqa: E402

b = win.book
TMP = tempfile.mkdtemp(prefix="sheet-odbc-")
dbp = os.path.join(TMP, "sales.db")
con = sqlite3.connect(dbp)
con.execute("create table sales(region text, units integer, total real)")
con.executemany("insert into sales values(?,?,?)",
                [("North", 30, 1200.5), ("South", 22, 950.0), ("East", 41, 1430.25)])
con.execute("create view big as select * from sales where units > 25")
con.commit()
con.close()

check("From ODBC is in the Get Data menu", hasattr(win, cmds.method_name("from-source:odbc")))
check("odbc is a source Sheet can read and list", "odbc" in datasources.READERS and "odbc" in datasources.ITEMS)

try:
    import pyodbc                                     # noqa: F401
    HAVE = True
except ImportError:
    HAVE = False

if not HAVE:
    try:
        datasources.odbc_sources()
        check("without pyodbc it says so", False, "no error raised")
    except ValueError as e:
        check("without pyodbc it says which package to install", "python3-pyodbc" in str(e), str(e))
    out = getdata_ui.from_source(win, "odbc", "DSN=nothing")
    check("and reading one stops politely", out is None)
    check("the chooser says the same", getdata_ui.choose_odbc(win) is None)
else:
    ini = os.path.join(TMP, "odbc.ini")
    drv = sorted((d for d in datasources.odbc_drivers() if "sqlite" in d.lower()),
                 key=lambda d: ("3" not in d, d))     # SQLite3 before the older SQLite2 driver
    open(ini, "w").write("[folio_test]\nDescription=Folio test\nDriver=%s\nDatabase=%s\n"
                         % (drv[0] if drv else "SQLite3", dbp))
    os.environ["ODBCINI"] = ini
    check("the SQLite ODBC driver is one of the drivers listed", bool(drv), datasources.odbc_drivers())
    names = [n for n, _d in datasources.odbc_sources()]
    check("the source set up on this computer is listed", "folio_test" in names, names)
    items = datasources.odbc_items("folio_test")
    check("its table and view are listed", [i[1] for i in items] == ["big", "sales"]
          and any("(view)" in i[0] for i in items), items)
    res = datasources.read_odbc({"path": "folio_test", "item": "sales"})
    check("a table reads with its headers and rows", res.headers == ["region", "units", "total"]
          and res.rows[2] == ["East", "41", "1430.25"], (res.headers, res.rows))
    res2 = datasources.read_odbc({"path": "folio_test", "item": "select region from sales where units > 25".upper()})
    check("a SELECT reads too", [r[0] for r in res2.rows] == ["North", "East"], res2.rows)
    cs = "DRIVER={%s};DATABASE=%s" % (drv[0] if drv else "SQLite3", dbp)
    res3 = datasources.read_odbc({"path": cs, "item": "sales"})
    check("a whole connection string works as well as a DSN", len(res3.rows) == 3, res3.rows)
    with_pw = cs + ";PWD=hunter2;UID=me"
    kept = datasources.odbc_scrub(with_pw)
    check("a typed password is left out of what the query keeps",
          "hunter2" not in kept and "UID=me" in kept, kept)
    res5 = datasources.read_odbc({"path": kept, "item": "sales"})
    check("and the source still reads in this session", len(res5.rows) == 3, res5.rows)
    check("the chooser offers the source it found", getdata_ui.choose_odbc(win) == "folio_test")
    out = getdata_ui.from_source(win, "odbc", "folio_test", item="sales", choice="load")
    pump(0.3)
    q = getdata_ui.find_query(b, "sales")
    check("From ODBC loads the table as a query", out and q and q["kind"] == "odbc"
          and q["path"] == "folio_test" and b.display(out[0], 3, 0) == "East", (out, q))
    res4 = getdata_ui.read_source(q, b)
    check("Refresh reads the source again", res4.rows[0][0] == "North", res4.rows[:1])
    try:
        datasources.odbc_items("no_such_source_here")
        check("a source that isn't there is reported", False, "no error raised")
    except Exception as e:
        check("a source that isn't there is reported", "no_such_source_here" in str(e) or "data source" in str(e).lower(), str(e))

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
sys.stdout.flush()
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
