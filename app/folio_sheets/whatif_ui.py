"""What-If Analysis, Solver and Forecast Sheet (SH-20): Goal Seek, Data Table, the Scenario
Manager (with its summary report), Solver (Simplex LP, GRG Nonlinear, Evolutionary, with an
Answer Report) and Forecast Sheet (FORECAST.ETS with confidence bounds and a chart).

The dialogs are as in the desktop spreadsheet; each has an apply step the audits drive
directly. Values are tried in the engine without touching the sheet: only what you keep goes
in, as one undo step."""
import datetime
import re

from gi.repository import Gtk

from . import APP_NAME
from .core import a1

REF = re.compile(r"^\s*=?\s*(?:(?:'[^']+'|[A-Za-z0-9_.]+)!)?\$?([A-Za-z]{1,3})\$?(\d+)(?::\$?([A-Za-z]{1,3})\$?(\d+))?\s*$")


def col_num(s):
    n = 0
    for ch in s.upper():
        n = n * 26 + ord(ch) - 64
    return n - 1


def parse_ref(text):
    """'B3', '$B$3', 'Sheet1!$B$3:$C$4' -> (r1, c1, r2, c2), or None."""
    m = REF.match(text or "")
    if not m:
        return None
    r1, c1 = int(m.group(2)) - 1, col_num(m.group(1))
    if m.group(3):
        r2, c2 = int(m.group(4)) - 1, col_num(m.group(3))
    else:
        r2, c2 = r1, c1
    return min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2)


def parse_refs(text):
    out = []
    for part in (text or "").split(","):
        if not part.strip():
            continue
        r = parse_ref(part)
        if r is None:
            return None
        out.append(r)
    return out


def abs_ref(r):
    def one(rr, cc):
        a = a1(rr, cc)
        m = re.match(r"([A-Z]+)(\d+)", a)
        return "$%s$%s" % (m.group(1), m.group(2))
    return one(r[0], r[1]) if (r[0], r[1]) == (r[2], r[3]) else "%s:%s" % (one(r[0], r[1]), one(r[2], r[3]))


def _row(grid, k, label, widget):
    grid.attach(Gtk.Label(label=label, xalign=0), 0, k, 1, 1)
    grid.attach(widget, 1, k, 1, 1)


def _entry(name, text=""):
    e = Gtk.Entry(text=text)
    e.set_name(name)
    e.set_width_chars(24)
    e.set_activates_default(True)
    return e


def _dialog(win, title, ok="OK"):
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.add_button(ok, Gtk.ResponseType.OK)
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(column_spacing=10, row_spacing=6)
    grid.set_border_width(12)
    d.get_content_area().pack_start(grid, True, True, 0)
    return d, grid


def _sel_ref(win):
    g = win.grid
    r1, c1, r2, c2 = g.sel.ranges[-1]
    return abs_ref((min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2)))


def _num_text(v):
    if isinstance(v, float):
        return ("%.15g" % v)
    return str(v)


# ---------------------------------------------------------------- Goal Seek
class GoalSeekDialog:
    def __init__(self, win):
        self.win = win
        self.d, grid = _dialog(win, "Goal Seek")
        r, c = win.grid.sel.active
        self.set_cell = _entry("set-cell", abs_ref((r, c, r, c)))
        self.to_value = _entry("to-value")
        self.changing = _entry("changing-cell")
        _row(grid, 0, "Set cell:", self.set_cell)
        _row(grid, 1, "To value:", self.to_value)
        _row(grid, 2, "By changing cell:", self.changing)
        self.d.show_all()

    def apply(self):
        """Seek; returns (message, found, value found, target value) or an error text."""
        w = self.win
        t = parse_ref(self.set_cell.get_text())
        if t is None or (t[0], t[1]) != (t[2], t[3]):
            return "Reference must be to a single cell."
        try:
            goal = float(self.to_value.get_text())
        except ValueError:
            return "Invalid value."
        ch = parse_ref(self.changing.get_text())
        if ch is None or (ch[0], ch[1]) != (ch[2], ch[3]):
            return "Reference must be to a single cell."
        try:
            x, val, found = w.book.goal_seek(w.grid.sheet, t[0], t[1], goal, ch[0], ch[1])
        except ValueError as e:
            return str(e)
        self.result = (t, ch, x, val, found, goal)
        return None


class GoalSeekStatus:
    """'Goal Seeking with Cell B3 found a solution.' — OK keeps it, Cancel puts things back."""
    def __init__(self, win, result):
        self.win = win
        t, ch, x, val, found, goal = result
        self.result = result
        self.d = Gtk.Dialog(title="Goal Seek Status", transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(12)
        cell = a1(t[0], t[1])
        self.message = ("Goal Seeking with Cell %s found a solution." % cell if found
                        else "Goal Seeking with Cell %s may not have found a solution." % cell)
        self.msg = Gtk.Label(label=self.message, xalign=0)
        box.pack_start(self.msg, False, False, 0)
        g = Gtk.Grid(column_spacing=12, row_spacing=2)
        g.attach(Gtk.Label(label="Target value:", xalign=0), 0, 0, 1, 1)
        g.attach(Gtk.Label(label=_num_text(goal), xalign=1), 1, 0, 1, 1)
        g.attach(Gtk.Label(label="Current value:", xalign=0), 0, 1, 1, 1)
        g.attach(Gtk.Label(label=w_display(val), xalign=1), 1, 1, 1, 1)
        box.pack_start(g, False, False, 0)
        self.d.get_content_area().pack_start(box, True, True, 0)
        self.d.show_all()

    def keep(self):
        t, ch, x, val, found, goal = self.result
        w = self.win
        w.book.set_input(w.grid.sheet, ch[0], ch[1], _num_text(x))
        w.grid.refresh()
        w.update_title()


def w_display(v):
    return ("%.6g" % v) if isinstance(v, float) else str(v)


def goal_seek(win):
    d = GoalSeekDialog(win)
    while True:
        if d.d.run() != Gtk.ResponseType.OK:
            d.d.destroy()
            return False
        err = d.apply()
        if err is None:
            break
        win.dialogs.message(err, "Goal Seek")
    d.d.destroy()
    st = GoalSeekStatus(win, d.result)
    ok = st.d.run() == Gtk.ResponseType.OK
    if ok:
        st.keep()
    st.d.destroy()
    return ok


# ---------------------------------------------------------------- Data Table
class DataTableDialog:
    def __init__(self, win):
        self.win = win
        self.d, grid = _dialog(win, "Data Table")
        self.row_input = _entry("row-input")
        self.col_input = _entry("column-input")
        _row(grid, 0, "Row input cell:", self.row_input)
        _row(grid, 1, "Column input cell:", self.col_input)
        self.d.show_all()

    def apply(self):
        w = self.win
        g = w.grid
        r1, c1, r2, c2 = g.sel.ranges[-1]
        sel = (min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2))
        ri = self.row_input.get_text().strip()
        ci = self.col_input.get_text().strip()
        row = parse_ref(ri) if ri else None
        col = parse_ref(ci) if ci else None
        if (ri and (row is None or row[:2] != row[2:])) or (ci and (col is None or col[:2] != col[2:])) or (not ri and not ci):
            return "Input cell reference is not valid."
        try:
            w.book.create_data_table(g.sheet, *sel, row_input=row[:2] if row else None, col_input=col[:2] if col else None)
        except ValueError as e:
            return str(e)
        g.refresh()
        w.update_title()
        return None


