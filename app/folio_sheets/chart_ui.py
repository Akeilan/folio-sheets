"""Charts: Insert Chart / Change Chart Type, Select Data, Format Chart, and the
commands of the Chart Design tab."""
import copy

from gi.repository import Gtk

# (page, [(label, kind)])
PAGES = [
    ("Column", [("Clustered Column", "col"), ("Stacked Column", "col-stacked"), ("100% Stacked Column", "col-percent")]),
    ("Bar", [("Clustered Bar", "bar"), ("Stacked Bar", "bar-stacked"), ("100% Stacked Bar", "bar-percent")]),
    ("Line", [("Line", "line"), ("Stacked Line", "line-stacked"), ("100% Stacked Line", "line-percent"),
              ("Line with Markers", "line-markers")]),
    ("Pie", [("Pie", "pie"), ("Doughnut", "doughnut")]),
    ("Area", [("Area", "area"), ("Stacked Area", "area-stacked"), ("100% Stacked Area", "area-percent")]),
    ("X Y (Scatter)", [("Scatter", "scatter"), ("Scatter with Smooth Lines", "scatter-smooth"),
                       ("Scatter with Straight Lines", "scatter-lines")]),
    ("Combo", [("Clustered Column - Line", "combo"), ("Clustered Column - Line on Secondary Axis", "combo-secondary")]),
]

KIND_LABELS = {k: lab for _, items in PAGES for lab, k in items}
SERIES_KINDS = [("Clustered Column", "col"), ("Line", "line"), ("Line with Markers", "line-markers"), ("Area", "area")]
LEGENDS = [("None", "none"), ("Right", "r"), ("Top", "t"), ("Left", "l"), ("Bottom", "b"), ("Top Right", "tr")]
PALETTES = ["Colorful", "Monochrome 1", "Monochrome 2", "Monochrome 3", "Monochrome 4", "Monochrome 5", "Monochrome 6"]


def page_of(kind):
    for name, items in PAGES:
        if any(k == kind for _, k in items):
            return name
    if kind.startswith("combo"):
        return "Combo"
    return "Column"


def recommended(win, rg):
    """Kinds that suit the data, best first."""
    b = win.book
    s = win.grid.sheet
    try:
        sp = b.range_spec(s, rg, "col")
    except ValueError:
        return []
    n = len(sp["series"])
    has_cats = any(x["cat"] for x in sp["series"])
    out = ["col", "line", "bar"]
    if n == 1:
        out.insert(1, "pie")
    if not has_cats:
        out.insert(0, "scatter") if n >= 1 and rg[3] > rg[1] else None
    if n >= 2:
        out += ["col-stacked", "combo"]
    out.append("area")
    seen = []
    for k in out:
        if k not in seen:
            seen.append(k)
    return seen


def _apply_kind(spec, kind):
    spec = copy.deepcopy(spec)
    real = "combo" if kind.startswith("combo") else kind
    if real == "combo" and spec["kind"] != "combo":
        n = len(spec["series"])
        for k, ss in enumerate(spec["series"]):
            ss["kind"] = "line" if (k == n - 1 and n > 1) else "col"
            ss["secondary"] = kind == "combo-secondary" and ss["kind"] == "line"
    elif kind == "combo-secondary":
        for ss in spec["series"]:
            ss["secondary"] = ss["kind"] != "col"
    spec["kind"] = real
    return spec


