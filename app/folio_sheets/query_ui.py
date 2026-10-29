"""The query editor (S3-05, Excel's Power Query editor): a query's rows shaped by a list of
applied steps (choose or remove columns, keep or remove rows, filter, sort, change type, rename,
replace values, split, group by, merge and append queries, an index column, first row as
headers), shown in a preview with the steps beside it. The steps are kept with the query and
run again on every Refresh."""
import re

from gi.repository import Gtk

from . import APP_NAME

STEP_NAMES = {
    "promote": "Promoted Headers", "demote": "Demoted Headers", "choose": "Removed Other Columns",
    "remove": "Removed Columns", "rename": "Renamed Columns", "type": "Changed Type", "filter": "Filtered Rows",
    "top": "Kept First Rows", "skip": "Removed Top Rows", "blank": "Removed Blank Rows",
    "dedupe": "Removed Duplicates", "sort": "Sorted Rows", "replace": "Replaced Value", "split": "Split Column",
    "group": "Grouped Rows", "merge": "Merged Queries", "append": "Appended Query", "index": "Added Index",
    "reorder": "Reordered Columns", "upper": "Uppercased Text", "lower": "Lowercased Text", "trim": "Trimmed Text",
}
FILTER_OPS = [("equals", "eq"), ("does not equal", "ne"), ("contains", "contains"), ("does not contain", "notcontains"),
              ("begins with", "begins"), ("ends with", "ends"), ("is greater than", "gt"),
              ("is greater than or equal to", "ge"), ("is less than", "lt"), ("is less than or equal to", "le"),
              ("is blank", "blank"), ("is not blank", "notblank")]
KINDS = [("Decimal Number", "number"), ("Whole Number", "whole"), ("Text", "text"), ("Date", "date"),
         ("True/False", "bool")]
AGGS = [("Count Rows", "count"), ("Sum", "sum"), ("Average", "avg"), ("Min", "min"), ("Max", "max"),
        ("Count Distinct Rows", "distinct")]


def _num(v):
    from .getdata_ui import _num as n
    x = n(v) if isinstance(v, str) else None
    if x is None:
        return None
    try:
        return float(x)
    except ValueError:
        return None


def _fmt(x):
    if x is None:
        return ""
    if isinstance(x, float) and x.is_integer() and abs(x) < 1e15:
        return str(int(x))
    return "%.15g" % x


def _col(res, name):
    try:
        return res.headers.index(name)
    except ValueError:
        low = [h.lower() for h in res.headers]
        if name.lower() in low:
            return low.index(name.lower())
        raise ValueError("The column '%s' of the table wasn't found." % name)


def step_label(st):
    return STEP_NAMES.get(st["op"], st["op"])