def data_table(win):
    d = DataTableDialog(win)
    ok = False
    while True:
        if d.d.run() != Gtk.ResponseType.OK:
            break
        err = d.apply()
        if err is None:
            ok = True
            break
        win.dialogs.message(err, "Data Table")
    d.d.destroy()
    return ok


# ---------------------------------------------------------------- Scenario Manager
def _author(win):
    try:
        return win.app.user_name()
    except Exception:
        return ""


class ScenarioEdit:
    """Add Scenario / Edit Scenario, then Scenario Values."""
    def __init__(self, win, sc=None):
        self.win = win
        self.sc = sc
        self.d, grid = _dialog(win, "Edit Scenario" if sc else "Add Scenario")
        self.name = _entry("scenario-name", sc["name"] if sc else "")
        cells = ",".join(abs_ref((r, c, r, c)) for r, c, _v in sc["cells"]) if sc else _sel_ref(win)
        self.cells = _entry("changing-cells", cells)
        today = datetime.date.today().strftime("%m/%d/%Y")
        if sc:
            comment = sc["comment"]
            if sc["comment"] and "Modified by" not in sc["comment"]:
                comment = sc["comment"] + " Modified by %s on %s" % (_author(win), today)
        else:
            comment = "Created by %s on %s" % (_author(win), today)
        self.comment = Gtk.TextView()
        self.comment.set_name("comment")
        self.comment.set_size_request(260, 60)
        self.comment.get_buffer().set_text(comment)
        self.locked = Gtk.CheckButton(label="Prevent changes")
        self.locked.set_active(sc["locked"] if sc else True)
        self.hidden = Gtk.CheckButton(label="Hide")
        self.hidden.set_active(sc["hidden"] if sc else False)
        _row(grid, 0, "Scenario name:", self.name)
        _row(grid, 1, "Changing cells:", self.cells)
        grid.attach(Gtk.Label(label="Ctrl+click cells to select non-adjacent changing cells.", xalign=0), 1, 2, 1, 1)
        _row(grid, 3, "Comment:", self.comment)
        grid.attach(Gtk.Label(label="Protection", xalign=0), 0, 4, 1, 1)
        grid.attach(self.locked, 1, 4, 1, 1)
        grid.attach(self.hidden, 1, 5, 1, 1)
        self.d.show_all()

    def cell_list(self):
        refs = parse_refs(self.cells.get_text())
        if not refs:
            return None
        out = []
        for r1, c1, r2, c2 in refs:
            for r in range(r1, r2 + 1):
                for c in range(c1, c2 + 1):
                    if (r, c) not in out:
                        out.append((r, c))
        return out if len(out) <= 32 else None

    def entry(self, values):
        b = self.comment.get_buffer()
        return {"name": self.name.get_text().strip(), "comment": b.get_text(b.get_start_iter(), b.get_end_iter(), False),
                "user": _author(self.win), "locked": self.locked.get_active(), "hidden": self.hidden.get_active(),
                "cells": [(r, c, v) for (r, c), v in values]}


class ScenarioValues:
    def __init__(self, win, cells, start):
        self.win = win
        self.d, grid = _dialog(win, "Scenario Values")
        grid.attach(Gtk.Label(label="Enter values for each of the changing cells.", xalign=0), 0, 0, 2, 1)
        self.entries = []
        for k, (r, c) in enumerate(cells):
            e = _entry("value%d" % k, start.get((r, c), ""))
            e.set_width_chars(16)
            _row(grid, k + 1, "%d:  %s" % (k + 1, abs_ref((r, c, r, c))), e)
            self.entries.append(((r, c), e))
        self.d.show_all()

    def values(self):
        return [(rc, e.get_text().strip().lstrip("=")) for rc, e in self.entries]