class ChartTypeDialog:
    """Insert Chart (a new chart of rg) or Change Chart Type (chart index)."""

    def __init__(self, win, rg=None, index=None, page=None):
        self.win = win
        self.rg = rg
        self.index = index
        b = win.book
        s = win.grid.sheet
        if index is not None:
            self.spec = b.chart_spec(s, index)
        else:
            self.spec = b.range_spec(s, rg, "col")
        self.kind = self.spec["kind"]
        self.recs = recommended(win, rg) if index is None and rg is not None else []
        d = self.d = Gtk.Dialog(title="Insert Chart" if index is None else "Change Chart Type", transient_for=win, modal=True)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        d.set_default_size(820, 560)
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_border_width(12)
        d.get_content_area().pack_start(box, True, True, 0)
        self.pages = Gtk.ListBox()
        self.pages.set_name("pages")
        self.page_names = (["Recommended Charts"] if self.recs else []) + [p for p, _ in PAGES]
        for p in self.page_names:
            row = Gtk.ListBoxRow()
            lab = Gtk.Label(label=p, xalign=0)
            lab.set_margin_start(8)
            lab.set_margin_end(8)
            lab.set_margin_top(4)
            lab.set_margin_bottom(4)
            row.add(lab)
            row.page = p
            self.pages.add(row)
        box.pack_start(self.pages, False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.pack_start(right, True, True, 0)
        self.subtypes = Gtk.FlowBox()
        self.subtypes.set_name("subtypes")
        self.subtypes.set_selection_mode(Gtk.SelectionMode.NONE)
        self.subtypes.set_max_children_per_line(6)
        right.pack_start(self.subtypes, False, False, 0)
        self.title_lab = Gtk.Label(xalign=0)
        self.title_lab.get_style_context().add_class("dim-label")
        right.pack_start(self.title_lab, False, False, 0)
        self.preview = Gtk.DrawingArea()
        self.preview.set_size_request(480, 288)
        self.preview.connect("draw", self.on_draw)
        right.pack_start(self.preview, True, True, 0)
        self.combo_grid = Gtk.Grid(row_spacing=4, column_spacing=12)
        self.combo_grid.set_name("combo-series")
        right.pack_start(self.combo_grid, False, False, 0)
        self.buttons = []
        self.pages.connect("row-selected", self.on_page)
        start = page or (("Recommended Charts" if self.recs else "Column") if index is None else page_of(self.kind))
        self.select_page(start)
        d.show_all()
        self.sync_combo()

    def select_page(self, name):
        for row in self.pages.get_children():
            if row.page == name:
                self.pages.select_row(row)
                return

    def on_page(self, lb, row):
        if row is None:
            return
        for c in self.subtypes.get_children():
            self.subtypes.remove(c)
        self.buttons = []
        if row.page == "Recommended Charts":
            items = [(KIND_LABELS.get(k, k), k) for k in self.recs]
        else:
            items = dict(PAGES)[row.page]
        group = None
        for lab, k in items:
            rb = Gtk.RadioButton.new_with_label_from_widget(group, lab)
            rb.set_mode(False)
            rb.kind = k
            group = group or rb
            rb.connect("toggled", self.on_subtype)
            self.subtypes.add(rb)
            self.buttons.append(rb)
        self.subtypes.show_all()
        pick = next((b for b in self.buttons if b.kind == self.kind), self.buttons[0] if self.buttons else None)
        if pick is not None:
            pick.set_active(True)
            self.set_kind(pick.kind)

    def on_subtype(self, rb):
        if rb.get_active():
            self.set_kind(rb.kind)

    def set_kind(self, kind):
        self.kind = kind
        self.spec = _apply_kind(self.spec, kind)
        self.title_lab.set_text(KIND_LABELS.get(kind, kind))
        self.sync_combo()
        self.preview.queue_draw()

    def sync_combo(self):
        for c in self.combo_grid.get_children():
            self.combo_grid.remove(c)
        if self.spec["kind"] != "combo":
            self.combo_grid.hide()
            return
        self.combo_grid.attach(Gtk.Label(label="Series Name", xalign=0), 0, 0, 1, 1)
        self.combo_grid.attach(Gtk.Label(label="Chart Type", xalign=0), 1, 0, 1, 1)
        self.combo_grid.attach(Gtk.Label(label="Secondary Axis", xalign=0), 2, 0, 1, 1)
        b = self.win.book
        for k, ss in enumerate(self.spec["series"]):
            name = ss["name"].strip('"') if ss["name"].startswith('"') else (self._ref_text(ss["name"]) or "Series%d" % (k + 1))
            self.combo_grid.attach(Gtk.Label(label=name, xalign=0), 0, k + 1, 1, 1)
            cb = Gtk.ComboBoxText()
            cb.set_name("kind-%d" % k)
            for lab, kk in SERIES_KINDS:
                cb.append(kk, lab)
            cb.set_active_id(ss["kind"] if ss["kind"] in dict((x[1], 1) for x in SERIES_KINDS) else "col")
            cb.connect("changed", lambda w, k=k: self._series_kind(k, w.get_active_id()))
            self.combo_grid.attach(cb, 1, k + 1, 1, 1)
            ch = Gtk.CheckButton()
            ch.set_name("secondary-%d" % k)
            ch.set_active(ss["secondary"])
            ch.connect("toggled", lambda w, k=k: self._series_secondary(k, w.get_active()))
            self.combo_grid.attach(ch, 2, k + 1, 1, 1)
        del b
        self.combo_grid.show_all()

    def _ref_text(self, ref):
        if not ref:
            return ""
        try:
            pts = self.win.book.chart_ref_text(ref)
        except Exception:
            return ref
        return pts

    def _series_kind(self, k, kind):
        self.spec["series"][k]["kind"] = kind
        self.preview.queue_draw()

    def _series_secondary(self, k, on):
        self.spec["series"][k]["secondary"] = on
        self.preview.queue_draw()

    def on_draw(self, area, cr):
        a = area.get_allocation()
        w, h = a.width, a.height
        cw, ch = min(w, 560), min(h, 336)
        cr.translate((w - cw) / 2, (h - ch) / 2)
        try:
            self.win.book.render_spec(self.win.grid.sheet, cr, float(cw), float(ch), self.spec, 1.0)
        except ValueError as e:
            cr.set_source_rgb(0.4, 0.4, 0.4)
            cr.move_to(12, 24)
            cr.show_text(str(e))
        return False

    def run(self):
        r = self.d.run()
        spec = self.spec
        self.d.destroy()
        return spec if r == Gtk.ResponseType.OK else None


def _visible_center(win):
    g = win.grid
    a = g.area.get_allocation()
    x, y = win.book.sheet_point(g.sheet, a.width / 2.0, a.height / 2.0, g.params())
    return max(0.0, x - 240.0), max(0.0, y - 144.0)


def chart_range(win):
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    return rg


def insert_chart(win, kind=None, page=None):
    g = win.grid
    rg = chart_range(win)
    b = win.book
    err = b.chart_check(g.sheet, rg, "col")
    if err:
        win.dialogs.message(err, "Insert Chart")
        return False
    if kind is None:
        spec = ChartTypeDialog(win, rg=rg, page=page).run()
        if spec is None:
            return False
    else:
        spec = _apply_kind(b.range_spec(g.sheet, rg, "scatter" if kind.startswith("scatter") else "col"), kind)
    x, y = _visible_center(win)
    try:
        i = b.add_chart_spec(g.sheet, spec, x, y)
    except ValueError as e:
        win.dialogs.message(str(e), "Insert Chart")
        return False
    g.select_object(i)
    win.on_object_selection(switch_tab=True)
    win.update_title()
    return True


def change_type(win, i):
    spec = ChartTypeDialog(win, index=i).run()
    if spec is None:
        return False
    try:
        win.book.set_chart_spec(win.grid.sheet, i, spec, "Change Chart Type")
    except ValueError as e:
        win.dialogs.message(str(e), "Change Chart Type")
        return False
    win.grid.refresh()
    win.update_title()
    return True


def select_data(win, i):
    g = win.grid
    b = win.book
    s = g.sheet
    spec = b.chart_spec(s, i)
    d = Gtk.Dialog(title="Select Data Source", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    d.set_default_size(640, 420)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    d.get_content_area().pack_start(box, True, True, 0)
    row = Gtk.Box(spacing=8)
    row.pack_start(Gtk.Label(label="Chart data range:"), False, False, 0)
    rng = Gtk.Entry(text="=" + (b.chart_range_text(s, i) or "") if b.chart_range_text(s, i) else "")
    rng.set_name("range")
    rng.set_activates_default(True)
    row.pack_start(rng, True, True, 0)
    box.pack_start(row, False, False, 0)
    switch = Gtk.Button(label="Switch Row/Column")
    switch.set_name("switch")
    box.pack_start(switch, False, False, 0)
    store = Gtk.ListStore(str, str, str)
    for ss in spec["series"]:
        store.append([ss["name"], ss["val"], ss["cat"]])
    view = Gtk.TreeView(model=store)
    view.set_name("series")
    for k, title in enumerate(("Series Name", "Series Values", "Category (X) Labels")):
        cell = Gtk.CellRendererText(editable=True)
        cell.connect("edited", lambda _c, path, text, k=k: store.__setitem__(path, _set(store[path], k, text)))
        col = Gtk.TreeViewColumn(title, cell, text=k)
        col.set_resizable(True)
        col.set_expand(True)
        view.append_column(col)
    sw = Gtk.ScrolledWindow()
    sw.set_vexpand(True)
    sw.add(view)
    box.pack_start(sw, True, True, 0)
    btns = Gtk.Box(spacing=8)
    add = Gtk.Button(label="Add")
    add.set_name("add")
    rem = Gtk.Button(label="Remove")
    rem.set_name("remove")
    btns.pack_start(add, False, False, 0)
    btns.pack_start(rem, False, False, 0)
    box.pack_start(btns, False, False, 0)
    state = {"switched": False}

    def on_switch(_b):
        state["switched"] = not state["switched"]
        d.response(Gtk.ResponseType.APPLY)
    switch.connect("clicked", on_switch)
    add.connect("clicked", lambda _b: store.append(['"Series%d"' % (len(store) + 1), "", spec["series"][0]["cat"] if spec["series"] else ""]))

    def on_remove(_b):
        m, it = view.get_selection().get_selected()
        if it is not None:
            m.remove(it)
    rem.connect("clicked", on_remove)
    d.show_all()
    orig_range = rng.get_text()
    while True:
        r = d.run()
        if r == Gtk.ResponseType.APPLY:
            # Switch Row/Column applies at once, as the original does
            try:
                b.switch_chart_rows(s, i)
            except ValueError as e:
                win.dialogs.message(str(e), "Select Data Source")
                continue
            spec = b.chart_spec(s, i)
            store.clear()
            for ss in spec["series"]:
                store.append([ss["name"], ss["val"], ss["cat"]])
            g.refresh()
            continue
        if r != Gtk.ResponseType.OK:
            d.destroy()
            if state["switched"]:
                b.undo()
                g.refresh()
            return False
        try:
            if rng.get_text().strip() and rng.get_text() != orig_range:
                b.set_chart_range(s, i, rng.get_text())
            else:
                spec = b.chart_spec(s, i)
                old = spec["series"]
                spec["series"] = []
                for k, rowv in enumerate(store):
                    base = dict(old[k]) if k < len(old) else {"kind": "col", "secondary": False, "color": None}
                    base.update({"name": rowv[0], "val": rowv[1], "cat": rowv[2]})
                    spec["series"].append(base)
                b.set_chart_spec(s, i, spec, "Select Data")
        except ValueError as e:
            win.dialogs.message(str(e), "Select Data Source")
            continue
        break
    d.destroy()
    g.refresh()
    win.update_title()
    return True


def _set(row, k, text):
    v = [row[0], row[1], row[2]]
    v[k] = text
    return v


def format_chart(win, i):
    g = win.grid
    b = win.book
    s = g.sheet
    spec = b.chart_spec(s, i)
    d = Gtk.Dialog(title="Format Chart", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(row_spacing=6, column_spacing=12)
    grid.set_border_width(12)
    d.get_content_area().pack_start(grid, True, True, 0)
    y = 0

    def row(label, w):
        nonlocal y
        grid.attach(Gtk.Label(label=label, xalign=0), 0, y, 1, 1)
        grid.attach(w, 1, y, 1, 1)
        y += 1
    tmode = Gtk.ComboBoxText()
    tmode.set_name("title-mode")
    for k, lab in (("none", "None"), ("auto", "Automatic"), ("text", "Custom")):
        tmode.append(k, lab)
    tmode.set_active_id(spec["title_mode"])
    row("Chart title:", tmode)
    title = Gtk.Entry(text=spec["title"] if spec["title_mode"] == "text" else (b.chart_title(s, i) or ""))
    title.set_name("title")
    title.set_activates_default(True)
    row("", title)
    tmode.connect("changed", lambda w: title.set_sensitive(w.get_active_id() == "text"))
    title.set_sensitive(spec["title_mode"] == "text")
    legend = Gtk.ComboBoxText()
    legend.set_name("legend")
    for lab, k in LEGENDS:
        legend.append(k, lab)
    legend.set_active_id(spec["legend"] if spec["legend"] in dict((k, 1) for _, k in LEGENDS) else "r")
    row("Legend:", legend)
    pie = spec["kind"] in ("pie", "doughnut")
    xt = Gtk.Entry(text=spec["x_title"])
    xt.set_name("x-title")
    yt = Gtk.Entry(text=spec["y_title"])
    yt.set_name("y-title")
    if not pie:
        row("Horizontal axis title:", xt)
        row("Vertical axis title:", yt)
    grid_chk = Gtk.CheckButton(label="Major gridlines")
    grid_chk.set_name("gridlines")
    grid_chk.set_active(spec["gridlines"])
    if not pie:
        row("", grid_chk)
    labels = Gtk.CheckButton(label="Data labels")
    labels.set_name("labels")
    labels.set_active(spec["labels"])
    row("", labels)
    pal = Gtk.ComboBoxText()
    pal.set_name("colors")
    for k, lab in enumerate(PALETTES):
        pal.append(str(k), lab)
    pal.set_active_id(str(spec["palette"]))
    row("Colors:", pal)
    d.show_all()
    r = d.run()
    if r != Gtk.ResponseType.OK:
        d.destroy()
        return False
    spec["title_mode"] = tmode.get_active_id() or "auto"
    spec["title"] = title.get_text()
    spec["legend"] = legend.get_active_id() or "none"
    spec["x_title"] = xt.get_text()
    spec["y_title"] = yt.get_text()
    spec["gridlines"] = grid_chk.get_active()
    spec["labels"] = labels.get_active()
    spec["palette"] = int(pal.get_active_id() or 0)
    if spec["palette"]:
        for ss in spec["series"]:
            ss["color"] = None
    d.destroy()
    try:
        b.set_chart_spec(s, i, spec, "Format Chart")
    except ValueError as e:
        win.dialogs.message(str(e), "Format Chart")
        return False
    g.refresh()
    win.update_title()
    return True


def add_element(win, i, what):
    """Chart Design ▸ Add Chart Element: what = "title:none|auto", "legend:<pos>",
    "axis-title-x:on|off", "axis-title-y:on|off", "labels:on|off", "gridlines:on|off"."""
    b = win.book
    s = win.grid.sheet
    spec = b.chart_spec(s, i)
    key, _, val = what.partition(":")
    if key == "title":
        spec["title_mode"] = "none" if val == "none" else ("auto" if spec["title_mode"] == "none" else spec["title_mode"])
    elif key == "legend":
        spec["legend"] = val
    elif key == "axis-title-x":
        spec["x_title"] = (spec["x_title"] or "Axis Title") if val == "on" else ""
    elif key == "axis-title-y":
        spec["y_title"] = (spec["y_title"] or "Axis Title") if val == "on" else ""
    elif key == "labels":
        spec["labels"] = val == "on"
    elif key == "gridlines":
        spec["gridlines"] = val == "on"
    b.set_chart_spec(s, i, spec, "Add Chart Element")
    win.grid.refresh()
    win.update_title()


def set_colors(win, i, n):
    b = win.book
    s = win.grid.sheet
    spec = b.chart_spec(s, i)
    spec["palette"] = n
    for ss in spec["series"]:
        ss["color"] = None
    b.set_chart_spec(s, i, spec, "Change Colors")
    win.grid.refresh()
    win.update_title()
