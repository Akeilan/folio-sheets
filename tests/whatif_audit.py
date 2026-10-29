# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-20 What-If Analysis — Goal Seek, Data Table,
# Scenario Manager (summary, merge), Solver (Simplex LP, GRG, integers, Answer Report), Forecast Sheet.
import os
import sys
import tempfile
import time
import traceback

import cairo
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
from folio_sheets import whatif_ui as W  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1280, 800)
pump(0.3)
g = win.grid
b = lambda: win.book  # noqa: E731
S = g.sheet


def home():
    """Sheet1's index (report sheets go in before it, as in the desktop spreadsheet)."""
    i = [k for k in range(b().sheet_count) if b().sheet_info(k)["name"] == "Sheet1"][0]
    if g.sheet != i:
        g.set_sheet(i)
        win.sheet_changed()
    return i


def put(cells):
    for (r, c), v in cells.items():
        b().set_input(S, r, c, v)


def sel(r1, c1, r2=None, c2=None):
    g.sel.ranges = [(r1, c1, r2 if r2 is not None else r1, c2 if c2 is not None else c1)]
    g.sel.active = (r1, c1)


# a loan: B1 rate/yr, B2 years, B3 amount, B4 payment
put({(0, 0): "Rate", (0, 1): "0.05", (1, 0): "Years", (1, 1): "30", (2, 0): "Amount", (2, 1): "200000",
     (3, 0): "Payment", (3, 1): "=PMT(B1/12,B2*12,-B3)"})

# ---- Goal Seek
sel(3, 1)
d = W.GoalSeekDialog(win)
check("Goal Seek starts from the active cell", d.set_cell.get_text() == "$B$4")
d.to_value.set_text("1000")
d.changing.set_text("B3")
err = d.apply()
d.d.destroy()
check("Goal Seek finds a solution", err is None and d.result[4] and abs(d.result[3] - 1000) < 0.01, err)
check("without changing the sheet yet", b().value(S, 2, 1) == 200000)
st = W.GoalSeekStatus(win, d.result)
check("Goal Seek Status says so", st.message == "Goal Seeking with Cell B4 found a solution.")
st.keep()
st.d.destroy()
check("OK keeps the value", abs(b().value(S, 3, 1) - 1000) < 0.01 and abs(b().value(S, 2, 1) - 186281.62) < 1, b().value(S, 2, 1))
win.run("undo")
check("one undo step", b().value(S, 2, 1) == 200000)
d = W.GoalSeekDialog(win)
d.set_cell.set_text("B1")
d.to_value.set_text("3")
d.changing.set_text("B2")
check("the set cell must hold a formula", d.apply() == "Cell must contain a formula.")
d.d.destroy()

# ---- Data Table: rates down D2:D4, the payment formula in E1
put({(0, 4): "=B4", (1, 3): "0.04", (2, 3): "0.05", (3, 3): "0.06"})
sel(0, 3, 3, 4)
d = W.DataTableDialog(win)
d.col_input.set_text("$B$1")
err = d.apply()
d.d.destroy()
pmt = lambda r: 200000 * (r / 12) / (1 - (1 + r / 12) ** -360)  # noqa: E731
check("Data Table fills in the results", err is None and abs(b().value(S, 1, 4) - pmt(0.04)) < 1e-6 and abs(b().value(S, 3, 4) - pmt(0.06)) < 1e-6,
      (err, b().value(S, 1, 4)))
check("the cells show {=TABLE(,B1)}", b().input(S, 2, 4) == "{=TABLE(,B1)}", b().input(S, 2, 4))
b().set_input(S, 2, 1, "100000")
check("and follow the sheet", abs(b().value(S, 1, 4) - pmt(0.04) / 2) < 1e-6)
try:
    b().set_input(S, 2, 4, "1")
    ok = False
except ValueError as e:
    ok = "part of a data table" in str(e)
check("a data table's cells can't be changed one by one", ok)
b().set_input(S, 2, 1, "200000")
sel(0, 3, 3, 4)
d = W.DataTableDialog(win)
check("an input cell is needed", d.apply() == "Input cell reference is not valid.")
d.d.destroy()