def apply_step(res, st, resolve=None):
    """One step on a Result (returns a new one). resolve(name) -> another query's Result."""
    from .getdata_ui import Result, _headers, _kind
    H, R, K = list(res.headers), [list(r) for r in res.rows], list(res.kinds)
    F = dict(res.formats)
    op = st["op"]
    if op == "promote":
        if not R:
            return res
        first = R.pop(0)
        H = _headers(first, len(H))
        K = [_kind([r[j] for r in R]) for j in range(len(H))]
        F = {}
    elif op == "demote":
        R.insert(0, H)
        H = ["Column%d" % (j + 1) for j in range(len(H))]
        K = ["text"] * len(H)
        F = {}
    elif op in ("choose", "remove", "reorder"):
        cols = [_col(res, c) for c in st["cols"]]
        keep = cols if op in ("choose", "reorder") else [j for j in range(len(H)) if j not in cols]
        if op == "reorder":
            keep = cols + [j for j in range(len(H)) if j not in cols]
        H = [H[j] for j in keep]
        K = [K[j] for j in keep]
        R = [[r[j] for j in keep] for r in R]
        F = {keep.index(j): f for j, f in res.formats.items() if j in keep}
    elif op == "rename":
        j = _col(res, st["col"])
        new = st["new"].strip() or H[j]
        if any(h.lower() == new.lower() for k, h in enumerate(H) if k != j):
            raise ValueError("The column '%s' already exists." % new)
        H[j] = new
    elif op == "type":
        j = _col(res, st["col"])
        kind = st["kind"]
        for r in R:
            v = r[j]
            if v == "":
                continue
            if kind in ("number", "whole"):
                x = _num(v)
                if x is None:
                    r[j] = "#VALUE!"
                else:
                    r[j] = _fmt(float(round(x)) if kind == "whole" else x)
            elif kind == "bool":
                r[j] = "TRUE" if v.strip().upper() in ("TRUE", "1", "YES") else "FALSE"
            elif kind == "text":
                r[j] = v
        K[j] = {"whole": "number"}.get(kind, kind)
        F.pop(j, None)
        if kind == "date":
            F[j] = "m/d/yyyy"
    elif op == "filter":
        j = _col(res, st["col"])
        o, val = st["cmp"], st.get("value", "")
        R = [r for r in R if _match(r[j], o, val)]
    elif op == "top":
        R = R[:int(st["n"])]
    elif op == "skip":
        R = R[int(st["n"]):]
    elif op == "blank":
        R = [r for r in R if any(v.strip() for v in r)]
    elif op == "dedupe":
        cols = [_col(res, c) for c in st.get("cols") or H]
        seen, out = set(), []
        for r in R:
            key = tuple(r[j].lower() for j in cols)
            if key not in seen:
                seen.add(key)
                out.append(r)
        R = out
    elif op == "sort":
        for name, asc in reversed(st["keys"]):
            j = _col(res, name)
            R.sort(key=lambda r: _sort_key(r[j]), reverse=not asc)
    elif op == "replace":
        j = _col(res, st["col"])
        find, repl = st.get("find", ""), st.get("with", "")
        whole = st.get("whole", False)
        for r in R:
            if whole:
                if r[j] == find:
                    r[j] = repl
            elif find:
                r[j] = r[j].replace(find, repl)
    elif op in ("upper", "lower", "trim"):
        j = _col(res, st["col"])
        for r in R:
            r[j] = r[j].upper() if op == "upper" else r[j].lower() if op == "lower" else r[j].strip()
    elif op == "split":
        j = _col(res, st["col"])
        d = st.get("delim", ",")
        mode = st.get("mode", "each")
        parts = []
        for r in R:
            v = r[j]
            if mode == "left":
                p = v.split(d, 1)
            elif mode == "right":
                p = v.rsplit(d, 1)
            else:
                p = v.split(d) if d else [v]
            parts.append(p)
        n = max([len(p) for p in parts] + [1])
        names = ["%s.%d" % (H[j], k + 1) for k in range(n)]
        H = H[:j] + names + H[j + 1:]
        R = [r[:j] + (p + [""] * (n - len(p))) + r[j + 1:] for r, p in zip(R, parts)]
        K = K[:j] + [_kind([p[k] if k < len(p) else "" for p in parts]) for k in range(n)] + K[j + 1:]
        F = {}
    elif op == "index":
        start = int(st.get("start", 1))
        H.append(st.get("name") or "Index")
        K.append("number")
        for i, r in enumerate(R):
            r.append(str(start + i))
    elif op == "group":
        keys = [_col(res, c) for c in st["by"]]
        aggs = st["aggs"]   # [(new name, agg, column or None)]
        groups, order = {}, []
        for r in R:
            k = tuple(r[j] for j in keys)
            if k not in groups:
                groups[k] = []
                order.append(k)
            groups[k].append(r)
        H = [res.headers[j] for j in keys] + [a[0] for a in aggs]
        K = [res.kinds[j] for j in keys] + ["number"] * len(aggs)
        R = []
        for k in order:
            rows = groups[k]
            out = list(k)
            for _name, agg, col in aggs:
                if agg == "count":
                    out.append(str(len(rows)))
                    continue
                if agg == "distinct":
                    out.append(str(len({tuple(r) for r in rows})))
                    continue
                j = _col(res, col)
                nums = [x for x in (_num(r[j]) for r in rows) if x is not None]
                if agg == "sum":
                    out.append(_fmt(sum(nums)))
                elif agg == "avg":
                    out.append(_fmt(sum(nums) / len(nums)) if nums else "")
                elif agg == "min":
                    out.append(_fmt(min(nums)) if nums else "")
                else:
                    out.append(_fmt(max(nums)) if nums else "")
            R.append(out)
        F = {}
    elif op == "append":
        for name in st["queries"]:
            other = resolve(name) if resolve else None
            if other is None:
                raise ValueError("The query '%s' wasn't found." % name)
            for h in other.headers:
                if h.lower() not in [x.lower() for x in H]:
                    H.append(h)
                    K.append("text")
                    for r in R:
                        r.append("")
            idx = [[x.lower() for x in other.headers].index(h.lower()) if h.lower() in [x.lower() for x in other.headers] else None
                   for h in H]
            for orow in other.rows:
                R.append([orow[i] if i is not None else "" for i in idx])
    elif op == "merge":
        other = resolve(st["query"]) if resolve else None
        if other is None:
            raise ValueError("The query '%s' wasn't found." % st["query"])
        j = _col(res, st["left"])
        k = [h.lower() for h in other.headers].index(st["right"].lower()) if st["right"].lower() in [h.lower() for h in other.headers] else None
        if k is None:
            raise ValueError("The column '%s' of %s wasn't found." % (st["right"], st["query"]))
        how = st.get("kind", "left")
        cols = [h for i, h in enumerate(other.headers) if i != k] if st.get("expand", True) else []
        prefix = st.get("prefix", st["query"])
        index = {}
        for orow in other.rows:
            index.setdefault(orow[k].lower(), []).append(orow)
        newh = ["%s.%s" % (prefix, h) for h in cols]
        sel = [other.headers.index(h) for h in cols]
        out = []
        matched = set()
        for r in R:
            hits = index.get(r[j].lower(), [])
            if hits:
                matched.add(r[j].lower())
            if how == "anti":
                if not hits:
                    out.append(r)
                continue
            if not hits:
                if how in ("left", "full"):
                    out.append(r + [""] * len(sel))
                continue
            for o in hits:
                out.append(r + [o[i] for i in sel])
        if how == "full":
            for key, rows in index.items():
                if key not in matched:
                    for o in rows:
                        blank = [""] * len(H)
                        blank[j] = o[k]
                        out.append(blank + [o[i] for i in sel])
        if how != "anti":
            H += newh
            K += [other.kinds[i] for i in sel]
        R = out
    else:
        raise ValueError("Unknown step: %s" % op)
    return Result(H, R, K, F)


