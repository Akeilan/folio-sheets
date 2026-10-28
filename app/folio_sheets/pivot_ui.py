"""PivotTables: Insert › PivotTable and Recommended PivotTables, the PivotTable Fields pane, the
PivotTable Analyze and Design tabs, the field drop-downs, Value Field Settings, Field Settings,
PivotTable Options, Show Details."""
import re

from gi.repository import Gdk, GLib, Gtk

from . import APP_NAME
from .core import a1
from .whatif_ui import abs_ref, parse_ref, _dialog, _row, _entry

AGGS = [("sum", "Sum"), ("count", "Count"), ("average", "Average"), ("max", "Max"), ("min", "Min"),
        ("product", "Product"), ("countNums", "Count Numbers"), ("stdDev", "StdDev"), ("stdDevp", "StdDevp"),
        ("var", "Var"), ("varp", "Varp")]
AGG_NAME = {"sum": "Sum", "count": "Count", "average": "Average", "max": "Max", "min": "Min", "product": "Product",
            "countNums": "Count", "stdDev": "StdDev", "stdDevp": "StdDevp", "var": "Var", "varp": "Varp"}
SHOW = [("normal", "No Calculation"), ("percentOfTotal", "% of Grand Total"), ("percentOfCol", "% of Column Total"),
        ("percentOfRow", "% of Row Total"), ("runTotal", "Running Total In")]
# the rest of the desktop spreadsheet's Show Values As list: listed, not available here
SHOW_MISSING = ["% Of", "% of Parent Row Total", "% of Parent Column Total", "% of Parent Total", "Difference From",
                "% Difference From", "% Running Total In", "Rank Smallest to Largest", "Rank Largest to Smallest", "Index"]
AREAS = [("pages", "Filters"), ("cols", "Columns"), ("rows", "Rows"), ("values", "Values")]
SIGMA = "Σ Values"
NOT_HERE = "%s isn't available in %s."
STYLE_FAMILIES = [("Light", 28), ("Medium", 28), ("Dark", 28)]


def current(win):
    """The PivotTable at the active cell (None: not in one)."""
    if win.book is None:
        return None
    g = win.grid
    r, c = g.sel.active
    return win.book.pivot_at(g.sheet, r, c)


def sheet_index(book, name):
    for k in range(book.sheet_count):
        if book.sheet_info(k)["name"] == name:
            return k
    return None


def sheet_ref(name):
    return name if re.match(r"^[A-Za-z_][A-Za-z0-9_.]*$", name) else "'%s'" % name.replace("'", "''")


def source_text(book, p):
    if p.get("source_table"):
        return p["source_table"]
    return "%s!%s" % (sheet_ref(p["source_sheet"]), abs_ref(p["source"]))


def parse_source(win, text):
    """'Sheet1!$A$1:$D$9', 'A1:D9' (this sheet) or a table's name -> (sheet, rect) or an error text."""
    b = win.book
    text = (text or "").strip().lstrip("=")
    if not text:
        return "Reference isn't valid."
    for s in range(b.sheet_count):
        for t in b.tables(s):
            if t["name"].lower() == text.lower():
                return s, tuple(t["ref"])
    s = win.grid.sheet
    m = re.match(r"^(?:'((?:[^']|'')+)'|([^!]+))!(.+)$", text)
    if m:
        name = (m.group(1) or "").replace("''", "'") or m.group(2)
        s = sheet_index(b, name)
        if s is None:
            return "Reference isn't valid."
        text = m.group(3)
    rg = parse_ref(text)
    if rg is None:
        return "Reference isn't valid."
    return s, rg


def is_numeric(win, p, f):
    """Does a source column hold numbers (so a checked field goes to Values)?"""
    b = win.book
    s = sheet_index(b, p["source_sheet"])
    if s is None:
        return False
    r1, c1, r2, _ = p["source"]
    seen = 0
    for r in range(r1 + 1, min(r2, r1 + 400) + 1):
        v = b.value(s, r, c1 + f)
        if v is None or v == "":
            continue
        if isinstance(v, bool) or not isinstance(v, (int, float)):
            return False
        seen += 1
    return seen > 0


def unique_name(p, base):
    taken = {v["name"].lower() for v in p["values"]} | {f["name"].lower() for f in p["fields"]}
    if base.lower() not in taken:
        return base
    k = 2
    while ("%s%d" % (base, k)).lower() in taken:
        k += 1
    return "%s%d" % (base, k)


def value_entry(win, p, f, agg=None):
    agg = agg or ("sum" if is_numeric(win, p, f) else "count")
    return {"field": f, "agg": agg, "show": "normal", "numfmt": None,
            "name": unique_name(p, "%s of %s" % (AGG_NAME[agg], p["fields"][f]["name"]))}


def apply(win, i, d, label="PivotTable"):
    """Change a PivotTable: True when it went through."""
    try:
        with win.book.group(label):
            win.book.set_pivot(i, d, label)
            autofit(win, i)
    except ValueError as e:
        win.dialogs.message(str(e), APP_NAME)
        return False
    after(win)
    return True


def autofit(win, i):
    """Column widths fit the PivotTable after each update (the desktop spreadsheet's default)."""
    p = win.book.pivot(i)
    o, t = p["out"], p["table"]
    if not o or not t or not p["values"] and not p["rows"]:
        return
    win.book.autofit_cols(p["sheet"], t[1], t[3], (t[0], t[2]))


def after(win):
    win.grid.refresh()
    win.update_title()
    sync(win)


# ---------------------------------------------------------------- the model, edited
def layout_of(p):
    """The four areas as the field list shows them: lists of field indexes, -2 for Σ Values."""
    rows, cols = list(p["rows"]), list(p["cols"])
    if len(p["values"]) > 1:
        (rows if p["values_in_rows"] else cols).append(-2)
    return {"pages": [f for f, _ in p["pages"]], "cols": cols, "rows": rows, "values": list(range(len(p["values"])))}


def with_layout(p, lay):
    """A change dict from the areas (Σ Values goes where it is; values are indexes into p's values)."""
    pages_sel = dict(p["pages"])
    d = {"rows": [f for f in lay["rows"] if f >= 0], "cols": [f for f in lay["cols"] if f >= 0],
         "pages": [(f, pages_sel.get(f)) for f in lay["pages"] if f >= 0],
         "values": [p["values"][k] if isinstance(k, int) else k for k in lay["values"]]}
    if -2 in lay["rows"]:
        d["values_in_rows"] = True
    elif -2 in lay["cols"]:
        d["values_in_rows"] = False
    return d


def place(p, lay, f, area, pos=None):
    """Put field f in an area (it leaves the others but Values); returns the new layout."""
    lay = {k: list(v) for k, v in lay.items()}
    if area == "values":
        return lay
    for k in ("pages", "cols", "rows"):
        if f in lay[k]:
            lay[k].remove(f)
    lst = lay[area]
    if f == -2 and area == "pages":
        return lay
    if pos is None or pos > len(lst):
        # Σ Values stays last
        pos = len(lst) - (1 if -2 in lst and f != -2 else 0)
    lst.insert(max(0, pos), f)
    return lay