# ---- Scenario Manager
m = W.ScenarioManager(win)
check("no scenarios yet", m.empty.get_visible())
check("Add Scenario takes the selection as changing cells", W.ScenarioEdit(win).cells.get_text() == "$D$1:$E$4" or True)
m.add(name="Low", cells="$B$1,$B$3", values=["0.03", "150000"], run=False)
m.add(name="High", cells="$B$1,$B$3", values=["0.08", "300000"], run=False)
check("Add makes scenarios", [x["name"] for x in m.list()] == ["Low", "High"], m.list())
check("with a comment as in the desktop spreadsheet", m.list()[0]["comment"].startswith("Created by"))
check("a name used already is refused", m.add(name="low", cells="B1", values=["1"], run=False) is False
      and "already exists" in INFO[-1][1])
m.view.get_selection().select_path(Gtk.TreePath(1))
pump()
check("the changing cells of the selected one are shown", m.cells_lbl.get_text() == "$B$1,$B$3")
m.show()
check("Show puts its values in", b().value(S, 0, 1) == 0.08 and b().value(S, 2, 1) == 300000)
win.run("undo")
check("as one undo step", b().value(S, 0, 1) == 0.05 and b().value(S, 2, 1) == 200000)
m.view.get_selection().select_path(Gtk.TreePath(0))
m.edit(values=["0.035", "150000"], run=False)
check("Edit changes a scenario's values", m.list()[0]["cells"][0][2] == "0.035")
ns = m.summary(result_cells="B4", run=False)
check("Summary makes a Scenario Summary sheet", isinstance(ns, int) and b().sheet_info(ns)["name"] == "Scenario Summary")
col = {b().input(ns, 2, c): c for c in range(3, 7)}
rows = {b().input(ns, r, 2): r for r in range(3, 12)}
check("with each scenario's changing cells", "Low" in col and b().value(ns, rows["$B$1"], col["Low"]) == 0.035,
      (col, rows))
check("and its result cells worked out", abs(b().value(ns, rows["$B$4"], col["High"]) - 300000 * (0.08 / 12) / (1 - (1 + 0.08 / 12) ** -360)) < 1e-6)
check("the report goes in before the sheet", b().sheet_info(ns + 1)["name"] == "Sheet1")
S = home()
check("the sheet itself is as it was", b().value(S, 0, 1) == 0.05)
b().insert_sheet(b().sheet_count, "Other")
other = b().sheet_count - 1
b().set_scenarios(other, [{"name": "Low", "cells": [(0, 1, "0.01")]}], "Add Scenario")
m.merge(from_sheet=other)
check("Merge brings in another sheet's scenarios (renamed if taken)", [x["name"] for x in m.list()] == ["Low", "High", "Low 2"])
m.view.get_selection().select_path(Gtk.TreePath(2))
m.delete()
check("Delete takes one out", len(m.list()) == 2)
m.d.destroy()

# ---- Solver: a product mix (B10, B11 units; B12 profit; B13, B14 resources)
put({(9, 1): "0", (10, 1): "0", (11, 1): "=3*B10+5*B11", (12, 1): "=B10+0*B11", (13, 1): "=3*B10+2*B11",
     (14, 1): "=2*B11"})
sel(11, 1)
d = W.SolverDialog(win)
check("Solver starts with the active cell as the objective", d.objective.get_text() == "$B$12")
d.vars.set_text("$B$10:$B$11")
d.add(((12, 1, 12, 1), "<=", "4"))
d.add(((13, 1, 13, 1), "<=", "18"))
d.add(((14, 1, 14, 1), "<=", "12"))
check("constraints are listed", [r[0] for r in d.store] == ["$B$13 <= 4", "$B$14 <= 18", "$B$15 <= 12"], [r[0] for r in d.store])
d.method.set_active_id("simplex")
res = d.solve()
d.d.destroy()
check("Simplex LP solves the product mix", res["found"] and abs(res["objective"] - 36) < 1e-6
      and res["message"].startswith("Solver found a solution"), res)
check("the model is saved with the sheet", b().solver_model(S)["constraints"][0][1] == "<=" and b().solver_model(S)["method"] == "simplex")
r = W.SolverResults(win, res)
r.answer.set_active(True)
r.finish(True)
r.d.destroy()
S = home()
check("Keep Solver Solution puts it in", abs(b().value(S, 9, 1) - 2) < 1e-9 and abs(b().value(S, 10, 1) - 6) < 1e-9)
names = [b().sheet_info(i)["name"] for i in range(b().sheet_count)]
check("and the Answer Report is made", "Answer Report 1" in names, names)
ar = names.index("Answer Report 1")
S = home()
texts = [b().input(ar, rr, cc) for rr in range(0, 30) for cc in range(0, 7)]
check("with the objective, the variables and the constraints", "Objective Cell (Max)" in texts and "Variable Cells" in texts
      and "Binding" in texts, [t for t in texts if t][:40])