def _sort_key(v):
    x = _num(v)
    if v.strip() == "":
        return (2, 0, "")
    return (0, x, "") if x is not None else (1, 0, v.lower())


def _match(v, op, val):
    if op == "blank":
        return v.strip() == ""
    if op == "notblank":
        return v.strip() != ""
    a, b = _num(v), _num(val)
    if op in ("gt", "ge", "lt", "le"):
        if a is None or b is None:
            x, y = v.lower(), val.lower()
        else:
            x, y = a, b
        return {"gt": x > y, "ge": x >= y, "lt": x < y, "le": x <= y}[op]
    lv, lw = v.lower(), val.lower()
    if op == "eq":
        return (a == b) if a is not None and b is not None else lv == lw
    if op == "ne":
        return not ((a == b) if a is not None and b is not None else lv == lw)
    if op == "contains":
        return lw in lv
    if op == "notcontains":
        return lw not in lv
    if op == "begins":
        return lv.startswith(lw)
    if op == "ends":
        return lv.endswith(lw)
    return True


def apply_steps(res, steps, resolve=None, upto=None):
    for st in steps[:upto] if upto is not None else steps:
        res = apply_step(res, st, resolve)
    return res


# ------------------------------------------------------------------ the editor
def _ask(parent, title, fields):
    """A small form: fields = [(label, kind, default, choices)]; kind entry|combo|check|spin.
    Returns the values or None."""
    d = Gtk.Dialog(title=title, transient_for=parent, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(row_spacing=6, column_spacing=10)
    grid.set_border_width(12)
    ws = []
    for i, (label, kind, default, choices) in enumerate(fields):
        grid.attach(Gtk.Label(label=label, xalign=0), 0, i, 1, 1)
        if kind == "combo":
            w = Gtk.ComboBoxText()
            for c in choices:
                w.append_text(c)
            w.set_active(choices.index(default) if default in choices else 0)
        elif kind == "check":
            w = Gtk.CheckButton()
            w.set_active(bool(default))
        elif kind == "spin":
            w = Gtk.SpinButton.new_with_range(0, 1048576, 1)
            w.set_value(default or 0)
        else:
            w = Gtk.Entry()
            w.set_text(default or "")
            w.set_activates_default(True)
        grid.attach(w, 1, i, 1, 1)
        ws.append((kind, w, choices))
    d.get_content_area().pack_start(grid, True, True, 0)
    d.show_all()
    r = d.run()
    out = []
    for kind, w, choices in ws:
        if kind == "combo":
            out.append(choices[max(0, w.get_active())])
        elif kind == "check":
            out.append(w.get_active())
        elif kind == "spin":
            out.append(int(w.get_value()))
        else:
            out.append(w.get_text())
    d.destroy()
    return out if r == Gtk.ResponseType.OK else None


class QueryEditor(Gtk.Dialog):
    """The query editor: the preview of the shaped rows, the commands, and the applied steps.
    Responses: OK = Close & Load, 1 = Close & Load To..., CANCEL = discard."""

    def __init__(self, win, q, base):
        super().__init__(title="%s - Query Editor" % q.get("name", "Query"), transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.set_default_size(980, 620)
        self.win, self.q, self.base = win, q, base
        self.q.setdefault("steps", [])
        self.current = None          # the selected column's name
        self.upto = None             # steps shown (None: all)
        area = self.get_content_area()
        area.set_spacing(6)
        area.set_border_width(8)
        bar = Gtk.FlowBox()
        bar.set_selection_mode(Gtk.SelectionMode.NONE)
        bar.set_max_children_per_line(30)
        self.buttons = {}
        for label, fn in (("Choose Columns", self.cmd_choose), ("Remove Columns", self.cmd_remove),
                          ("Keep Top Rows", self.cmd_top), ("Remove Top Rows", self.cmd_skip),
                          ("Remove Blank Rows", lambda: self.add_step({"op": "blank"})),
                          ("Remove Duplicates", lambda: self.add_step({"op": "dedupe", "cols": [self.current]} if self.current else {"op": "dedupe"})),
                          ("Sort Ascending", lambda: self._need() and self.add_step({"op": "sort", "keys": [(self.current, True)]})),
                          ("Sort Descending", lambda: self._need() and self.add_step({"op": "sort", "keys": [(self.current, False)]})),
                          ("Filter Rows", self.cmd_filter), ("Data Type", self.cmd_type),
                          ("Use First Row as Headers", lambda: self.add_step({"op": "promote"})),
                          ("Use Headers as First Row", lambda: self.add_step({"op": "demote"})),
                          ("Rename", self.cmd_rename), ("Replace Values", self.cmd_replace),
                          ("Split Column", self.cmd_split), ("Group By", self.cmd_group),
                          ("Trim", lambda: self._need() and self.add_step({"op": "trim", "col": self.current})),
                          ("UPPERCASE", lambda: self._need() and self.add_step({"op": "upper", "col": self.current})),
                          ("lowercase", lambda: self._need() and self.add_step({"op": "lower", "col": self.current})),
                          ("Index Column", lambda: self.add_step({"op": "index", "start": 1})),
                          ("Merge Queries", self.cmd_merge), ("Append Queries", self.cmd_append)):
            bt = Gtk.Button(label=label)
            bt.connect("clicked", lambda _b, f=fn: f())
            bar.add(bt)
            self.buttons[label] = bt
        area.pack_start(bar, False, False, 0)
        body = Gtk.Paned()
        self.view_sw = Gtk.ScrolledWindow()
        self.view_sw.set_hexpand(True)
        self.view_sw.set_vexpand(True)
        body.pack1(self.view_sw, True, False)
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        side.set_size_request(240, -1)
        side.pack_start(Gtk.Label(label="Query Settings", xalign=0), False, False, 0)
        side.pack_start(Gtk.Label(label="Name", xalign=0), False, False, 0)
        self.name = Gtk.Entry()
        self.name.set_text(q.get("name", "Query1"))
        side.pack_start(self.name, False, False, 0)
        side.pack_start(Gtk.Label(label="Applied Steps", xalign=0), False, False, 6)
        self.steps = Gtk.ListBox()
        self.steps.connect("row-selected", self._step_selected)
        ssw = Gtk.ScrolledWindow()
        ssw.set_vexpand(True)
        ssw.add(self.steps)
        side.pack_start(ssw, True, True, 0)
        dele = Gtk.Button(label="Delete Step")
        dele.connect("clicked", lambda *_: self.delete_step())
        side.pack_start(dele, False, False, 0)
        body.pack2(side, False, False)
        area.pack_start(body, True, True, 0)
        self.status = Gtk.Label(xalign=0)
        area.pack_start(self.status, False, False, 0)
        self.add_button("Close & Load", Gtk.ResponseType.OK)
        self.add_button("Close & Load To...", 1)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.refresh()
        self.show_all()

    # -------------------------------------------------- state
    def resolve(self, name):
        from . import getdata_ui
        if name.lower() == self.q.get("name", "").lower():
            return None
        q = getdata_ui.find_query(self.win.book, name)
        return getdata_ui.read(q, self.win.book) if q else None

    def result(self, upto=None):
        return apply_steps(self.base, self.q["steps"], self.resolve, upto)

    def refresh(self):
        for ch in self.steps.get_children():
            self.steps.remove(ch)
        for label in ["Source"] + [step_label(s) for s in self.q["steps"]]:
            row = Gtk.ListBoxRow()
            row.add(Gtk.Label(label=label, xalign=0))
            self.steps.add(row)
        self.steps.show_all()
        try:
            res = self.result(self.upto)
            err = None
        except ValueError as e:
            res, err = None, str(e)
        child = self.view_sw.get_child()
        if child is not None:
            self.view_sw.remove(child)
        if res is None:
            self.view_sw.add(Gtk.Label(label="Expression.Error: %s" % err))
            self.status.set_text("")
        else:
            self.view_sw.add(self._view(res))
            self.status.set_text("%d columns, %d rows" % (len(res.headers), len(res.rows)))
        self.view_sw.show_all()
        self.res = res

    def _view(self, res, limit=500):
        n = max(len(res.headers), 1)
        store = Gtk.ListStore(*([str] * n))
        for row in res.rows[:limit]:
            store.append([(row[j] if j < len(row) else "") for j in range(n)])
        tv = Gtk.TreeView(model=store)
        tv.set_grid_lines(Gtk.TreeViewGridLines.BOTH)
        for j, h in enumerate(res.headers):
            col = Gtk.TreeViewColumn(h + ("  ✓" if h == self.current else ""), Gtk.CellRendererText(), text=j)
            col.set_clickable(True)
            col.set_resizable(True)
            col.connect("clicked", lambda _c, name=h: self.select_column(name))
            tv.append_column(col)
        return tv

    def select_column(self, name):
        self.current = name
        self.refresh()

    def _step_selected(self, _lb, row):
        if row is None:
            return
        i = row.get_index()
        upto = None if i >= len(self.q["steps"]) else i
        if upto != self.upto:
            self.upto = upto
            self.refresh()

    def _need(self):
        if not self.current or self.res is None or self.current not in self.res.headers:
            self.win.dialogs.message("Select a column first: click its heading.", "Query Editor")
            return False
        return True

    def add_step(self, st):
        """Add a step after the last one (the preview shows them all again)."""
        try:
            apply_steps(self.base, self.q["steps"] + [st], self.resolve)
        except ValueError as e:
            self.win.dialogs.message(str(e), "Query Editor")
            return False
        self.q["steps"].append(st)
        self.upto = None
        if st["op"] == "rename" and self.current == st.get("col"):
            self.current = st["new"]
        self.refresh()
        return True

    def delete_step(self, k=None):
        if k is None:
            row = self.steps.get_selected_row()
            k = (row.get_index() - 1) if row is not None else len(self.q["steps"]) - 1
        if 0 <= k < len(self.q["steps"]):
            del self.q["steps"][k]
            self.upto = None
            self.refresh()

    def finish(self):
        self.q["name"] = self.name.get_text().strip() or self.q.get("name", "Query1")
        return self.q, self.result()

    # -------------------------------------------------- commands
    def _cols(self):
        return list(self.res.headers) if self.res else []

    def cmd_choose(self):
        cols = self._cols()
        d = Gtk.Dialog(title="Choose Columns", transient_for=self, modal=True)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        box = d.get_content_area()
        checks = []
        for c in cols:
            cb = Gtk.CheckButton(label=c)
            cb.set_active(True)
            box.pack_start(cb, False, False, 0)
            checks.append(cb)
        d.show_all()
        r = d.run()
        keep = [c for c, cb in zip(cols, checks) if cb.get_active()]
        d.destroy()
        if r == Gtk.ResponseType.OK and keep:
            self.add_step({"op": "choose", "cols": keep})

    def cmd_remove(self):
        if self._need():
            self.add_step({"op": "remove", "cols": [self.current]})

    def cmd_top(self):
        v = _ask(self, "Keep Top Rows", [("Number of rows", "spin", 10, None)])
        if v:
            self.add_step({"op": "top", "n": v[0]})

    def cmd_skip(self):
        v = _ask(self, "Remove Top Rows", [("Number of rows", "spin", 1, None)])
        if v:
            self.add_step({"op": "skip", "n": v[0]})

    def cmd_filter(self):
        if not self._need():
            return
        names = [n for n, _ in FILTER_OPS]
        v = _ask(self, "Filter Rows", [("Keep rows where '%s'" % self.current, "combo", names[0], names), ("Value", "entry", "", None)])
        if v:
            self.add_step({"op": "filter", "col": self.current, "cmp": dict(FILTER_OPS)[v[0]], "value": v[1]})

    def cmd_type(self):
        if not self._need():
            return
        names = [n for n, _ in KINDS]
        v = _ask(self, "Change Type", [("Data type of '%s'" % self.current, "combo", "Text", names)])
        if v:
            self.add_step({"op": "type", "col": self.current, "kind": dict(KINDS)[v[0]]})

    def cmd_rename(self):
        if not self._need():
            return
        v = _ask(self, "Rename", [("New name", "entry", self.current, None)])
        if v and v[0].strip():
            self.add_step({"op": "rename", "col": self.current, "new": v[0].strip()})

    def cmd_replace(self):
        if not self._need():
            return
        v = _ask(self, "Replace Values", [("Value To Find", "entry", "", None), ("Replace With", "entry", "", None),
                                          ("Match entire cell contents", "check", False, None)])
        if v:
            self.add_step({"op": "replace", "col": self.current, "find": v[0], "with": v[1], "whole": v[2]})

    def cmd_split(self):
        if not self._need():
            return
        modes = ["Each occurrence of the delimiter", "Left-most delimiter", "Right-most delimiter"]
        v = _ask(self, "Split Column by Delimiter", [("Delimiter", "entry", ",", None), ("Split at", "combo", modes[0], modes)])
        if v:
            self.add_step({"op": "split", "col": self.current, "delim": v[0], "mode": ["each", "left", "right"][modes.index(v[1])]})

    def cmd_group(self):
        if not self._need():
            return
        names = [n for n, _ in AGGS]
        cols = self._cols()
        v = _ask(self, "Group By", [("Group by", "combo", self.current, cols), ("New column name", "entry", "Count", None),
                                    ("Operation", "combo", names[0], names), ("Column", "combo", cols[0], cols)])
        if v:
            agg = dict(AGGS)[v[2]]
            self.add_step({"op": "group", "by": [v[0]], "aggs": [(v[1] or "Count", agg, None if agg in ("count", "distinct") else v[3])]})

    def _other_queries(self):
        from . import getdata_ui
        return [q["name"] for q in getdata_ui.queries(self.win.book) if q["name"].lower() != self.q.get("name", "").lower()]

    def cmd_merge(self):
        if not self._need():
            return
        others = self._other_queries()
        if not others:
            self.win.dialogs.message("There are no other queries in this workbook to merge with.", "Merge Queries")
            return
        kinds = ["Left Outer (all from first, matching from second)", "Inner (only matching rows)",
                 "Full Outer (all rows from both)", "Left Anti (rows only in first)"]
        v = _ask(self, "Merge", [("Merge with", "combo", others[0], others), ("Its matching column", "entry", self.current, None),
                                 ("Join Kind", "combo", kinds[0], kinds)])
        if v:
            self.add_step({"op": "merge", "query": v[0], "left": self.current, "right": v[1] or self.current,
                           "kind": ["left", "inner", "full", "anti"][kinds.index(v[2])]})

    def cmd_append(self):
        others = self._other_queries()
        if not others:
            self.win.dialogs.message("There are no other queries in this workbook to append.", "Append Queries")
            return
        v = _ask(self, "Append", [("Table to append", "combo", others[0], others)])
        if v:
            self.add_step({"op": "append", "queries": [v[0]]})


def edit(win, q, base):
    """Run the editor on q (its base rows: base). Returns (choice, q, result) or None."""
    d = QueryEditor(win, q, base)
    r = d.run()
    out = None
    if r in (Gtk.ResponseType.OK, 1):
        try:
            q2, res = d.finish()
            out = ("load" if r == Gtk.ResponseType.OK else "load-to", q2, res)
        except ValueError as e:
            win.dialogs.message(str(e), "Query Editor")
    d.destroy()
    return out


def _unused():
    return re, APP_NAME