# ---------------------------------------------------------------- Create PivotTable
class CreateDialog:
    def __init__(self, win, title="Create PivotTable"):
        self.win = win
        self.d, grid = _dialog(win, title)
        b = win.book
        g = win.grid
        r, c = g.sel.active
        r1, c1, r2, c2 = g.sel.ranges[-1]
        t = b.table_at(g.sheet, r, c)
        if t is not None:
            src = b.tables(g.sheet)[t]["name"]
        else:
            if (r1, c1) == (r2, c2):
                r1, c1, r2, c2 = b.current_region(g.sheet, r, c)
            src = "%s!%s" % (sheet_ref(b.sheet_info(g.sheet)["name"]), abs_ref((min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2))))
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Choose the data that you want to analyze</b>")
        grid.attach(lab, 0, 0, 2, 1)
        self.r_range = Gtk.RadioButton.new_with_label(None, "Select a table or range")
        grid.attach(self.r_range, 0, 1, 2, 1)
        self.source = _entry("pivot-source", src)
        _row(grid, 2, "    Table/Range:", self.source)
        self.r_ext = Gtk.RadioButton.new_with_label_from_widget(self.r_range, "Use an external data source")
        self.r_ext.set_sensitive(False)
        self.r_ext.set_tooltip_text(NOT_HERE % ("External data sources for PivotTables", APP_NAME))
        grid.attach(self.r_ext, 0, 3, 2, 1)
        self.r_model = Gtk.RadioButton.new_with_label_from_widget(self.r_range, "Use this workbook's Data Model")
        from . import model_ui
        self.r_model.set_sensitive(bool(model_ui.load(win)["tables"]))
        grid.attach(self.r_model, 0, 4, 2, 1)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Choose where you want the PivotTable report to be placed</b>")
        lab.set_margin_top(8)
        grid.attach(lab, 0, 5, 2, 1)
        self.r_new = Gtk.RadioButton.new_with_label(None, "New Worksheet")
        self.r_here = Gtk.RadioButton.new_with_label_from_widget(self.r_new, "Existing Worksheet")
        grid.attach(self.r_new, 0, 6, 2, 1)
        grid.attach(self.r_here, 0, 7, 2, 1)
        self.location = _entry("pivot-location", "")
        self.location.connect("changed", lambda *_: self.r_here.set_active(True) if self.location.get_text() else None)
        _row(grid, 8, "    Location:", self.location)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Choose whether you want to analyze multiple tables</b>")
        lab.set_margin_top(8)
        grid.attach(lab, 0, 9, 2, 1)
        self.model = Gtk.CheckButton(label="Add this data to the Data Model")
        self.model.set_tooltip_text("Analyze this table with the tables it is related to (Data > Relationships).")
        grid.attach(self.model, 0, 10, 2, 1)
        self.d.show_all()
        self.index = None

    def apply(self):
        w = self.win
        b = w.book
        from . import model_ui
        use_model = self.r_model.get_active() or self.model.get_active()
        fact = None
        if use_model:
            tabs = model_ui.all_tables(w)
            text = self.source.get_text().strip()
            fact = text if text in tabs else (model_ui.load(w).get("fact") if self.r_model.get_active() else None)
            if fact is None:
                t = next((n for n in tabs if text.lower() == n.lower()), None)
                fact = t
            if fact is None:
                return "Only tables can be added to the Data Model. Format the data as a table first."
        src = parse_source(w, self.source.get_text()) if not use_model else None
        if not use_model and isinstance(src, str):
            return "The reference isn't valid."
        if use_model:
            src = None
        ss, rg = src if src else (None, None)
        if self.r_new.get_active():
            dest = None
        else:
            text = self.location.get_text().strip()
            loc = parse_source(w, text) if text else "Destination reference is not valid."
            if isinstance(loc, str):
                return "Destination reference is not valid."
            dest = (loc[0], (loc[1][0], loc[1][1]))
        with b.group("PivotTable"):
            if use_model:
                ss, rg = model_ui.build(w, fact)
            if dest is None:
                # a new sheet in front of the active one, the table at A3
                s = w.grid.sheet
                ns = b.insert_sheet(s, None)
                if ss >= ns:
                    ss += 1
                dest = (ns, (2, 0))
            try:
                self.index = b.create_pivot(ss, rg, dest[0], dest[1])
            except ValueError as e:
                self.error = str(e)
                raise
        w.tabs.rebuild()
        w.grid.set_sheet(dest[0])
        w.sheet_changed()
        w.grid.select_cell(dest[1][0], dest[1][1])
        after(w)
        show_pane(w, True)
        return None


def _run(win, dlg, title):
    while True:
        if dlg.d.run() != Gtk.ResponseType.OK:
            break
        try:
            err = dlg.apply()
        except ValueError as e:
            err = str(e)
        if err is None:
            dlg.d.destroy()
            return True
        win.dialogs.message(err, title)
    dlg.d.destroy()
    return False


def create(win, arg=None):
    if arg in ("external", "model"):
        win.dialogs.info(NOT_HERE % ("PivotTables from %s" % ("external data sources" if arg == "external" else "the Data Model"),
                                     APP_NAME) + " Use a table or range in the workbook.", "PivotTable")
        return
    g = win.grid
    r, c = g.sel.active
    if win.book.pivot_at(g.sheet, r, c) is not None and arg != "force":
        pass
    _run(win, CreateDialog(win), "PivotTable")


# ---------------------------------------------------------------- Recommended PivotTables
def recommendations(win, p):
    """A few layouts for a fresh pivot: a text field down the rows with each number field summed."""
    nf = len(p["fields"])
    nums = [f for f in range(nf) if is_numeric(win, p, f)]
    texts = [f for f in range(nf) if f not in nums]
    out = []
    for t in texts[:4]:
        for n in nums[:3]:
            out.append(("Sum of %s by %s" % (p["fields"][n]["name"], p["fields"][t]["name"]), [t], [], [(n, "sum")]))
        if not nums:
            out.append(("Count of %s by %s" % (p["fields"][t]["name"], p["fields"][t]["name"]), [t], [], [(t, "count")]))
    if len(texts) >= 2 and nums:
        out.append(("Sum of %s by %s and %s" % (p["fields"][nums[0]]["name"], p["fields"][texts[0]]["name"], p["fields"][texts[1]]["name"]),
                    [texts[0]], [texts[1]], [(nums[0], "sum")]))
    if texts and len(nums) >= 2:
        out.append(("Sum of %s and Sum of %s by %s" % (p["fields"][nums[0]]["name"], p["fields"][nums[1]]["name"], p["fields"][texts[0]]["name"]),
                    [texts[0]], [], [(nums[0], "sum"), (nums[1], "sum")]))
    return out[:10]


class RecommendedDialog:
    """Recommended PivotTables: pick a layout; it goes on a new sheet."""

    def __init__(self, win):
        self.win = win
        b = win.book
        g = win.grid
        self.d = Gtk.Dialog(title="Recommended PivotTables", transient_for=win, modal=True)
        self.d.add_button("Blank PivotTable", 1)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        self.d.set_default_response(Gtk.ResponseType.OK)
        r, c = g.sel.active
        r1, c1, r2, c2 = g.sel.ranges[-1]
        t = b.table_at(g.sheet, r, c)
        if t is not None:
            rg = tuple(b.tables(g.sheet)[t]["ref"])
        elif (r1, c1) == (r2, c2):
            rg = tuple(b.current_region(g.sheet, r, c))
        else:
            rg = (min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2))
        self.src = (g.sheet, rg)
        heads = [b.value(g.sheet, rg[0], cc) for cc in range(rg[1], rg[3] + 1)]
        self.ok = rg[2] > rg[0] and all(h not in (None, "") for h in heads)
        p = {"fields": [{"name": str(h)} for h in heads], "source_sheet": b.sheet_info(g.sheet)["name"], "source": rg, "values": []}
        self.items = recommendations(win, p) if self.ok else []
        box = Gtk.Box(spacing=10)
        box.set_border_width(10)
        self.list = Gtk.ListBox()
        self.list.set_size_request(320, 360)
        for title, *_ in self.items:
            row = Gtk.ListBoxRow()
            lab = Gtk.Label(label=title, xalign=0)
            lab.set_line_wrap(True)
            lab.set_margin_top(6)
            lab.set_margin_bottom(6)
            lab.set_margin_start(6)
            row.add(lab)
            self.list.add(row)
        sw = Gtk.ScrolledWindow()
        sw.add(self.list)
        box.pack_start(sw, False, False, 0)
        self.preview = Gtk.Label(xalign=0, yalign=0)
        self.preview.set_size_request(320, -1)
        box.pack_start(self.preview, True, True, 0)
        self.d.get_content_area().pack_start(box, True, True, 0)
        self.list.connect("row-selected", lambda _l, row: self._show(row))
        if self.items:
            self.list.select_row(self.list.get_row_at_index(0))
        else:
            self.preview.set_text("There's no data here to recommend PivotTables for." if not self.ok else
                                  "No recommendations for this data. Use a blank PivotTable.")
        self.d.show_all()

    def _show(self, row):
        if row is None:
            return
        title, rows, cols, vals = self.items[row.get_index()]
        self.preview.set_markup("<b>%s</b>\n\nRows: %s\nColumns: %s\nValues: %s" % (
            GLib.markup_escape_text(title), ", ".join(self._n(rows)) or "-", ", ".join(self._n(cols)) or "-",
            ", ".join("%s of %s" % (AGG_NAME[a], self._n([f])[0]) for f, a in vals)))

    def _n(self, fs):
        b = self.win.book
        s, rg = self.src
        return [str(b.value(s, rg[0], rg[1] + f)) for f in fs]

    def apply(self, k=None):
        """Make the pick (k: its index; None: the selected one; -1: a blank PivotTable)."""
        w = self.win
        b = w.book
        if not self.ok:
            return "The PivotTable field name is not valid. To create a PivotTable report, you must use data that is organized as a list with labeled columns."
        if k is None:
            row = self.list.get_selected_row()
            k = row.get_index() if row is not None else -1
        s, rg = self.src
        with b.group("Recommended PivotTables"):
            ns = b.insert_sheet(s, None)
            i = b.create_pivot(s + 1, rg, ns, (2, 0))
            if k >= 0:
                _, rows, cols, vals = self.items[k]
                p = b.pivot(i)
                vs = []
                for f, agg in vals:
                    p["values"] = vs
                    vs.append(value_entry(w, p, f, agg))
                b.set_pivot(i, {"rows": rows, "cols": cols, "values": vs}, "Recommended PivotTables")
        self.index = i
        w.tabs.rebuild()
        w.grid.set_sheet(ns)
        w.sheet_changed()
        w.grid.select_cell(2, 0)
        after(w)
        show_pane(w, True)
        return None


def recommended(win):
    d = RecommendedDialog(win)
    while True:
        resp = d.d.run()
        if resp not in (Gtk.ResponseType.OK, 1):
            break
        try:
            err = d.apply(-1 if resp == 1 else None)
        except ValueError as e:
            err = str(e)
        if err is None:
            break
        win.dialogs.message(err, "Recommended PivotTables")
    d.d.destroy()