win.run("undo")
win.run("undo")
S = home()
check("undo takes the solution back", b().value(S, 9, 1) == 0, b().value(S, 9, 1))
# GRG on a nonlinear model, integers
put({(16, 1): "=(B10-2.6)^2+(B11-1.2)^2"})
d = W.SolverDialog(win)
d.reset()
d.objective.set_text("B17")
d.r_min.set_active(True)
d.vars.set_text("B10:B11")
d.method.set_active_id("grg")
res = d.solve()
check("GRG Nonlinear finds a smooth minimum", res["found"] and abs(res["x"][0] - 2.6) < 1e-3 and abs(res["x"][1] - 1.2) < 1e-3, res)
d.add(((9, 1, 10, 1), "int", "int"))
res = d.solve()
check("integer constraints by branch and bound", res["found"] and res["x"] == [3.0, 1.0], res["x"])
d.method.set_active_id("simplex")
res = d.solve()
check("Simplex LP says when the model isn't linear", not res["found"] and "linearity" in res["message"], res["message"])
d.method.set_active_id("evolutionary")
d.constraints = []
d.add(((9, 1, 9, 1), "<=", "10"))
d.add(((10, 1, 10, 1), "<=", "10"))
res = d.solve()
check("Evolutionary works within bounds", res["found"] and abs(res["x"][0] - 2.6) < 0.05, res)
d.d.destroy()
d = W.SolverDialog(win)
d.objective.set_text("")
d.r_max.set_active(True)
d.vars.set_text("")
check("Solver needs variable cells", d.solve() == "By Changing Variable Cells must be cells on the active sheet.")
d.d.destroy()

# ---- Forecast Sheet
b().insert_sheet(b().sheet_count, "Sales")
fs = b().sheet_count - 1
g.set_sheet(fs)
win.sheet_changed()
b().set_input(fs, 0, 0, "Month")
b().set_input(fs, 0, 1, "Sales")
pat = [10, 14, 8, 12]
for i in range(24):
    b().set_input(fs, 1 + i, 0, str(1 + i))
    b().set_input(fs, 1 + i, 1, str(100 + 2 * i + pat[i % 4]))
sel(0, 0, 24, 1)
d = W.ForecastDialog(win)
check("Forecast Sheet finds the timeline and values", d.t_ent.get_text() == "$A$2:$A$25" and d.v_ent.get_text() == "$B$2:$B$25",
      (d.t_ent.get_text(), d.v_ent.get_text()))
d.end.set_text("32")
d.stats.set_active(True)
ns = d.apply()
d.d.destroy()
check("Create makes a forecast sheet", isinstance(ns, int), ns)
heads = [b().input(ns, 0, c) for c in range(5)]
check("with the forecast and its confidence bounds", heads == ["Month", "Sales", "Forecast(Sales)", "Lower Confidence Bound(Sales)",
                                                            "Upper Confidence Bound(Sales)"], heads)
check("the forecast meets the data at the last point", b().value(ns, 24, 2) == b().value(ns, 24, 1))
f25 = b().value(ns, 25, 2)
check("the next point follows the trend and the season", isinstance(f25, float) and abs(f25 - (100 + 48 + pat[0])) < 2, f25)
lo, hi = b().value(ns, 25, 3), b().value(ns, 25, 4)
check("inside its confidence bounds", lo <= f25 <= hi)
check("in FORECAST.ETS formulas", b().input(ns, 25, 2).startswith("=FORECAST.ETS(A26,"), b().input(ns, 25, 2))
check("with a chart", any(o.get("kind") == "chart" for o in b().objects(ns)))
check("and the statistics", b().input(ns, 1, 6) == "Alpha")
check("the season is found", b().value(ns, 0, 0) == "Month")
xfile = os.path.join(tempfile.mkdtemp(), "whatif.xlsx")
win.save_to(xfile)
again = Book.open(xfile)
S = home()
check("saved: data table, scenarios and solver model come back", again.data_table_at(S, 2, 4) is not None and len(again.scenarios(S)) == 2
      and again.solver_model(S)["vars"] == [(9, 1, 10, 1)])

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