class ScenarioManager:
    def __init__(self, win):
        self.win = win
        self.d = Gtk.Dialog(title="Scenario Manager", transient_for=win, modal=True)
        self.d.add_button("Show", 1)
        self.d.add_button("Close", Gtk.ResponseType.CLOSE)
        body = Gtk.Box(spacing=10)
        body.set_border_width(12)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        left.pack_start(Gtk.Label(label="Scenarios:", xalign=0), False, False, 0)
        self.store = Gtk.ListStore(str)
        self.view = Gtk.TreeView(model=self.store)
        self.view.set_headers_visible(False)
        self.view.set_name("scenarios")
        self.view.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        self.view.get_selection().connect("changed", lambda *_: self._details())
        self.view.connect("row-activated", lambda *_: self.show())
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(240, 160)
        sw.add(self.view)
        left.pack_start(sw, True, True, 0)
        self.empty = Gtk.Label(label="No Scenarios defined. Choose Add to add scenarios.")
        left.pack_start(self.empty, False, False, 0)
        body.pack_start(left, True, True, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        for label, cb in (("Add...", self.add), ("Delete", self.delete), ("Edit...", self.edit),
                          ("Merge...", self.merge), ("Summary...", self.summary)):
            b = Gtk.Button(label=label)
            b.connect("clicked", lambda _b, cb=cb: cb())
            right.pack_start(b, False, False, 0)
        body.pack_start(right, False, False, 0)
        area = self.d.get_content_area()
        area.pack_start(body, True, True, 0)
        info = Gtk.Grid(column_spacing=8, row_spacing=2)
        info.set_border_width(12)
        info.attach(Gtk.Label(label="Changing cells:", xalign=0), 0, 0, 1, 1)
        self.cells_lbl = Gtk.Label(xalign=0)
        info.attach(self.cells_lbl, 1, 0, 1, 1)
        info.attach(Gtk.Label(label="Comment:", xalign=0), 0, 1, 1, 1)
        self.comment_lbl = Gtk.Label(xalign=0)
        self.comment_lbl.set_line_wrap(True)
        info.attach(self.comment_lbl, 1, 1, 1, 1)
        area.pack_start(info, False, False, 0)
        self.d.connect("response", self._response)
        self.refresh()
        self.d.show_all()
        self.empty.set_visible(not len(self.store))

    @property
    def s(self):
        return self.win.grid.sheet

    def list(self):
        return self.win.book.scenarios(self.s)

    def refresh(self, select=None):
        self.store.clear()
        for sc in self.list():
            self.store.append([sc["name"]])
        if len(self.store):
            self.view.get_selection().select_path(Gtk.TreePath(min(select or 0, len(self.store) - 1)))
        self.empty.set_visible(not len(self.store))
        self._details()

    def current(self):
        m, it = self.view.get_selection().get_selected()
        return m.get_path(it).get_indices()[0] if it is not None else None

    def _details(self):
        i = self.current()
        lst = self.list()
        if i is None or i >= len(lst):
            self.cells_lbl.set_text("")
            self.comment_lbl.set_text("")
            return
        sc = lst[i]
        self.cells_lbl.set_text(",".join(abs_ref((r, c, r, c)) for r, c, _ in sc["cells"]))
        self.comment_lbl.set_text(sc["comment"])

    def _response(self, d, resp):
        if resp == 1:
            self.show()
            return
        d.destroy()

    def save(self, lst, label):
        try:
            self.win.book.set_scenarios(self.s, lst, label)
        except ValueError as e:
            self.win.dialogs.message(str(e), "Scenario Manager")
            return False
        self.win.update_title()
        return True

    def add(self, name=None, cells=None, values=None, edit_index=None, run=True):
        """Add (or Edit) a scenario; the arguments let the audits skip the dialogs."""
        w = self.win
        lst = self.list()
        sc = lst[edit_index] if edit_index is not None else None
        e = ScenarioEdit(w, sc)
        if name is not None:
            e.name.set_text(name)
        if cells is not None:
            e.cells.set_text(cells)
        while True:
            if run and e.d.run() != Gtk.ResponseType.OK:
                e.d.destroy()
                return False
            nm = e.name.get_text().strip()
            cl = e.cell_list()
            if not nm:
                w.dialogs.message("Enter a name for the scenario.", "Add Scenario")
            elif any(x["name"].lower() == nm.lower() for k, x in enumerate(lst) if k != edit_index):
                w.dialogs.message("Scenario name already exists. Enter a unique scenario name.", "Add Scenario")
            elif cl is None:
                w.dialogs.message("Changing cell reference is not valid. Up to 32 changing cells are allowed.", "Add Scenario")
            else:
                break
            if not run:
                e.d.destroy()
                return False
        e.d.hide()
        start = {}
        if sc:
            start = {(r, c): v for r, c, v in sc["cells"]}
        for r, c in cl:
            if (r, c) not in start:
                start[(r, c)] = w.book.input(self.s, r, c).lstrip("=") if not w.book.is_formula(self.s, r, c) else _num_text(w.book.value(self.s, r, c))
        sv = ScenarioValues(w, cl, start)
        if values is not None:
            for (_rc, ent), v in zip(sv.entries, values):
                ent.set_text(str(v))
        ok = (not run) or sv.d.run() == Gtk.ResponseType.OK
        vals = sv.values()
        sv.d.destroy()
        entry = e.entry(vals)
        e.d.destroy()
        if not ok:
            return False
        if edit_index is not None:
            lst[edit_index] = entry
        else:
            lst.append(entry)
        if self.save(lst, "Edit Scenario" if edit_index is not None else "Add Scenario"):
            self.refresh(edit_index if edit_index is not None else len(lst) - 1)
            return True
        return False

    def edit(self, **kw):
        i = self.current()
        if i is None:
            return False
        if self.list()[i]["locked"] and self.win.book.sheet_protected(self.s) if hasattr(self.win.book, "sheet_protected") else False:
            self.win.dialogs.message("The scenario is protected.", "Scenario Manager")
            return False
        return self.add(edit_index=i, **kw)

    def delete(self):
        i = self.current()
        if i is None:
            return False
        lst = self.list()
        del lst[i]
        ok = self.save(lst, "Delete Scenario")
        self.refresh(i)
        return ok

    def show(self, i=None):
        i = self.current() if i is None else i
        if i is None:
            return False
        try:
            self.win.book.show_scenario(self.s, i)
        except ValueError as e:
            self.win.dialogs.message(str(e), "Scenario Manager")
            return False
        self.win.grid.refresh()
        self.win.update_title()
        self.win.on_selection_changed()
        return True

    def merge(self, from_sheet=None):
        """Merge Scenarios: copy another sheet's scenarios here."""
        w = self.win
        b = w.book
        if from_sheet is None:
            d, grid = _dialog(w, "Merge Scenarios")
            combo = Gtk.ComboBoxText()
            for i in range(b.sheet_count):
                if i != self.s:
                    combo.append(str(i), b.sheet_info(i)["name"])
            combo.set_active(0)
            _row(grid, 0, "Merge scenarios from sheet:", combo)
            d.show_all()
            ok = d.run() == Gtk.ResponseType.OK
            sid = combo.get_active_id()
            d.destroy()
            if not ok or sid is None:
                return False
            from_sheet = int(sid)
        lst = self.list()
        names = {x["name"].lower() for x in lst}
        for sc in b.scenarios(from_sheet):
            nm = sc["name"]
            k = 2
            while nm.lower() in names:
                nm = "%s %d" % (sc["name"], k)
                k += 1
            sc = dict(sc, name=nm)
            names.add(nm.lower())
            lst.append(sc)
        ok = self.save(lst, "Merge Scenarios")
        self.refresh()
        return ok

    def summary(self, result_cells=None, run=True):
        """Scenario Summary: a new sheet with each scenario's changing and result cells."""
        w = self.win
        if result_cells is None:
            d, grid = _dialog(w, "Scenario Summary")
            r1 = Gtk.RadioButton.new_with_label(None, "Scenario summary")
            r2 = Gtk.RadioButton.new_with_label_from_widget(r1, "Scenario PivotTable report")
            grid.attach(Gtk.Label(label="Report type", xalign=0), 0, 0, 2, 1)
            grid.attach(r1, 0, 1, 2, 1)
            grid.attach(r2, 0, 2, 2, 1)
            ent = _entry("result-cells", _sel_ref(w))
            _row(grid, 3, "Result cells:", ent)
            d.show_all()
            ok = d.run() == Gtk.ResponseType.OK
            pivot = r2.get_active()
            result_cells = ent.get_text()
            d.destroy()
            if not ok:
                return False
        else:
            pivot = False
        refs = parse_refs(result_cells) if result_cells.strip() else []
        if refs is None:
            w.dialogs.message("Result cells reference is not valid.", "Scenario Summary")
            return False
        results = [(r, c) for r1, c1, r2, c2 in refs for r in range(r1, r2 + 1) for c in range(c1, c2 + 1)]
        if pivot:
            if not results:
                w.dialogs.message("A Scenario PivotTable report needs result cells.", "Scenario Summary")
                return False
            return make_pivot_report(w, self.s, results)
        return make_summary(w, self.s, results)


def make_summary(win, s, results):
    b = win.book
    lst = b.scenarios(s)
    if not lst:
        win.dialogs.message("No scenarios defined.", "Scenario Summary")
        return False
    changing = []
    for sc in lst:
        for r, c, _ in sc["cells"]:
            if (r, c) not in changing:
                changing.append((r, c))
    vals = b.scenario_results(s, results) if results else [[] for _ in lst]
    with b.group("Scenario Summary"):
        ns = b.insert_sheet(s, "Scenario Summary")
        put = lambda r, c, v: b.set_input(ns, r, c, v if isinstance(v, str) else _num_text(v))  # noqa: E731
        put(1, 1, "Scenario Summary")
        put(2, 3, "Current Values:")
        for k, sc in enumerate(lst):
            put(2, 4 + k, sc["name"])
        put(3, 1, "Changing Cells:")
        row = 4
        for r, c in changing:
            put(row, 2, abs_ref((r, c, r, c)))
            put(row, 3, b.input(s, r, c) if not b.is_formula(s, r, c) else b.value(s, r, c))
            for k, sc in enumerate(lst):
                v = {(rr, cc): x for rr, cc, x in sc["cells"]}.get((r, c))
                put(row, 4 + k, v if v is not None else (b.input(s, r, c) if not b.is_formula(s, r, c) else b.value(s, r, c)))
            row += 1
        if results:
            put(row, 1, "Result Cells:")
            row += 1
            for i, (r, c) in enumerate(results):
                put(row, 2, abs_ref((r, c, r, c)))
                cur = b.value(s, r, c)
                put(row, 3, cur if not isinstance(cur, str) else "'" + cur)
                for k in range(len(lst)):
                    v = vals[k][i]
                    put(row, 4 + k, v if not isinstance(v, str) else "'" + v)
                row += 1
        put(row, 1, "Notes:  Current Values column represents values of changing cells at")
        put(row + 1, 1, "time Scenario Summary Report was created.  Changing cells for each")
        put(row + 2, 1, "scenario are highlighted in gray.")
        b.restyle(ns, [(2, 1, 2, 3 + len(lst))], [{"font": {"bold": True}}], "Bold") if hasattr(b, "restyle") else None
    win.tabs.rebuild()
    win.grid.set_sheet(ns)
    win.sheet_changed()
    win.update_title()
    return ns


def make_pivot_report(win, s, results):
    """Scenario PivotTable report: the scenarios down the rows, a result cell's value in each
    column, in a PivotTable on a new sheet (its data kept on a hidden sheet of its own)."""
    b = win.book
    lst = b.scenarios(s)
    if not lst:
        win.dialogs.message("No scenarios defined.", "Scenario Summary")
        return False
    changing = []
    for sc in lst:
        for r, c, _ in sc["cells"]:
            if (r, c) not in changing:
                changing.append((r, c))
    rows = [r for r, _c in changing]
    cols = [c for _r, c in changing]
    span = (min(rows), min(cols), max(rows), max(cols))
    by = "%s by" % abs_ref(span) if len(changing) > 1 else "%s by" % abs_ref((rows[0], cols[0], rows[0], cols[0]))
    vals = b.scenario_results(s, results)
    heads = [by] + [abs_ref((r, c, r, c)) for r, c in results]
    with b.group("Scenario PivotTable Report"):
        name, k = "Scenario Data", 1
        while b.check_sheet_name(name, None):
            k += 1
            name = "Scenario Data (%d)" % k
        n = b.sheet_count
        ds = b.insert_sheet(n, name)
        b.set_sheet_state(ds, "veryHidden")
        for j, h in enumerate(heads):
            b.set_input(ds, 0, j, h)
        for i, sc in enumerate(lst):
            b.set_input(ds, i + 1, 0, sc["name"] if not re.match(r"^[-+=\d]", sc["name"]) else "'" + sc["name"])
            for j, v in enumerate(vals[i]):
                b.set_input(ds, i + 1, j + 1, v if isinstance(v, str) and v.startswith("#") else ("'" + v if isinstance(v, str) else _num_text(v)))
        rname, k = "Scenario PivotTable", 1
        while b.check_sheet_name(rname, None):
            k += 1
            rname = "Scenario PivotTable (%d)" % k
        dest = b.insert_sheet(s, rname)
        ds = ds + 1 if ds >= s else ds
        pi = b.create_pivot(ds, (0, 0, len(lst), len(heads) - 1), dest, (2, 0))
        b.set_pivot(pi, {"rows": [0], "values": [{"field": j, "agg": "sum", "name": heads[j]} for j in range(1, len(heads))]},
                    "Scenario PivotTable Report")
    win._after_structure(dest)
    win.grid.set_sheet(dest)
    win.update_title()
    return dest


def scenario_manager(win):
    m = ScenarioManager(win)
    m.d.run()
    try:
        m.d.destroy()
    except Exception:
        pass
    return m


# ---------------------------------------------------------------- Solver
RELS = ["<=", "=", ">=", "int", "bin", "dif"]
METHODS = [("grg", "GRG Nonlinear"), ("simplex", "Simplex LP"), ("evolutionary", "Evolutionary")]


class ConstraintDialog:
    def __init__(self, win, title="Add Constraint", c=None):
        self.win = win
        self.d = Gtk.Dialog(title=title, transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("Add", 2)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        g = Gtk.Grid(column_spacing=8, row_spacing=4)
        g.set_border_width(12)
        self.lhs = _entry("cell-reference", abs_ref(c[0]) if c else "")
        self.rel = Gtk.ComboBoxText()
        self.rel.set_name("relation")
        for r in RELS:
            self.rel.append(r, r)
        self.rel.set_active_id(c[1] if c else "<=")
        self.rhs = _entry("constraint", c[2] if c else "")
        self.rel.connect("changed", lambda *_: self._sync())
        g.attach(Gtk.Label(label="Cell Reference:", xalign=0), 0, 0, 1, 1)
        g.attach(Gtk.Label(label="Constraint:", xalign=0), 2, 0, 1, 1)
        g.attach(self.lhs, 0, 1, 1, 1)
        g.attach(self.rel, 1, 1, 1, 1)
        g.attach(self.rhs, 2, 1, 1, 1)
        self.d.get_content_area().pack_start(g, True, True, 0)
        self.d.show_all()
        self._sync()

    def _sync(self):
        r = self.rel.get_active_id()
        word = {"int": "integer", "bin": "binary", "dif": "AllDifferent"}.get(r)
        if word:
            self.rhs.set_text(word)
        self.rhs.set_sensitive(word is None)

    def constraint(self):
        """(lhs rect, rel, rhs) or an error message."""
        lhs = parse_ref(self.lhs.get_text())
        if lhs is None:
            return "Cell Reference is not valid."
        rel = self.rel.get_active_id()
        rhs = self.rhs.get_text().strip().lstrip("=")
        if rel in ("int", "bin", "dif"):
            return (lhs, rel, rel)
        if not rhs:
            return "Constraint must be a number, a simple reference, or a formula with a numeric value."
        try:
            float(rhs)
        except ValueError:
            if parse_ref(rhs) is None:
                return "Constraint must be a number, a simple reference, or a formula with a numeric value."
            r = parse_ref(rhs)
            rhs = abs_ref(r)
        return (lhs, rel, rhs)


class SolverDialog:
    """Solver Parameters."""
    def __init__(self, win):
        self.win = win
        b = win.book
        m = b.solver_model(win.grid.sheet)
        self.model = m
        self.d = Gtk.Dialog(title="Solver Parameters", transient_for=win, modal=True)
        self.d.add_button("Close", Gtk.ResponseType.CLOSE)
        self.d.add_button("Solve", Gtk.ResponseType.OK)
        self.d.set_default_response(Gtk.ResponseType.OK)
        g = Gtk.Grid(column_spacing=8, row_spacing=6)
        g.set_border_width(12)
        r, c = win.grid.sel.active
        obj = m["objective"]
        self.objective = _entry("objective", abs_ref((obj[0], obj[1], obj[0], obj[1])) if obj else abs_ref((r, c, r, c)))
        g.attach(Gtk.Label(label="Set Objective:", xalign=0), 0, 0, 1, 1)
        g.attach(self.objective, 1, 0, 3, 1)
        g.attach(Gtk.Label(label="To:", xalign=0), 0, 1, 1, 1)
        self.r_max = Gtk.RadioButton.new_with_label(None, "Max")
        self.r_min = Gtk.RadioButton.new_with_label_from_widget(self.r_max, "Min")
        self.r_val = Gtk.RadioButton.new_with_label_from_widget(self.r_max, "Value Of:")
        self.value = _entry("value-of", _num_text(m["value"]) if m["goal"] == "value" else "0")
        self.value.set_width_chars(8)
        {"max": self.r_max, "min": self.r_min, "value": self.r_val}[m["goal"]].set_active(True)
        row = Gtk.Box(spacing=10)
        for x in (self.r_max, self.r_min, self.r_val, self.value):
            row.pack_start(x, False, False, 0)
        g.attach(row, 1, 1, 3, 1)
        g.attach(Gtk.Label(label="By Changing Variable Cells:", xalign=0), 0, 2, 4, 1)
        self.vars = _entry("variable-cells", ",".join(abs_ref(v) for v in m["vars"]))
        g.attach(self.vars, 0, 3, 4, 1)
        g.attach(Gtk.Label(label="Subject to the Constraints:", xalign=0), 0, 4, 4, 1)
        self.store = Gtk.ListStore(str)
        self.constraints = list(m["constraints"])
        self.view = Gtk.TreeView(model=self.store)
        self.view.set_name("constraints")
        self.view.set_headers_visible(False)
        self.view.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(320, 120)
        sw.add(self.view)
        g.attach(sw, 0, 5, 3, 1)
        btns = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        for label, cb in (("Add", self.add), ("Change", self.change), ("Delete", self.delete),
                          ("Reset All", self.reset), ("Load/Save", self.load_save)):
            bt = Gtk.Button(label=label)
            bt.connect("clicked", lambda _b, cb=cb: cb())
            btns.pack_start(bt, False, False, 0)
        g.attach(btns, 3, 5, 1, 1)
        self.nonneg = Gtk.CheckButton(label="Make Unconstrained Variables Non-Negative")
        self.nonneg.set_active(m["nonneg"])
        g.attach(self.nonneg, 0, 6, 4, 1)
        g.attach(Gtk.Label(label="Select a Solving Method:", xalign=0), 0, 7, 1, 1)
        self.method = Gtk.ComboBoxText()
        self.method.set_name("method")
        for k, label in METHODS:
            self.method.append(k, label)
        self.method.set_active_id(m["method"])
        g.attach(self.method, 1, 7, 2, 1)
        opt = Gtk.Button(label="Options")
        opt.connect("clicked", lambda *_: self.options())
        g.attach(opt, 3, 7, 1, 1)
        help_ = Gtk.Label(xalign=0)
        help_.set_line_wrap(True)
        help_.set_max_width_chars(60)
        help_.set_text("Solving Method\nSelect the GRG Nonlinear engine for Solver Problems that are smooth nonlinear. "
                       "Select the LP Simplex engine for linear Solver Problems, and select the Evolutionary engine "
                       "for Solver problems that are non-smooth.")
        g.attach(help_, 0, 8, 4, 1)
        self.d.get_content_area().pack_start(g, True, True, 0)
        self._fill()
        self.d.show_all()

    def _fill(self):
        self.store.clear()
        for lhs, rel, rhs in sorted(self.constraints, key=lambda x: (x[0], x[1])):
            self.store.append(["%s %s %s" % (abs_ref(lhs), rel, {"int": "integer", "bin": "binary", "dif": "AllDifferent"}.get(rel, rhs))])

    def _sorted(self):
        return sorted(self.constraints, key=lambda x: (x[0], x[1]))

    def _current(self):
        m, it = self.view.get_selection().get_selected()
        return m.get_path(it).get_indices()[0] if it is not None else None

    def add(self, c=None):
        """Add Constraint (c: (lhs rect, rel, rhs) skips the dialog)."""
        if c is not None:
            self.constraints.append(c)
            self._fill()
            return True
        while True:
            cd = ConstraintDialog(self.win)
            resp = cd.d.run()
            got = cd.constraint() if resp in (2, Gtk.ResponseType.OK) else None
            cd.d.destroy()
            if got is None:
                return False
            if isinstance(got, str):
                self.win.dialogs.message(got, "Solver")
                continue
            self.constraints.append(got)
            self._fill()
            if resp != 2:
                return True

    def change(self):
        i = self._current()
        if i is None:
            return False
        cur = self._sorted()[i]
        cd = ConstraintDialog(self.win, "Change Constraint", cur)
        resp = cd.d.run()
        got = cd.constraint() if resp in (2, Gtk.ResponseType.OK) else None
        cd.d.destroy()
        if got is None or isinstance(got, str):
            return False
        self.constraints.remove(cur)
        self.constraints.append(got)
        self._fill()
        return True

    def delete(self):
        i = self._current()
        if i is None:
            return False
        self.constraints.remove(self._sorted()[i])
        self._fill()
        return True

    def reset(self):
        self.constraints = []
        self.objective.set_text("")
        self.vars.set_text("")
        self.r_max.set_active(True)
        self.nonneg.set_active(True)
        self.method.set_active_id("grg")
        self._fill()

    def load_save(self):
        self.win.dialogs.info("Load/Save Model isn't available in %s: the model is saved with the sheet." % APP_NAME, "Solver")

    def options(self):
        m = self.model
        d, grid = _dialog(self.win, "Options")
        prec = _entry("precision", _num_text(m["precision"]))
        gap = _entry("integer-optimality", _num_text(m["int_gap"]))
        it = _entry("iterations", str(m["max_iter"]))
        _row(grid, 0, "Constraint Precision:", prec)
        _row(grid, 1, "Integer Optimality (%):", gap)
        _row(grid, 2, "Max Iterations:", it)
        d.show_all()
        if d.run() == Gtk.ResponseType.OK:
            try:
                m["precision"] = float(prec.get_text())
                m["int_gap"] = float(gap.get_text())
                m["max_iter"] = int(it.get_text())
            except ValueError:
                self.win.dialogs.message("Enter numbers for the options.", "Solver")
        d.destroy()

    def model_dict(self):
        """The model as entered, or an error message."""
        m = dict(self.model)
        o = self.objective.get_text().strip()
        if o:
            r = parse_ref(o)
            if r is None or r[:2] != r[2:]:
                return "Objective Cell must be a single cell on the active sheet."
            m["objective"] = r[:2]
        else:
            m["objective"] = None
        m["goal"] = "max" if self.r_max.get_active() else "min" if self.r_min.get_active() else "value"
        if m["goal"] == "value":
            try:
                m["value"] = float(self.value.get_text())
            except ValueError:
                return "Value Of must be a number."
        refs = parse_refs(self.vars.get_text())
        if not refs:
            return "By Changing Variable Cells must be cells on the active sheet."
        m["vars"] = refs
        m["constraints"] = self._sorted()
        m["nonneg"] = self.nonneg.get_active()
        m["method"] = self.method.get_active_id()
        return m

    def solve(self):
        """Solve; returns the result (dict) or an error message. The model is saved with the sheet."""
        m = self.model_dict()
        if isinstance(m, str):
            return m
        w = self.win
        try:
            w.book.set_solver_model(w.grid.sheet, m)
        except ValueError as e:
            return str(e)
        res = w.book.solve(w.grid.sheet, m)
        res["model"] = m
        return res


class SolverResults:
    def __init__(self, win, res):
        self.win = win
        self.res = res
        self.d = Gtk.Dialog(title="Solver Results", transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(12)
        self.msg = Gtk.Label(label=res["message"], xalign=0)
        self.msg.set_line_wrap(True)
        self.msg.set_max_width_chars(50)
        box.pack_start(self.msg, False, False, 0)
        self.keep = Gtk.RadioButton.new_with_label(None, "Keep Solver Solution")
        self.restore = Gtk.RadioButton.new_with_label_from_widget(self.keep, "Restore Original Values")
        if not res["found"]:
            self.restore.set_active(True)
        box.pack_start(self.keep, False, False, 0)
        box.pack_start(self.restore, False, False, 0)
        box.pack_start(Gtk.Label(label="Reports", xalign=0), False, False, 4)
        self.answer = Gtk.CheckButton(label="Answer")
        self.answer.set_sensitive(res["found"])
        box.pack_start(self.answer, False, False, 0)
        self.back = Gtk.CheckButton(label="Return to Solver Parameters Dialog")
        box.pack_start(self.back, False, False, 0)
        self.d.get_content_area().pack_start(box, True, True, 0)
        self.d.show_all()

    def finish(self, ok=True):
        """OK: keep the solution (or not), make the reports."""
        w = self.win
        res = self.res
        s = w.grid.sheet
        if ok and self.keep.get_active() and res["found"]:
            with w.book.group("Solver"):
                for (r, c), v in zip(res["cells"], res["x"]):
                    w.book.set_input(s, r, c, _num_text(round(v, 12)))
            w.grid.refresh()
            w.update_title()
        if ok and self.answer.get_active() and res["found"]:
            answer_report(w, s, res)


def answer_report(win, s, res):
    b = win.book
    m = res["model"]
    name = "Answer Report 1"
    k = 1
    names = {b.sheet_info(i)["name"] for i in range(b.sheet_count)}
    while name in names:
        k += 1
        name = "Answer Report %d" % k
    sheet = b.sheet_info(s)["name"]
    with b.group("Answer Report"):
        ns = b.insert_sheet(s, name)
        src = s + 1
        put = lambda r, c, v: b.set_input(ns, r, c, v if isinstance(v, str) else _num_text(v))  # noqa: E731
        put(0, 0, "%s Answer Report" % APP_NAME)
        put(1, 0, "Worksheet: [%s]%s" % (win.doc_name(), sheet))
        put(2, 0, "Report Created: %s" % datetime.datetime.now().strftime("%m/%d/%Y %I:%M:%S %p"))
        put(3, 0, "Result: " + res["message"])
        put(4, 0, "Solver Engine")
        put(5, 1, "Engine: " + dict(METHODS)[m["method"]])
        row = 7
        put(row, 0, "Objective Cell (%s)" % {"max": "Max", "min": "Min", "value": "Value Of"}[m["goal"]])
        put(row + 1, 1, "Cell")
        put(row + 1, 2, "Name")
        put(row + 1, 3, "Original Value")
        put(row + 1, 4, "Final Value")
        if m["objective"]:
            r, c = m["objective"]
            put(row + 2, 1, abs_ref((r, c, r, c)))
            put(row + 2, 3, res["objective_start"])
            put(row + 2, 4, res["objective"])
        row += 4
        put(row, 0, "Variable Cells")
        for k2, h in enumerate(("Cell", "Name", "Original Value", "Final Value", "Integer")):
            put(row + 1, 1 + k2, h)
        ints = set()
        for lhs, rel, _rhs in m["constraints"]:
            if rel in ("int", "bin"):
                for rr in range(lhs[0], lhs[2] + 1):
                    for cc in range(lhs[1], lhs[3] + 1):
                        ints.add((rr, cc))
        for i, ((r, c), x0, x) in enumerate(zip(res["cells"], res["start"], res["x"])):
            put(row + 2 + i, 1, abs_ref((r, c, r, c)))
            put(row + 2 + i, 3, x0)
            put(row + 2 + i, 4, round(x, 12))
            put(row + 2 + i, 5, "Integer" if (r, c) in ints else "Contin")
        row += 3 + len(res["cells"])
        put(row, 0, "Constraints")
        for k2, h in enumerate(("Cell", "Name", "Cell Value", "Formula", "Status", "Slack")):
            put(row + 1, 1 + k2, h)
        row += 2
        for lhs, rel, rhs in m["constraints"]:
            if rel in ("int", "bin", "dif"):
                continue
            for rr in range(lhs[0], lhs[2] + 1):
                for cc in range(lhs[1], lhs[3] + 1):
                    put(row, 1, abs_ref((rr, cc, rr, cc)))
                    val = b.value(src, rr, cc)
                    put(row, 3, val if not isinstance(val, str) else "'" + val)
                    put(row, 4, "'%s%s%s" % (abs_ref((rr, cc, rr, cc)), rel, rhs))
                    try:
                        target = float(rhs)
                    except ValueError:
                        rr2 = parse_ref(rhs)
                        tv = b.value(src, rr2[0], rr2[1]) if rr2 else 0
                        target = tv if isinstance(tv, (int, float)) else 0
                    slack = abs((val if isinstance(val, (int, float)) else 0) - target)
                    put(row, 5, "Binding" if slack < 1e-7 else "Not Binding")
                    put(row, 6, slack)
                    row += 1
    # the report goes in before the sheet; the sheet stays the one shown
    win.grid.set_sheet(s + 1)
    win.tabs.rebuild()
    win.sheet_changed()
    return ns


def solver(win):
    d = SolverDialog(win)
    while True:
        resp = d.d.run()
        res = d.solve() if resp == Gtk.ResponseType.OK else None
        if res is None:
            d.d.destroy()
            return False
        if isinstance(res, str):
            win.dialogs.message(res, "Solver")
            continue
        d.d.hide()
        r = SolverResults(win, res)
        ok = r.d.run() == Gtk.ResponseType.OK
        back = r.back.get_active()
        r.finish(ok)
        r.d.destroy()
        if not back:
            d.d.destroy()
            return ok
        d.d.show()


# ---------------------------------------------------------------- Forecast Sheet
class ForecastDialog:
    """Create Forecast Worksheet: from a timeline and values, a new sheet with the forecast, its
    confidence bounds and a chart."""
    def __init__(self, win):
        self.win = win
        self.ok = False
        g = win.grid
        b = win.book
        s = g.sheet
        r1, c1, r2, c2 = g.sel.ranges[-1]
        r1, c1, r2, c2 = min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2)
        if (r1, c1) == (r2, c2):
            r1, c1, r2, c2 = b.current_region(s, r1, c1)
        header = r1 if not isinstance(b.value(s, r1, c1 + 1), (int, float)) else None
        top = r1 + 1 if header is not None else r1
        self.timeline = (top, c1, r2, c1)
        self.values = (top, c1 + 1, r2, c1 + 1)
        self.header = b.input(s, header, c1 + 1) if header is not None else "Values"
        self.time_header = b.input(s, header, c1) if header is not None else "Timeline"
        self.d = Gtk.Dialog(title="Create Forecast Worksheet", transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("Create", Gtk.ResponseType.OK)
        grid = Gtk.Grid(column_spacing=10, row_spacing=6)
        grid.set_border_width(12)
        self.kind = Gtk.ComboBoxText()
        self.kind.append("line", "Line")
        self.kind.append("col", "Column")
        self.kind.set_active_id("line")
        _row(grid, 0, "Chart type:", self.kind)
        ts = [b.value(s, r, c1) for r in range(top, r2 + 1)]
        nums = [t for t in ts if isinstance(t, (int, float))]
        step = (nums[-1] - nums[0]) / max(len(nums) - 1, 1) if len(nums) > 1 else 1
        self.end = _entry("forecast-end", _num_text(round(nums[-1] + step * max(5, len(nums) // 3), 10)) if nums else "")
        _row(grid, 1, "Forecast End:", self.end)
        self.start = _entry("forecast-start", _num_text(nums[-1]) if nums else "")
        _row(grid, 2, "Forecast Start:", self.start)
        self.conf = Gtk.SpinButton.new_with_range(1, 99, 1)
        self.conf.set_value(95)
        self.conf_on = Gtk.CheckButton(label="Confidence Interval (%)")
        self.conf_on.set_active(True)
        grid.attach(self.conf_on, 0, 3, 1, 1)
        grid.attach(self.conf, 1, 3, 1, 1)
        self.auto = Gtk.RadioButton.new_with_label(None, "Detect Automatically")
        self.manual = Gtk.RadioButton.new_with_label_from_widget(self.auto, "Set Manually")
        self.season = Gtk.SpinButton.new_with_range(2, 8760, 1)
        sb = Gtk.Box(spacing=6)
        for x in (self.auto, self.manual, self.season):
            sb.pack_start(x, False, False, 0)
        _row(grid, 4, "Seasonality:", sb)
        self.stats = Gtk.CheckButton(label="Include forecast statistics")
        grid.attach(self.stats, 0, 5, 2, 1)
        self.t_ent = _entry("timeline-range", abs_ref(self.timeline))
        self.v_ent = _entry("values-range", abs_ref(self.values))
        _row(grid, 6, "Timeline Range:", self.t_ent)
        _row(grid, 7, "Values Range:", self.v_ent)
        self.fill = Gtk.ComboBoxText()
        self.fill.append("1", "Interpolation")
        self.fill.append("0", "Zeros")
        self.fill.set_active_id("1")
        _row(grid, 8, "Fill Missing Points Using:", self.fill)
        self.agg = Gtk.ComboBoxText()
        for k, label in enumerate(("Average", "Count", "CountA", "Max", "Median", "Min", "Sum"), start=1):
            self.agg.append(str(k), label)
        self.agg.set_active_id("1")
        _row(grid, 9, "Aggregate Duplicates Using:", self.agg)
        self.d.get_content_area().pack_start(grid, True, True, 0)
        self.d.show_all()

    def apply(self):
        """Make the forecast sheet; its index, or an error message."""
        w = self.win
        b = w.book
        s = w.grid.sheet
        tl = parse_ref(self.t_ent.get_text())
        vl = parse_ref(self.v_ent.get_text())
        if tl is None or vl is None or tl[2] - tl[0] != vl[2] - vl[0] or tl[1] != tl[3] or vl[1] != vl[3]:
            return "The timeline range and the values range must be the same size."
        try:
            end = float(self.end.get_text())
            start = float(self.start.get_text())
        except ValueError:
            return "Enter a forecast end (a number or a date in the timeline's terms)."
        ts = [b.value(s, r, tl[1]) for r in range(tl[0], tl[2] + 1)]
        if not all(isinstance(t, (int, float)) for t in ts) or len(ts) < 3:
            return "The timeline range must hold numbers or dates."
        steps = sorted({round(ts[k + 1] - ts[k], 9) for k in range(len(ts) - 1)})
        step = steps[0] if steps else 1
        if step <= 0:
            return "The timeline must have consistent steps."
        if end <= ts[-1]:
            return "The forecast end must be after the last point of the timeline."
        sheet = b.sheet_info(s)["name"]
        q = sheet if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", sheet) else "'%s'" % sheet.replace("'", "''")
        vref = "%s!%s" % (q, abs_ref(vl))
        tref = "%s!%s" % (q, abs_ref(tl))
        season = "1" if self.auto.get_active() else str(int(self.season.get_value()))
        opts = ",%s,%s,%s" % (season, self.fill.get_active_id(), self.agg.get_active_id())
        conf = self.conf.get_value() / 100.0 if self.conf_on.get_active() else None
        tfmt = b.style_spec(s, tl[0], tl[1]).get("numfmt", "General")
        data = [(b.value(s, r, tl[1]), b.value(s, r, vl[1])) for r in range(tl[0], tl[2] + 1)]
        with b.group("Forecast Sheet"):
            # the forecast sheet goes in before the data's sheet, as in the desktop spreadsheet
            ns = b.insert_sheet(s, None)
            put = lambda r, c, v: b.set_input(ns, r, c, v if isinstance(v, str) else _num_text(v))  # noqa: E731
            heads = [self.time_header, self.header, "Forecast(%s)" % self.header]
            if conf is not None:
                heads += ["Lower Confidence Bound(%s)" % self.header, "Upper Confidence Bound(%s)" % self.header]
            for k, h in enumerate(heads):
                put(0, k, h)
            row = 1
            for t0, v in data:
                put(row, 0, t0)
                if v is not None:
                    put(row, 1, v if not isinstance(v, str) else "'" + v)
                row += 1
            last = row - 1
            # the forecast joins the data at its last point
            put(last, 2, "=B%d" % (last + 1))
            if conf is not None:
                put(last, 3, "=B%d" % (last + 1))
                put(last, 4, "=B%d" % (last + 1))
            nexts = _next_points(ts, step, end)
            first_f = row
            for t in nexts:
                put(row, 0, t)
                put(row, 2, "=FORECAST.ETS(A%d,%s,%s%s)" % (row + 1, vref, tref, opts))
                if conf is not None:
                    ci = "FORECAST.ETS.CONFINT(A%d,%s,%s,%s%s)" % (row + 1, vref, tref, _num_text(conf), opts)
                    put(row, 3, "=C%d-%s" % (row + 1, ci))
                    put(row, 4, "=C%d+%s" % (row + 1, ci))
                row += 1
            ncols = len(heads)
            if tfmt and tfmt != "General":
                b.restyle(ns, [(1, 0, row - 1, 0)], [{"numfmt": tfmt}], "Number Format") if hasattr(b, "restyle") else None
            try:
                b.create_table(ns, (0, 0, row - 1, ncols - 1), True, "TableStyleMedium2")
            except ValueError:
                pass
            if self.stats.get_active():
                labels = [("Alpha", 1), ("Beta", 2), ("Gamma", 3), ("MASE", 4), ("SMAPE", 5), ("MAE", 6), ("RMSE", 7)]
                put(0, ncols + 1, "Statistic")
                put(0, ncols + 2, "Value")
                for k, (lab, code) in enumerate(labels, start=1):
                    put(k, ncols + 1, lab)
                    put(k, ncols + 2, "=FORECAST.ETS.STAT(%s,%s,%d%s)" % (vref, tref, code, opts))
            b.autofit_cols(ns, 0, ncols - 1, None)
            try:
                kind = self.kind.get_active_id()
                nq = b.sheet_info(ns)["name"]
                nq = nq if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", nq) else "'%s'" % nq.replace("'", "''")
                cat = "%s!$A$2:$A$%d" % (nq, row)
                series = []
                for k in range(1, ncols):
                    col = "ABCDE"[k]
                    series.append({"name": "%s!$%s$1" % (nq, col), "cat": cat, "val": "%s!$%s$2:$%s$%d" % (nq, col, col, row),
                                   "kind": kind})
                b.add_chart_spec(ns, {"kind": kind, "title_mode": "none", "legend": "b", "series": series},
                                 b.col_x(ns, ncols + (4 if self.stats.get_active() else 1)), 20.0, 480.0, 288.0)
            except ValueError:
                pass
            _ = first_f
        w.tabs.rebuild()
        w.grid.set_sheet(ns)
        w.sheet_changed()
        w.update_title()
        return ns


def _next_points(ts, step, end):
    """The timeline after its last point up to the end: whole months on, when the dates step by
    months (or years); else the step."""
    out = []
    try:
        ds = [datetime.date(1899, 12, 30) + datetime.timedelta(days=int(t)) for t in ts]
    except (OverflowError, ValueError):
        ds = []
    monthly = None
    if ds and all(float(t).is_integer() for t in ts) and len({d.day for d in ds}) == 1:
        mi = [d.year * 12 + d.month for d in ds]
        diffs = {mi[k + 1] - mi[k] for k in range(len(mi) - 1)}
        if len(diffs) == 1 and min(diffs) > 0 and len({ts[k + 1] - ts[k] for k in range(len(ts) - 1)}) > 1:
            monthly = min(diffs)
    if monthly:
        last = ds[-1]
        k = 1
        while len(out) < 10000:
            m = last.year * 12 + last.month - 1 + monthly * k
            d = datetime.date(m // 12, m % 12 + 1, min(last.day, 28))
            t = (d - datetime.date(1899, 12, 30)).days
            if t > end + 1e-9:
                break
            out.append(t)
            k += 1
        return out
    t = ts[-1] + step
    while t <= end + 1e-9 and len(out) < 10000:
        out.append(t)
        t += step
    return out


def forecast_sheet(win):
    try:
        d = ForecastDialog(win)
    except Exception as e:
        win.dialogs.message("Select the timeline and the values to forecast. (%s)" % e, "Forecast Sheet")
        return False
    ok = False
    while True:
        if d.d.run() != Gtk.ResponseType.OK:
            break
        r = d.apply()
        if not isinstance(r, str):
            ok = True
            break
        win.dialogs.message(r, "Forecast Sheet")
    d.d.destroy()
    return ok