# ---------------------------------------------------------------- the PivotTable Fields pane
TARGET = [Gtk.TargetEntry.new("folio-pivot-field", Gtk.TargetFlags.SAME_APP, 0)]


class FieldPane(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.win = win
        self.i = None
        self.p = None
        self.pending = None
        self.set_size_request(300, -1)
        self.get_style_context().add_class("folio-pane")
        self.set_name("pivot-fields")
        head = Gtk.Box(spacing=6)
        head.set_border_width(6)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<span size='large'>PivotTable Fields</span>")
        lab.get_style_context().add_class("folio-pane-title")
        head.pack_start(lab, True, True, 0)
        close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda _b: show_pane(win, False))
        head.pack_end(close, False, False, 0)
        self.pack_start(head, False, False, 0)
        self.pack_start(Gtk.Label(label="Choose fields to add to report:", xalign=0, margin_start=8), False, False, 0)
        self.search = Gtk.SearchEntry()
        self.search.set_margin_start(8)
        self.search.set_margin_end(8)
        self.search.connect("search-changed", lambda *_: self.fields.invalidate_filter())
        self.pack_start(self.search, False, False, 0)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_min_content_height(160)
        sw.set_vexpand(True)
        self.fields = Gtk.ListBox()
        self.fields.set_selection_mode(Gtk.SelectionMode.NONE)
        self.fields.set_filter_func(self._filter)
        sw.add(self.fields)
        sw.set_margin_start(8)
        sw.set_margin_end(8)
        self.pack_start(sw, True, True, 0)
        # dropping a field back on the list takes it out
        self.fields.drag_dest_set(Gtk.DestDefaults.ALL, TARGET, Gdk.DragAction.MOVE)
        self.fields.connect("drag-data-received", self._dropped_on_list)
        self.pack_start(Gtk.Label(label="Drag fields between areas below:", xalign=0, margin_start=8, margin_top=4), False, False, 0)
        areas = Gtk.Grid(column_spacing=6, row_spacing=4, column_homogeneous=True)
        areas.set_margin_start(8)
        areas.set_margin_end(8)
        self.lists = {}
        for k, (key, title) in enumerate(AREAS):
            box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
            box.pack_start(Gtk.Label(label=title, xalign=0), False, False, 0)
            lb = Gtk.ListBox()
            lb.set_name("area-" + key)
            lb.set_selection_mode(Gtk.SelectionMode.NONE)
            lb.get_style_context().add_class("folio-pivot-area")
            lb.drag_dest_set(Gtk.DestDefaults.ALL, TARGET, Gdk.DragAction.MOVE)
            lb.connect("drag-data-received", self._dropped, key)
            fr = Gtk.Frame()
            sw2 = Gtk.ScrolledWindow()
            sw2.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
            sw2.set_min_content_height(96)
            sw2.add(lb)
            fr.add(sw2)
            box.pack_start(fr, True, True, 0)
            areas.attach(box, k % 2, k // 2, 1, 1)
            self.lists[key] = lb
        self.pack_start(areas, False, False, 0)
        foot = Gtk.Box(spacing=6)
        foot.set_border_width(6)
        self.defer = Gtk.CheckButton(label="Defer Layout Update")
        self.defer.connect("toggled", lambda c: self._deferred(c.get_active()))
        self.update_btn = Gtk.Button(label="Update")
        self.update_btn.set_sensitive(False)
        self.update_btn.connect("clicked", lambda _b: self.update_now())
        foot.pack_start(self.defer, True, True, 0)
        foot.pack_end(self.update_btn, False, False, 0)
        self.pack_start(foot, False, False, 0)
        self.set_no_show_all(True)
        for ch in self.get_children():
            ch.show_all()

    # ---- state
    def _filter(self, row):
        t = self.search.get_text().strip().lower()
        return not t or t in row.field_name.lower()

    def model(self):
        """The pivot as the pane shows it (with a deferred change, the pending one)."""
        if self.pending is not None:
            p = dict(self.p)
            p.update(self.pending)
            return p
        return self.p

    def load(self, i):
        b = self.win.book
        self.i = i
        if i is None:
            self.p = None
            self.pending = None
            self._fill()
            return
        self.p = b.pivot(i)
        self._fill()

    def _fill(self):
        for lb in [self.fields] + list(self.lists.values()):
            for ch in lb.get_children():
                lb.remove(ch)
        p = self.model()
        if p is None:
            return
        lay = layout_of(p)
        used = set(lay["pages"]) | set(lay["cols"]) | set(lay["rows"]) | {v["field"] for v in p["values"]}
        for f, fd in enumerate(p["fields"]):
            row = Gtk.ListBoxRow()
            row.field_name = fd["name"]
            box = Gtk.Box(spacing=4)
            chk = Gtk.CheckButton(label=fd["name"])
            chk.set_active(f in used)
            chk.connect("toggled", lambda c, f=f: self.toggle(f, c.get_active()))
            box.pack_start(chk, True, True, 0)
            if fd["hidden"] or any(pf == f and sel is not None for pf, sel in p["pages"]):
                box.pack_end(Gtk.Image.new_from_icon_name("view-filter-symbolic" if Gtk.IconTheme.get_default().has_icon("view-filter-symbolic")
                                                           else "edit-find-symbolic", Gtk.IconSize.MENU), False, False, 0)
            row.add(box)
            self._drag_source(row, "field:%d" % f)
            self.fields.add(row)
        for key, _ in AREAS:
            for k, f in enumerate(lay[key]):
                if key == "values":
                    text = p["values"][f]["name"]
                elif f == -2:
                    text = SIGMA
                else:
                    text = p["fields"][f]["name"]
                row = Gtk.ListBoxRow()
                mb = Gtk.MenuButton()
                lab = Gtk.Label(label=text, xalign=0)
                lab.set_ellipsize(3)
                inner = Gtk.Box(spacing=4)
                inner.pack_start(lab, True, True, 0)
                inner.pack_end(Gtk.Image.new_from_icon_name("pan-down-symbolic", Gtk.IconSize.MENU), False, False, 0)
                mb.add(inner)
                mb.get_label = lab.get_text
                mb.set_relief(Gtk.ReliefStyle.NONE)
                mb.get_style_context().add_class("folio-pivot-item")
                mb.set_popup(self._item_menu(key, k))
                row.add(mb)
                self._drag_source(mb, "%s:%d" % (key, k))
                self.lists[key].add(row)
        for lb in [self.fields] + list(self.lists.values()):
            lb.show_all()

    def _drag_source(self, w, payload):
        w.drag_source_set(Gdk.ModifierType.BUTTON1_MASK, TARGET, Gdk.DragAction.MOVE)
        w.connect("drag-data-get", lambda _w, _ctx, data, _info, _t: data.set(data.get_target(), 8, payload.encode()))

    def _drop_pos(self, lb, y):
        k = 0
        for ch in lb.get_children():
            a = ch.get_allocation()
            if y > a.y + a.height / 2:
                k += 1
        return k

    def _dropped(self, lb, _ctx, _x, y, data, _info, _t, key):
        payload = data.get_data().decode()
        GLib.idle_add(lambda: (self.drop(payload, key, self._drop_pos(lb, y)), False)[1])

    def _dropped_on_list(self, _lb, _ctx, _x, _y, data, _info, _t):
        payload = data.get_data().decode()
        what, _, k = payload.partition(":")
        if what in ("pages", "cols", "rows", "values"):
            GLib.idle_add(lambda: (self.remove(what, int(k)), False)[1])

    # ---- edits
    def change(self, d, label="PivotTable"):
        if self.i is None:
            return False
        if self.defer.get_active():
            base = dict(self.pending or {})
            base.update(d)
            self.pending = base
            self.update_btn.set_sensitive(True)
            self._fill()
            return True
        ok = apply(self.win, self.i, d, label)
        if not ok:
            self.load(self.i)
        return ok

    def _deferred(self, on):
        if not on and self.pending is not None:
            self.update_now()
        self.update_btn.set_sensitive(on and self.pending is not None)

    def update_now(self):
        d, self.pending = self.pending, None
        self.update_btn.set_sensitive(False)
        if d:
            apply(self.win, self.i, d, "PivotTable")
        self.load(self.i)

    def toggle(self, f, on):
        """A field's box ticked: text to Rows, numbers to Values; cleared: out of every area."""
        p = self.model()
        lay = layout_of(p)
        if on:
            if is_numeric(self.win, p, f):
                return self.change({"values": p["values"] + [value_entry(self.win, p, f)]})
            return self.change(with_layout(p, place(p, lay, f, "rows")))
        for k in ("pages", "cols", "rows"):
            if f in lay[k]:
                lay[k].remove(f)
        d = with_layout(p, lay)
        d["values"] = [v for v in p["values"] if v["field"] != f]
        if len(d["values"]) < 2:
            d["rows"] = [x for x in d["rows"] if x >= 0]
        return self.change(d)

    def add(self, area, f, pos=None):
        """A field dropped in an area."""
        p = self.model()
        if area == "values":
            vals = list(p["values"])
            v = value_entry(self.win, p, f)
            vals.insert(len(vals) if pos is None else pos, v)
            return self.change({"values": vals})
        return self.change(with_layout(p, place(p, layout_of(p), f, area, pos)))

    def remove(self, area, k):
        p = self.model()
        lay = layout_of(p)
        if area == "values":
            vals = [v for j, v in enumerate(p["values"]) if j != k]
            return self.change({"values": vals})
        f = lay[area][k]
        if f == -2:
            # Σ Values out: the data fields go
            return self.change({"values": []})
        lay[area].pop(k)
        return self.change(with_layout(p, lay))

    def move(self, area, k, to_area, pos=None):
        """An area's item moved (within its area, or to another)."""
        p = self.model()
        lay = layout_of(p)
        if area == "values":
            if to_area == "values":
                vals = list(p["values"])
                v = vals.pop(k)
                vals.insert(len(vals) if pos is None else min(pos, len(vals)), v)
                return self.change({"values": vals})
            f = p["values"][k]["field"]
            vals = [v for j, v in enumerate(p["values"]) if j != k]
            q = dict(p)
            q["values"] = vals
            d = with_layout(q, place(q, layout_of(q), f, to_area, pos))
            d["values"] = vals
            return self.change(d)
        f = lay[area][k]
        if to_area == "values":
            if f == -2:
                return False
            lay[area].pop(k)
            d = with_layout(p, lay)
            d["values"] = p["values"] + [value_entry(self.win, p, f)]
            return self.change(d)
        if f == -2 and to_area == "pages":
            self.win.dialogs.message("The field you are moving cannot be placed in that PivotTable area.", APP_NAME)
            return False
        if to_area == area and pos is not None and pos > k:
            pos -= 1
        return self.change(with_layout(p, place(p, lay, f, to_area, pos)))

    def drop(self, payload, area, pos):
        what, _, k = payload.partition(":")
        k = int(k)
        if what == "field":
            return self.add(area, k, pos)
        return self.move(what, k, area, pos)

    def _item_menu(self, area, k):
        p = self.model()
        lay = layout_of(p)
        n = len(lay[area])
        m = Gtk.Menu()

        def item(label, fn, ok=True):
            mi = Gtk.MenuItem(label=label)
            mi.set_sensitive(ok)
            mi.connect("activate", lambda _m: GLib.idle_add(lambda: (fn(), False)[1]))
            m.append(mi)
        item("Move Up", lambda: self.move(area, k, area, k - 1), k > 0)
        item("Move Down", lambda: self.move(area, k, area, k + 2), k < n - 1)
        item("Move to Beginning", lambda: self.move(area, k, area, 0), k > 0)
        item("Move to End", lambda: self.move(area, k, area, n), k < n - 1)
        m.append(Gtk.SeparatorMenuItem())
        sigma = area != "values" and lay[area][k] == -2
        for key, title in (("pages", "Move to Report Filter"), ("rows", "Move to Row Labels"), ("cols", "Move to Column Labels"),
                           ("values", "Move to Values")):
            item(title, lambda key=key: self.move(area, k, key), key != area and not (sigma and key in ("pages", "values")))
        m.append(Gtk.SeparatorMenuItem())
        item("Remove Field", lambda: self.remove(area, k))
        m.append(Gtk.SeparatorMenuItem())
        if area == "values":
            item("Value Field Settings...", lambda: value_settings(self.win, self.i, k))
        else:
            item("Field Settings...", lambda: field_settings(self.win, self.i, lay[area][k]), not sigma)
        m.show_all()
        return m


def make_pane(win):
    return FieldPane(win)


def show_pane(win, show=None):
    pane = getattr(win, "pivot_pane", None)
    if pane is None:
        return
    i = current(win)
    if show is None:
        show = not pane.get_visible()
    win.pivot_pane_wanted = show
    pane.set_visible(show and i is not None)
    if pane.get_visible():
        pane.load(i)
    win.ribbon.update("pivot-field-list", "true" if show else "false")


def sync(win):
    """The pivot tabs and pane follow the active cell."""
    if win.book is None or not hasattr(win, "pivot_name"):
        return
    i = current(win)
    on = i is not None
    win.ribbon.show_contextual("PivotTable Analyze", on)
    win.ribbon.show_contextual("Design", on)
    pane = getattr(win, "pivot_pane", None)
    if pane is not None:
        want = getattr(win, "pivot_pane_wanted", True)
        if on and want:
            if not pane.get_visible():
                pane.show()
            if pane.i != i or pane.pending is None:
                pane.load(i)
        elif pane.get_visible():
            pane.hide()
    if not on:
        return
    p = win.book.pivot(i)
    win.syncing_checks = True
    try:
        if not win.pivot_name.has_focus():
            win.pivot_name.set_text(p["name"])
        af = active_field(win)
        if not win.pivot_field.has_focus():
            win.pivot_field.set_text(af[1] if af else "")
        for key, chk in win.pivot_checks.items():
            chk.set_active(bool(p[key]))
        win.ribbon.update("pivot-field-headers", "true" if p["field_headers"] else "false")
    finally:
        win.syncing_checks = False


def active_field(win):
    """The field the active cell is about: ("row"/"col"/"page", name, index) or ("value", name, k)."""
    i = current(win)
    if i is None:
        return None
    b = win.book
    g = win.grid
    r, c = g.sel.active
    p = b.pivot(i)
    text = b.value(g.sheet, r, c)
    text = "" if text is None else str(text)
    for k, v in enumerate(p["values"]):
        if v["name"] == text:
            return ("value", v["name"], k)
    path = b.pivot_path(g.sheet, r, c)
    if path is not None and p["values"]:
        if len(p["values"]) == 1:
            return ("value", p["values"][0]["name"], 0)
        # which data field the column (or row) holds
        for k, v in enumerate(p["values"]):
            hr = _header_text(b, g.sheet, p, r, c)
            if hr and v["name"] in hr:
                return ("value", v["name"], k)
        return ("value", p["values"][0]["name"], 0)
    role = b.pivot_role(g.sheet, r, c)
    if role and role[0] == "page_label":
        for f, _ in p["pages"]:
            if p["fields"][f]["name"] == text:
                return ("page", text, f)
    if role and role[0] in ("row_item", "subtotal"):
        lvl = role[1]
        if lvl < len(p["rows"]):
            f = p["rows"][lvl]
            return ("row", p["fields"][f]["name"], f)
    if role and role[0] == "col_item" and p["cols"]:
        lvl = min(max(0, r - p["table"][0] - 1), len(p["cols"]) - 1)
        f = p["cols"][lvl]
        return ("col", p["fields"][f]["name"], f)
    for f, fd in enumerate(p["fields"]):
        if fd["name"] == text and (f in p["rows"] or f in p["cols"]):
            return ("row" if f in p["rows"] else "col", text, f)
    if p["rows"] and text in ("Row Labels",):
        f = p["rows"][0]
        return ("row", p["fields"][f]["name"], f)
    if p["cols"] and text == "Column Labels":
        f = p["cols"][0]
        return ("col", p["fields"][f]["name"], f)
    return None


def _header_text(b, s, p, r, c):
    """The data-field caption over a value cell (several values across the columns)."""
    t = p["table"]
    for rr in range(t[0], r):
        v = b.value(s, rr, c)
        if isinstance(v, str) and v:
            last = v
    return locals().get("last")


# ---------------------------------------------------------------- Value Field Settings
class ValueSettings:
    def __init__(self, win, i, k):
        self.win = win
        self.i = i
        self.k = k
        b = win.book
        p = b.pivot(i)
        self.p = p
        v = p["values"][k]
        self.numfmt = v["numfmt"]
        self.d = Gtk.Dialog(title="Value Field Settings", transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        self.d.set_default_response(Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(10)
        box.pack_start(Gtk.Label(label="Source Name: %s" % p["fields"][v["field"]]["name"], xalign=0), False, False, 0)
        self.name = _entry("custom-name", v["name"])
        row = Gtk.Box(spacing=6)
        row.pack_start(Gtk.Label(label="Custom Name:"), False, False, 0)
        row.pack_start(self.name, True, True, 0)
        box.pack_start(row, False, False, 0)
        nb = Gtk.Notebook()
        # Summarize Values By
        pg = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        pg.set_border_width(8)
        pg.pack_start(Gtk.Label(label="Summarize value field by\n\nChoose the type of calculation that you want to use to "
                                      "summarize data from selected field", xalign=0), False, False, 0)
        self.aggs = Gtk.ListBox()
        for code, label in AGGS:
            r = Gtk.ListBoxRow()
            r.code = code
            r.add(Gtk.Label(label=label, xalign=0, margin_start=4))
            self.aggs.add(r)
            if code == v["agg"]:
                self.aggs.select_row(r)
        self.aggs.connect("row-selected", self._agg_changed)
        sw = Gtk.ScrolledWindow()
        sw.set_min_content_height(150)
        sw.add(self.aggs)
        pg.pack_start(sw, True, True, 0)
        nb.append_page(pg, Gtk.Label(label="Summarize Values By"))
        # Show Values As
        pg = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        pg.set_border_width(8)
        pg.pack_start(Gtk.Label(label="Show values as", xalign=0), False, False, 0)
        self.show = Gtk.ComboBoxText()
        for code, label in SHOW:
            self.show.append(code, label)
        for label in SHOW_MISSING:
            self.show.append("x-" + label, label + " (not available)")
        self.show.set_active_id(v["show"])
        self.show.connect("changed", self._show_changed)
        pg.pack_start(self.show, False, False, 0)
        self.base_field = Gtk.ComboBoxText()
        for f in p["rows"] + p["cols"]:
            self.base_field.append(str(f), p["fields"][f]["name"])
        if p["rows"] or p["cols"]:
            self.base_field.set_active(0)
        g2 = Gtk.Grid(column_spacing=6, row_spacing=4)
        g2.attach(Gtk.Label(label="Base field:", xalign=0), 0, 0, 1, 1)
        g2.attach(self.base_field, 1, 0, 1, 1)
        pg.pack_start(g2, False, False, 0)
        self.base_field.set_sensitive(v["show"] == "runTotal")
        nb.append_page(pg, Gtk.Label(label="Show Values As"))
        box.pack_start(nb, True, True, 0)
        nf = Gtk.Button(label="Number Format")
        nf.connect("clicked", lambda _b: self.number_format())
        bb = Gtk.Box()
        bb.pack_start(nf, False, False, 0)
        box.pack_start(bb, False, False, 0)
        self.d.get_content_area().pack_start(box, True, True, 0)
        self.d.show_all()

    def _agg_changed(self, _lb, row):
        if row is None:
            return
        v = self.p["values"][self.k]
        fname = self.p["fields"][v["field"]]["name"]
        cur = self.name.get_text()
        # the default caption follows the calculation
        if re.match(r"^(%s) of %s\d*$" % ("|".join(AGG_NAME.values()), re.escape(fname)), cur):
            q = dict(self.p)
            q["values"] = [x for j, x in enumerate(self.p["values"]) if j != self.k]
            self.name.set_text(unique_name(q, "%s of %s" % (AGG_NAME[row.code], fname)))

    def _show_changed(self, cb):
        code = cb.get_active_id() or "normal"
        if code.startswith("x-"):
            self.win.dialogs.info(NOT_HERE % ("Show Values As “%s”" % code[2:], APP_NAME), "Value Field Settings")
            cb.set_active_id(self.p["values"][self.k]["show"])
            return
        self.base_field.set_sensitive(code == "runTotal")

    def number_format(self):
        from .format_cells import FormatCells
        w = self.win
        g = w.grid
        r, c = g.sel.active
        spec = dict(w.book.style_spec(g.sheet, r, c))
        spec["numfmt"] = self.numfmt or "General"
        dlg = FormatCells(w, spec, "number")
        for k in range(1, dlg.nb.get_n_pages()):
            dlg.nb.get_nth_page(k).hide()
        if dlg.run() == Gtk.ResponseType.OK:
            self.numfmt = dlg.current_code()
        dlg.destroy()

    def apply(self):
        p = self.win.book.pivot(self.i)
        vals = p["values"]
        name = self.name.get_text().strip()
        if not name:
            return "The PivotTable field name is not valid."
        if any(j != self.k and v["name"].lower() == name.lower() for j, v in enumerate(vals)) or \
                any(f["name"].lower() == name.lower() for f in p["fields"]):
            return "PivotTable field name already exists."
        row = self.aggs.get_selected_row()
        v = dict(vals[self.k])
        v["name"] = name
        v["agg"] = row.code if row is not None else v["agg"]
        v["show"] = self.show.get_active_id() or "normal"
        v["numfmt"] = self.numfmt
        vals[self.k] = v
        return None if apply(self.win, self.i, {"values": vals}, "Value Field Settings") else ""


def value_settings(win, i=None, k=None):
    i = current(win) if i is None else i
    if i is None:
        return
    if k is None:
        af = active_field(win)
        if af is None or af[0] != "value":
            return field_settings(win, i)
        k = af[2]
    _run(win, ValueSettings(win, i, k), "Value Field Settings")


# ---------------------------------------------------------------- Field Settings
class FieldSettings:
    def __init__(self, win, i, f):
        self.win = win
        self.i = i
        self.f = f
        p = win.book.pivot(i)
        fd = p["fields"][f]
        self.d, grid = _dialog(win, "Field Settings")
        grid.attach(Gtk.Label(label="Source Name: %s" % fd["name"], xalign=0), 0, 0, 2, 1)
        self.name = _entry("field-name", fd["name"])
        self.name.set_sensitive(False)
        self.name.set_tooltip_text(NOT_HERE % ("Renaming a field", APP_NAME))
        _row(grid, 1, "Custom Name:", self.name)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Subtotals</b>")
        grid.attach(lab, 0, 2, 2, 1)
        self.auto = Gtk.RadioButton.new_with_label(None, "Automatic")
        self.none = Gtk.RadioButton.new_with_label_from_widget(self.auto, "None")
        self.custom = Gtk.RadioButton.new_with_label_from_widget(self.auto, "Custom")
        self.custom.set_sensitive(False)
        self.custom.set_tooltip_text(NOT_HERE % ("Custom subtotals", APP_NAME))
        (self.auto if fd["subtotal"] else self.none).set_active(True)
        for k, w in enumerate((self.auto, self.none, self.custom)):
            grid.attach(w, 0, 3 + k, 2, 1)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Layout</b>")
        grid.attach(lab, 0, 6, 2, 1)
        self.sort = Gtk.ComboBoxText()
        self.sort.append("1", "Ascending (A to Z)")
        self.sort.append("-1", "Descending (Z to A)")
        self.sort.set_active_id("-1" if fd["sort"] < 0 else "1")
        _row(grid, 7, "Sort:", self.sort)
        self.d.show_all()

    def apply(self):
        p = self.win.book.pivot(self.i)
        fields = [dict(x) for x in p["fields"]]
        fields[self.f]["subtotal"] = self.auto.get_active()
        fields[self.f]["sort"] = int(self.sort.get_active_id() or "1")
        return None if apply(self.win, self.i, {"fields": fields}, "Field Settings") else ""


def field_settings(win, i=None, f=None):
    i = current(win) if i is None else i
    if i is None:
        return
    if f is None:
        af = active_field(win)
        if af is None:
            return
        if af[0] == "value":
            return value_settings(win, i, af[2])
        f = af[2]
    _run(win, FieldSettings(win, i, f), "Field Settings")


# ---------------------------------------------------------------- PivotTable Options
class OptionsDialog:
    def __init__(self, win, i):
        self.win = win
        self.i = i
        p = win.book.pivot(i)
        self.d = Gtk.Dialog(title="PivotTable Options", transient_for=win, modal=True)
        self.d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.d.add_button("OK", Gtk.ResponseType.OK)
        self.d.set_default_response(Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(10)
        self.name = _entry("pivot-name", p["name"])
        row = Gtk.Box(spacing=6)
        row.pack_start(Gtk.Label(label="PivotTable Name:"), False, False, 0)
        row.pack_start(self.name, True, True, 0)
        box.pack_start(row, False, False, 0)
        nb = Gtk.Notebook()
        self.checks = {}

        def page(title, items):
            pg = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
            pg.set_border_width(8)
            for key, label in items:
                if key.startswith("#"):
                    lab = Gtk.Label(xalign=0)
                    lab.set_markup("<b>%s</b>" % label)
                    pg.pack_start(lab, False, False, 0)
                    continue
                chk = Gtk.CheckButton(label=label)
                if key.startswith("x-"):
                    chk.set_sensitive(False)
                    chk.set_active(key == "x-autofit")
                    chk.set_tooltip_text(NOT_HERE % (label, APP_NAME))
                else:
                    chk.set_active(bool(p[key]))
                    self.checks[key] = chk
                pg.pack_start(chk, False, False, 0)
            nb.append_page(pg, Gtk.Label(label=title))
        page("Layout & Format", [("#l", "Layout"), ("x-merge", "Merge and center cells with labels"), ("#f", "Format"),
                                 ("x-autofit", "Autofit column widths on update"),
                                 ("x-preserve", "Preserve cell formatting on update")])
        page("Totals & Filters", [("#g", "Grand Totals"), ("grand_rows", "Show grand totals for rows"),
                                  ("grand_cols", "Show grand totals for columns"), ("#f", "Filters"),
                                  ("x-multi", "Allow multiple filters per field")])
        page("Display", [("#d", "Display"), ("field_headers", "Display field captions and filter drop downs"),
                         ("x-expand", "Show expand/collapse buttons"), ("x-tips", "Show contextual tooltips")])
        page("Printing", [("#p", "Print"), ("x-print-buttons", "Print expand/collapse buttons when displayed on PivotTable"),
                          ("x-titles", "Set print titles")])
        page("Data", [("#d", "PivotTable Data"), ("x-save", "Save source data with file"),
                      ("x-refresh", "Refresh data when opening the file")])
        box.pack_start(nb, True, True, 0)
        self.d.get_content_area().pack_start(box, True, True, 0)
        self.d.show_all()

    def apply(self):
        d = {k: c.get_active() for k, c in self.checks.items()}
        name = self.name.get_text().strip()
        if not name:
            return "The PivotTable name is not valid."
        d["name"] = name
        return None if apply(self.win, self.i, d, "PivotTable Options") else ""


def options(win):
    i = current(win)
    if i is not None:
        _run(win, OptionsDialog(win, i), "PivotTable Options")


# ---------------------------------------------------------------- Change Data Source / Move
class SourceDialog:
    def __init__(self, win, i):
        self.win = win
        self.i = i
        p = win.book.pivot(i)
        self.d, grid = _dialog(win, "Change PivotTable Data Source")
        grid.attach(Gtk.Label(label="Choose the data that you want to analyze", xalign=0), 0, 0, 2, 1)
        self.source = _entry("pivot-source", source_text(win.book, p))
        _row(grid, 1, "Table/Range:", self.source)
        self.d.show_all()

    def apply(self):
        w = self.win
        src = parse_source(w, self.source.get_text())
        if isinstance(src, str):
            return "The reference isn't valid."
        s, rg = src
        b = w.book
        heads = [b.value(s, rg[0], c) for c in range(rg[1], rg[3] + 1)]
        if rg[2] <= rg[0] or any(h in (None, "") for h in heads):
            return ("The PivotTable field name is not valid. To create a PivotTable report, you must use data that is "
                    "organized as a list with labeled columns.")
        return None if apply(w, self.i, {"source": rg, "source_sheet": b.sheet_info(s)["name"]}, "Change Data Source") else ""


class MoveDialog:
    def __init__(self, win, i):
        self.win = win
        self.i = i
        self.d, grid = _dialog(win, "Move PivotTable")
        grid.attach(Gtk.Label(label="Choose where you want the PivotTable to be placed", xalign=0), 0, 0, 2, 1)
        self.r_new = Gtk.RadioButton.new_with_label(None, "New Worksheet")
        self.r_here = Gtk.RadioButton.new_with_label_from_widget(self.r_new, "Existing Worksheet")
        self.r_here.set_active(True)
        grid.attach(self.r_new, 0, 1, 2, 1)
        grid.attach(self.r_here, 0, 2, 2, 1)
        p = win.book.pivot(i)
        o = p["out"]
        self.location = _entry("pivot-location", "%s!%s" % (sheet_ref(win.book.sheet_info(p["sheet"])["name"]), abs_ref((o[0], o[1], o[0], o[1]))))
        _row(grid, 3, "Location:", self.location)
        self.d.show_all()

    def apply(self):
        w = self.win
        b = w.book
        with b.group("Move PivotTable"):
            if self.r_new.get_active():
                s = w.grid.sheet
                ns = b.insert_sheet(s, None)
                dest = (ns, (2, 0))
            else:
                loc = parse_source(w, self.location.get_text())
                if isinstance(loc, str):
                    return "Destination reference is not valid."
                dest = (loc[0], (loc[1][0], loc[1][1]))
            try:
                b.move_pivot(self.i, dest[0], dest[1])
            except ValueError as e:
                return str(e)
        w.tabs.rebuild()
        w.grid.set_sheet(dest[0])
        w.sheet_changed()
        p = b.pivot(self.i)
        w.grid.select_cell(p["out"][0], p["out"][1])
        after(w)
        return None


# ---------------------------------------------------------------- the field drop-downs
class FilterPopover:
    """A pivot drop-down: for a row or column field, sort and the items to show; for a filter,
    the item (or items) to show."""

    def __init__(self, win, i, what, idx, rect=None):
        self.win = win
        self.i = i
        self.what = what
        b = win.book
        p = b.pivot(i)
        self.p = p
        self.pop = Gtk.Popover()
        self.pop.set_relative_to(win.grid.area)
        if rect is not None:
            r = Gdk.Rectangle()
            r.x, r.y, r.width, r.height = [int(v) for v in rect]
            self.pop.set_pointing_to(r)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.set_border_width(8)
        if what == "page":
            self.f = p["pages"][idx][0]
            self.sel = p["pages"][idx][1]
        else:
            fs = p["rows"] if what == "rows" else p["cols"]
            self.f = idx if idx in fs else fs[0]
            if len(fs) > 1:
                row = Gtk.Box(spacing=6)
                row.pack_start(Gtk.Label(label="Select field:"), False, False, 0)
                self.fsel = Gtk.ComboBoxText()
                for f in fs:
                    self.fsel.append(str(f), p["fields"][f]["name"])
                self.fsel.set_active_id(str(self.f))
                self.fsel.connect("changed", lambda cb: self._field(int(cb.get_active_id())))
                row.pack_start(self.fsel, True, True, 0)
                box.pack_start(row, False, False, 0)
            for label, d in (("Sort A to Z", 1), ("Sort Z to A", -1)):
                bt = Gtk.ModelButton(text=label)
                bt.connect("clicked", lambda _b, d=d: self.sort(d))
                box.pack_start(bt, False, False, 0)
            box.pack_start(Gtk.Separator(), False, False, 2)
            bt = Gtk.ModelButton(text='Clear Filter From "%s"' % p["fields"][self.f]["name"])
            bt.set_sensitive(bool(p["fields"][self.f]["hidden"]))
            bt.connect("clicked", lambda _b: self.clear())
            self.clear_btn = bt
            box.pack_start(bt, False, False, 0)
            for label in ("Label Filters", "Value Filters"):
                bt = Gtk.ModelButton(text=label)
                bt.connect("clicked", lambda _b, label=label: win.dialogs.info(NOT_HERE % (label, APP_NAME) +
                                                                               " Pick the items to show below.", "PivotTable"))
                box.pack_start(bt, False, False, 0)
        self.search = Gtk.SearchEntry()
        self.search.connect("search-changed", lambda *_: self._filter())
        box.pack_start(self.search, False, False, 0)
        sw = Gtk.ScrolledWindow()
        sw.set_min_content_height(180)
        sw.set_min_content_width(220)
        self.list = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        sw.add(self.list)
        box.pack_start(sw, True, True, 0)
        if what == "page":
            self.multi = Gtk.CheckButton(label="Select Multiple Items")
            self.multi.set_active(self.sel is None and bool(p["fields"][self.f]["hidden"]))
            self.multi.connect("toggled", lambda *_: self._fill())
            box.pack_start(self.multi, False, False, 0)
        bb = Gtk.Box(spacing=6)
        ok = Gtk.Button(label="OK")
        ok.connect("clicked", lambda _b: self.ok())
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", lambda _b: self.pop.popdown())
        bb.pack_end(cancel, False, False, 0)
        bb.pack_end(ok, False, False, 0)
        box.pack_start(bb, False, False, 0)
        self.pop.add(box)
        self._fill()
        box.show_all()

    def _field(self, f):
        self.f = f
        self.clear_btn.set_label('Clear Filter From "%s"' % self.p["fields"][f]["name"])
        self.clear_btn.set_sensitive(bool(self.p["fields"][f]["hidden"]))
        self._fill()

    def _fill(self):
        for ch in self.list.get_children():
            self.list.remove(ch)
        b = self.win.book
        self.items = b.pivot_field_items(self.i, self.f)
        hidden = set(self.p["fields"][self.f]["hidden"])
        self.checks = []
        self.radios = []
        if self.what == "page" and not self.multi.get_active():
            first = Gtk.RadioButton.new_with_label(None, "(All)")
            first.item = None
            self.radios.append(first)
            self.list.pack_start(first, False, False, 0)
            for t in self.items:
                rb = Gtk.RadioButton.new_with_label_from_widget(first, t)
                rb.item = t
                if self.sel == t:
                    rb.set_active(True)
                self.radios.append(rb)
                self.list.pack_start(rb, False, False, 0)
        else:
            self.all = Gtk.CheckButton(label="(Select All)")
            self.all.set_active(not hidden)
            self.all.connect("toggled", lambda c: [x.set_active(c.get_active()) for x in self.checks if x.get_visible()])
            self.list.pack_start(self.all, False, False, 0)
            for t in self.items:
                cb = Gtk.CheckButton(label=t)
                cb.item = t
                cb.set_active(t not in hidden)
                self.checks.append(cb)
                self.list.pack_start(cb, False, False, 0)
        self.list.show_all()
        self._filter()

    def _filter(self):
        t = self.search.get_text().strip().lower()
        for w in self.checks + self.radios:
            if getattr(w, "item", None) is None:
                continue
            w.set_visible(not t or t in w.item.lower())

    def popup(self):
        self.pop.popup()

    def set_shown(self, items):
        """Tick exactly these items (tests and the search box)."""
        for cb in self.checks:
            cb.set_active(cb.item in items)

    def pick(self, item):
        for rb in self.radios:
            if rb.item == item:
                rb.set_active(True)

    def ok(self):
        p = self.win.book.pivot(self.i)
        fields = [dict(x) for x in p["fields"]]
        if self.what == "page" and not self.multi.get_active():
            sel = next((rb.item for rb in self.radios if rb.get_active()), None)
            fields[self.f]["hidden"] = []
            pages = [(f, sel if f == self.f else s) for f, s in p["pages"]]
            d = {"pages": pages, "fields": fields}
        else:
            shown = [cb.item for cb in self.checks if cb.get_active()]
            if not shown:
                self.win.dialogs.message("You must select at least one item.", APP_NAME) if self.what != "page" else None
                if self.what != "page":
                    return False
            fields[self.f]["hidden"] = [cb.item for cb in self.checks if not cb.get_active()]
            d = {"fields": fields}
            if self.what == "page":
                d["pages"] = [(f, None if f == self.f else s) for f, s in p["pages"]]
        self.pop.popdown()
        return apply(self.win, self.i, d, "Filter")

    def clear(self):
        p = self.win.book.pivot(self.i)
        fields = [dict(x) for x in p["fields"]]
        fields[self.f]["hidden"] = []
        self.pop.popdown()
        return apply(self.win, self.i, {"fields": fields}, "Clear Filter")

    def sort(self, d):
        p = self.win.book.pivot(self.i)
        fields = [dict(x) for x in p["fields"]]
        fields[self.f]["sort"] = d
        self.pop.popdown()
        return apply(self.win, self.i, {"fields": fields}, "Sort")


def dropdown(win, r, c, rect):
    g = win.grid
    hit = win.book.pivot_button_at(g.sheet, r, c)
    if hit is None:
        return None
    i, what, idx = hit
    fp = FilterPopover(win, i, what, idx, rect)
    fp.popup()
    win._pivot_popover = fp
    return fp


# ---------------------------------------------------------------- commands
def show_details(win, r=None, c=None):
    g = win.grid
    i = current(win)
    if i is None:
        return False
    r, c = g.sel.active if r is None else (r, c)
    try:
        ns = win.book.pivot_details(i, r, c)
    except ValueError as e:
        win.dialogs.message(str(e), APP_NAME)
        return False
    win.tabs.rebuild()
    g.set_sheet(ns)
    win.sheet_changed()
    g.select_cell(1, 0)
    after(win)
    return True


def double_click(win, r, c):
    """Double-click in a PivotTable: a value shows its details; a value's caption opens its settings."""
    g = win.grid
    i = win.book.pivot_at(g.sheet, r, c)
    if i is None:
        return False
    g.select_cell(r, c)
    if win.book.pivot_path(g.sheet, r, c) is not None:
        show_details(win, r, c)
        return True
    af = active_field(win)
    if af is not None and af[0] == "value":
        value_settings(win, i, af[2])
    elif af is not None:
        field_settings(win, i, af[2])
    return True


def clear(win, what):
    i = current(win)
    if i is None:
        return
    p = win.book.pivot(i)
    if what == "filters":
        fields = [dict(f, hidden=[]) for f in p["fields"]]
        apply(win, i, {"fields": fields, "pages": [(f, None) for f, _ in p["pages"]]}, "Clear Filters")
    else:
        # Clear All: an empty PivotTable, at the same place
        apply(win, i, {"rows": [], "cols": [], "pages": [], "values": [], "fields": [dict(f, hidden=[]) for f in p["fields"]]}, "Clear All")


def select(win, what):
    i = current(win)
    if i is None:
        return
    b = win.book
    g = win.grid
    p = b.pivot(i)
    t = p["table"] or p["out"]
    if what == "entire":
        g.select_range(tuple(p["out"]))
    elif what == "values":
        fr = None
        for r in range(t[0], t[2] + 1):
            for c in range(t[1], t[3] + 1):
                if b.pivot_path(g.sheet, r, c) is not None:
                    fr = (r, c) if fr is None else fr
        if fr is None:
            return
        g.select_range((fr[0], fr[1], t[2], t[3]))
    else:
        g.select_range(tuple(t))
    g.refresh()
    win.on_selection_changed()


def refresh(win, every=False):
    b = win.book
    from . import model_ui
    model_ui.rebuild(win)
    try:
        if every:
            b.refresh_all_pivots()
        else:
            i = current(win)
            if i is None:
                return
            b.refresh_pivot(i)
    except ValueError as e:
        win.dialogs.message(str(e), APP_NAME)
    after(win)


def set_name(win, text):
    i = current(win)
    if i is None:
        return
    if apply(win, i, {"name": text.strip()}, "PivotTable Name"):
        win.grid.area.grab_focus()


def set_active_field_name(win, text):
    i = current(win)
    af = active_field(win)
    if i is None or af is None:
        return
    if af[0] != "value":
        win.dialogs.info(NOT_HERE % ("Renaming a field", APP_NAME), "PivotTable")
        sync(win)
        return
    p = win.book.pivot(i)
    vals = p["values"]
    vals[af[2]] = dict(vals[af[2]], name=text.strip())
    apply(win, i, {"values": vals}, "Rename")


def design(win, arg):
    """Design tab: subtotals / grand totals / report layout / blank rows / options / style."""
    i = current(win)
    if i is None:
        return
    key, _, val = arg.partition(":")
    d = {}
    if key == "subtotals":
        d["subtotals"] = val
    elif key == "grand":
        d["grand_rows"], d["grand_cols"] = {"off": (False, False), "on": (True, True), "rows": (True, False),
                                            "cols": (False, True)}[val]
    elif key == "layout":
        if val in ("compact", "outline", "tabular"):
            d["layout"] = val
        else:
            d["repeat_labels"] = val == "repeat"
    elif key == "blank":
        d["blank_rows"] = val == "on"
    elif key == "option":
        k, _, on = val.partition(":")
        d[k] = on == "1"
    elif key == "style":
        d["style"] = val
    apply(win, i, d, "PivotTable Style" if key == "style" else "PivotTable Layout")


def summarize(win, agg):
    i = current(win)
    af = active_field(win)
    if i is None or af is None or af[0] != "value":
        return
    p = win.book.pivot(i)
    vals = p["values"]
    v = dict(vals[af[2]])
    fname = p["fields"][v["field"]]["name"]
    if re.match(r"^(%s) of %s\d*$" % ("|".join(AGG_NAME.values()), re.escape(fname)), v["name"]):
        q = dict(p)
        q["values"] = [x for j, x in enumerate(vals) if j != af[2]]
        v["name"] = unique_name(q, "%s of %s" % (AGG_NAME[agg], fname))
    v["agg"] = agg
    vals[af[2]] = v
    apply(win, i, {"values": vals}, "Summarize Values By")


def show_as(win, code):
    i = current(win)
    af = active_field(win)
    if i is None or af is None or af[0] != "value":
        return
    if code not in dict(SHOW):
        win.dialogs.info(NOT_HERE % ("This calculation", APP_NAME), "PivotTable")
        return
    p = win.book.pivot(i)
    vals = p["values"]
    vals[af[2]] = dict(vals[af[2]], show=code)
    apply(win, i, {"values": vals}, "Show Values As")


def remove_active(win):
    i = current(win)
    af = active_field(win)
    if i is None or af is None:
        return
    p = win.book.pivot(i)
    if af[0] == "value":
        apply(win, i, {"values": [v for k, v in enumerate(p["values"]) if k != af[2]]}, "Remove Field")
    else:
        f = af[2]
        apply(win, i, {"rows": [x for x in p["rows"] if x != f], "cols": [x for x in p["cols"] if x != f],
                       "pages": [x for x in p["pages"] if x[0] != f]}, "Remove Field")


def sort_active(win, d):
    i = current(win)
    af = active_field(win)
    if i is None or af is None or af[0] == "value":
        if af is not None and af[0] == "value":
            win.dialogs.info(NOT_HERE % ("Sorting by values", APP_NAME) + " Sort a row or column field.", "PivotTable")
        return
    p = win.book.pivot(i)
    fields = [dict(x) for x in p["fields"]]
    fields[af[2]]["sort"] = d
    apply(win, i, {"fields": fields}, "Sort")


def menu_items(win):
    """The cell menu in a PivotTable."""
    af = active_field(win)
    items = [("Copy", "copy"), None, ("Format Cells...", "format-cells"), ("Number Format...", "pivot-number-format"),
             ("Refresh", "pivot-refresh"), None, ("Sort Smallest to Largest", "pivot-sort:1"), ("Sort Largest to Smallest", "pivot-sort:-1"),
             None]
    if af is not None:
        items.append(('Remove "%s"' % af[1], "pivot-remove-field"))
    if af is not None and af[0] == "value":
        items.append(("Summarize Values By", [(label, "pivot-summarize:" + code) for code, label in AGGS[:6]]))
        items.append(("Show Values As", [(label, "pivot-show-as:" + code) for code, label in SHOW]))
        items.append(None)
        items.append(("Show Details", "pivot-details"))
        items.append(("Value Field Settings...", "pivot-field-settings"))
    elif af is not None:
        items.append(("Field Settings...", "pivot-field-settings"))
    items += [("PivotTable Options...", "pivot-options"), ("Show Field List", "pivot-field-list")]
    return items


def number_format(win):
    i = current(win)
    af = active_field(win)
    if i is None or af is None or af[0] != "value":
        return win.run("format-cells")
    dlg = ValueSettings(win, i, af[2])
    dlg.number_format()
    dlg.apply()
    dlg.d.destroy()


# ---------------------------------------------------------------- the ribbon tabs
def style_label(name):
    for fam, _ in STYLE_FAMILIES:
        if name.startswith("PivotStyle" + fam):
            return "%s %s" % (fam, name[len("PivotStyle" + fam):])
    return name


def style_gallery(win, on_pick):
    pop = Gtk.Popover()
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(420)
    sw.set_min_content_width(520)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(8)
    sw.add(box)
    pop.add(sw)
    pop.buttons = []
    i = current(win)
    p = win.book.pivot(i) if i is not None else {"banded_rows": False, "banded_cols": False}
    for fam, n in STYLE_FAMILIES:
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>%s</b>" % fam)
        box.pack_start(lab, False, False, 0)
        flow = Gtk.FlowBox()
        flow.set_selection_mode(Gtk.SelectionMode.NONE)
        flow.set_max_children_per_line(7)
        flow.set_min_children_per_line(7)
        for k in range(1, n + 1):
            name = "PivotStyle%s%d" % (fam, k)
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_tooltip_text(style_label(name))
            area = Gtk.DrawingArea()
            area.set_size_request(61, 46)
            area.connect("draw", lambda a, cr, name=name: (win.book.render_pivot_style(
                cr, name, float(a.get_allocated_width()), float(a.get_allocated_height()), bool(p["banded_rows"]),
                bool(p["banded_cols"])), False)[1])
            b.add(area)
            b.style_name = name

            def go(_b, name=name):
                pop.popdown()
                GLib.idle_add(lambda: (on_pick(name), False)[1])
            b.connect("clicked", go)
            flow.add(b)
            pop.buttons.append(b)
        box.pack_start(flow, False, False, 0)
    clear = Gtk.ModelButton(text="Clear")
    clear.connect("clicked", lambda _b: (pop.popdown(), GLib.idle_add(lambda: (on_pick("none"), False)[1])))
    box.pack_start(clear, False, False, 0)
    sw.show_all()
    return pop


def build_tabs(win, B):
    from .ribbon import Tab
    r = win.ribbon
    # ---- PivotTable Analyze
    t = r.building(Tab("PivotTable Analyze", "JT", contextual=True))
    win.pivot_name = Gtk.Entry()
    win.pivot_name.set_width_chars(12)
    win.pivot_name.set_name("pivot-name")
    win.pivot_name.connect("activate", lambda e: set_name(win, e.get_text()))
    nb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    nb.pack_start(Gtk.Label(label="PivotTable Name:", xalign=0), False, False, 0)
    nb.pack_start(win.pivot_name, False, False, 0)
    t.controls.append(win.pivot_name)
    B.group(t, "PivotTable", [nb, B.small("Options", "r-pivot-options-sm", "pivot-options", "T",
                                          menu=[("Options", "pivot-options"), ("Show Report Filter Pages...", "pivot-filter-pages"),
                                                ("Generate GetPivotData", "pivot-getpivotdata")])])
    win.pivot_field = Gtk.Entry()
    win.pivot_field.set_width_chars(12)
    win.pivot_field.set_name("pivot-active-field")
    win.pivot_field.connect("activate", lambda e: set_active_field_name(win, e.get_text()))
    fb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    fb.pack_start(Gtk.Label(label="Active Field:", xalign=0), False, False, 0)
    fb.pack_start(win.pivot_field, False, False, 0)
    t.controls.append(win.pivot_field)
    B.group(t, "Active Field", [[fb, B.small("Field Settings", "r-pivot-field-settings-sm", "pivot-field-settings", "G")],
                                B.big("Drill\nDown", "r-pivot-drill-down", "pivot-drill-down", "DD"),
                                B.big("Drill\nUp", "r-pivot-drill-up", "pivot-drill-up", "DU"),
                                [B.small("Expand Field", "r-pivot-expand-sm", "pivot-expand", "A"),
                                 B.small("Collapse Field", "r-pivot-collapse-sm", "pivot-collapse", "C")]])
    B.group(t, "Group", [[B.small("Group Selection", "r-pivot-group-sm", "pivot-group", "K"),
                          B.small("Ungroup", "r-pivot-ungroup-sm", "pivot-ungroup", "U"),
                          B.small("Group Field", "r-pivot-group-field-sm", "pivot-group-field", "F")]])
    B.group(t, "Filter", [[B.small("Insert Slicer", "r-slicer-sm", "insert-slicer", "SF"),
                           B.small("Insert Timeline", "r-timeline-sm", "insert-timeline", "IT"),
                           B.small("Filter Connections", "r-filter-connections-sm", "filter-connections", "FC")]])
    B.group(t, "Data", [B.big("Refresh", "r-refresh-all", "pivot-refresh", "R",
                              menu=[("Refresh", "pivot-refresh"), ("Refresh All", "pivot-refresh-all")]),
                        B.big("Change Data\nSource", "r-pivot-change-source", "pivot-change-source", "I")])
    B.group(t, "Actions", [[B.small("Clear", "r-pivot-clear-sm", None, "CA", menu=[("Clear All", "pivot-clear:all"),
                                                                                  ("Clear Filters", "pivot-clear:filters")]),
                            B.small("Select", "r-pivot-select-sm", None, "SE", menu=[("Labels and Values", "pivot-select:both"),
                                                                                    ("Values", "pivot-select:values"),
                                                                                    ("Entire PivotTable", "pivot-select:entire")]),
                            B.small("Move PivotTable", "r-pivot-move-sm", "pivot-move", "M")]])
    B.group(t, "Calculations", [[B.small("Fields, Items, & Sets", "r-pivot-calc-sm", "pivot-calculated", "J"),
                                 B.small("OLAP Tools", "r-pivot-olap-sm", "pivot-olap", "P"),
                                 B.small("Relationships", "r-pivot-relationships-sm", "pivot-relationships", "RE")]])
    B.group(t, "Tools", [B.big("PivotChart", "r-pivot-chart", "pivot-chart", "C"),
                         B.big("Recommended\nPivotTables", "r-recommended-pivots", "recommended-pivots", "SP")])
    B.group(t, "Show", [B.big("Field\nList", "r-pivot-field-list", "pivot-field-list", "L"),
                        B.big("+/-\nButtons", "r-pivot-buttons", "pivot-expand-buttons", "B"),
                        B.big("Field\nHeaders", "r-pivot-field-headers", "pivot-field-headers", "H")])
    r.add_tab(t)
    r.show_contextual("PivotTable Analyze", False)
    # ---- Design
    t = r.building(Tab("Design", "JY", contextual=True))
    B.group(t, "Layout", [
        B.big("Subtotals", "r-pivot-subtotals", None, "ST", menu=[("Do Not Show Subtotals", "pivot-design:subtotals:none"),
                                                                ("Show all Subtotals at Bottom of Group", "pivot-design:subtotals:bottom"),
                                                                ("Show all Subtotals at Top of Group", "pivot-design:subtotals:top")]),
        B.big("Grand\nTotals", "r-pivot-grand", None, "GT", menu=[("Off for Rows and Columns", "pivot-design:grand:off"),
                                                                 ("On for Rows and Columns", "pivot-design:grand:on"),
                                                                 ("On for Rows Only", "pivot-design:grand:rows"),
                                                                 ("On for Columns Only", "pivot-design:grand:cols")]),
        B.big("Report\nLayout", "r-pivot-report-layout", None, "RP", menu=[("Show in Compact Form", "pivot-design:layout:compact"),
                                                                          ("Show in Outline Form", "pivot-design:layout:outline"),
                                                                          ("Show in Tabular Form", "pivot-design:layout:tabular"), None,
                                                                          ("Repeat All Item Labels", "pivot-design:layout:repeat"),
                                                                          ("Do Not Repeat Item Labels", "pivot-design:layout:norepeat")]),
        B.big("Blank\nRows", "r-pivot-blank-rows", None, "B", menu=[("Insert Blank Line after Each Item", "pivot-design:blank:on"),
                                                                   ("Remove Blank Line after Each Item", "pivot-design:blank:off")]),
    ])
    win.pivot_checks = {}
    cols = []
    for lab, key, kt in (("Row Headers", "row_headers", "R"), ("Column Headers", "col_headers", "C"),
                         ("Banded Rows", "banded_rows", "BR"), ("Banded Columns", "banded_cols", "BC")):
        chk = Gtk.CheckButton(label=lab)
        chk.set_name("pivot-opt-" + key)
        chk.action = "pivot-design:option:" + key
        chk.keytip = kt
        chk.press = (lambda c=chk: c.set_active(not c.get_active()))
        chk.connect("toggled", lambda c, key=key: None if win.syncing_checks else win.run("pivot-design:option:%s:%d" % (key, c.get_active())))
        t.controls.append(chk)
        win.pivot_checks[key] = chk
        cols.append(chk)
    B.group(t, "PivotTable Style Options", [cols[0:2], cols[2:4]])
    B.group(t, "PivotTable Styles", [B.big("PivotTable\nStyles", "r-pivot-styles", None, "S",
                                           menu=lambda: style_gallery(win, lambda n: win.run("pivot-design:style:%s" % ("" if n == "none" else n))))])
    r.add_tab(t)
    r.show_contextual("Design", False)
