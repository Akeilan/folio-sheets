"""Waterfall, Funnel, Histogram, Pareto, Box and Whisker, Treemap, Sunburst and Filled Map
charts: inserting them, changing their kind, data and title, and a map's options."""
from gi.repository import Gtk

from .core import parse_ref, range_a1

KINDS = [("Waterfall", "waterfall"), ("Funnel", "funnel"), ("Histogram", "histogram"), ("Pareto", "pareto"),
         ("Box and Whisker", "boxwhisker"), ("Treemap", "treemap"), ("Sunburst", "sunburst"), ("Filled Map", "regionmap")]
LABEL = dict((k, lab) for lab, k in KINDS)
GROUPS = {"hierarchy": ["treemap", "sunburst"], "statistic": ["histogram", "pareto", "boxwhisker"],
          "waterfall": ["waterfall", "funnel"], "map": ["regionmap"]}


def _range(win):
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    if rg[0:2] == rg[2:4]:
        rg = tuple(win.book.current_region(g.sheet, rg[0], rg[1]))
    return rg


def insert(win, kind, rg=None):
    from .chart_ui import _visible_center
    g, b = win.grid, win.book
    rg = rg or _range(win)
    x, y = _visible_center(win)
    try:
        i = b.insert_chartex(g.sheet, rg, kind, x, y)
    except ValueError as e:
        win.dialogs.message(str(e), "Insert Chart")
        return None
    g.sel_objects = [i]
    g.refresh()
    win.update_title()
    win.on_object_selection()
    return i


class KindDialog:
    """All of the newer charts with a preview: insert one, or change a chart's kind."""

    def __init__(self, win, kinds=None, current=None, rg=None, title="Insert Chart"):
        self.win, self.rg = win, rg
        self.kind = current or (kinds or [k for _, k in KINDS])[0]
        d = self.d = Gtk.Dialog(title=title, transient_for=win, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_size(700, 420)
        box = Gtk.Box(spacing=12)
        box.set_border_width(12)
        d.get_content_area().pack_start(box, True, True, 0)
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        group = None
        self.radios = {}
        for lab, k in KINDS:
            if kinds and k not in kinds:
                continue
            rb = Gtk.RadioButton.new_with_label_from_widget(group, lab)
            group = group or rb
            rb.set_active(k == self.kind)
            rb.connect("toggled", lambda w, k=k: (w.get_active() and self._pick(k)))
            self.radios[k] = rb
            side.pack_start(rb, False, False, 0)
        box.pack_start(side, False, False, 0)
        self.preview = Gtk.DrawingArea()
        self.preview.set_size_request(480, 288)
        self.preview.connect("draw", self._draw)
        box.pack_start(self.preview, True, True, 0)
        d.show_all()

    def _pick(self, k):
        self.kind = k
        self.preview.queue_draw()

    def _draw(self, w, cr):
        if self.rg is None:
            return
        a = w.get_allocation()
        try:
            self.win.book.render_chartex_preview(self.win.grid.sheet, cr, float(a.width), float(a.height), self.rg, self.kind)
        except ValueError:
            pass

    def run(self):
        r = self.d.run()
        k = self.kind
        self.d.destroy()
        return k if r == Gtk.ResponseType.OK else None


def more(win, group):
    rg = _range(win)
    k = KindDialog(win, GROUPS.get(group), rg=rg).run()
    return insert(win, k, rg) if k else None


def change_type(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    rg = None
    if info and info["values"]:
        rg = _source_rect(info)
    k = KindDialog(win, current=info["kind"], rg=rg, title="Change Chart Type").run()
    if not k:
        return False
    b.set_chartex(g.sheet, i, kind=k)
    g.refresh()
    win.update_title()
    return True


def _source_rect(info):
    refs = info["cats"] + info["values"]
    rects = [parse_ref(f.rsplit("!", 1)[-1].replace("$", "")) for f in refs]
    rects = [r for r in rects if r]
    if not rects:
        return None
    r1 = min(r[0] for r in rects) - 1
    return (max(0, r1), min(r[1] for r in rects), max(r[2] for r in rects), max(r[3] for r in rects))


def select_data(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    cur = _source_rect(info)
    t = win.dialogs.ask_text("Select Data Source", "Chart data range:", range_a1(cur) if cur else "")
    if not t:
        return False
    rg = parse_ref(t.rsplit("!", 1)[-1].replace("$", "").lstrip("="))
    if rg is None:
        win.dialogs.message("Reference isn't valid.", "Select Data Source")
        return False
    try:
        b.set_chartex(g.sheet, i, rg=tuple(rg))
    except ValueError as e:
        win.dialogs.message(str(e), "Select Data Source")
        return False
    g.refresh()
    win.update_title()
    return True


def edit_title(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    t = win.dialogs.ask_text("Chart Title", "Title (empty: the series name):", info["own_title"] or "")
    if t is None:
        return False
    if t.strip():
        b.set_chartex(g.sheet, i, title=t)
    else:
        b.set_chartex(g.sheet, i, clear_title=True)
    g.refresh()
    win.update_title()
    return True


# ---------------------------------------------------------------- Filled Map options (S3-13)
PROJECTIONS = [("automatic", "Automatic"), ("mercator", "Mercator"), ("miller", "Miller"), ("robinson", "Robinson"), ("albers", "Albers")]
AREAS = [("automatic", "Automatic"), ("dataOnly", "Only regions with data"), ("world", "World")]
LABELS = [("none", "None"), ("bestFitOnly", "Best Fit Only"), ("showAll", "Show All")]


def _rgba(c):
    from gi.repository import Gdk
    v = Gdk.RGBA()
    v.parse("#%06X" % c)
    return v


def _hex(btn):
    c = btn.get_rgba()
    return (int(round(c.red * 255)) << 16) | (int(round(c.green * 255)) << 8) | int(round(c.blue * 255))


class MapOptions:
    """Format Data Series > Series Options for a Filled Map: projection, area, labels, colours."""

    def __init__(self, win, i):
        self.win, self.i = win, i
        info = win.book.chartex_info(win.grid.sheet, i)
        m = info["map"]
        d = self.d = Gtk.Dialog(title="Format Data Series", transient_for=win, modal=True)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.add_button("OK", Gtk.ResponseType.OK)
        grid = Gtk.Grid(column_spacing=8, row_spacing=6, border_width=12)
        d.get_content_area().pack_start(grid, True, True, 0)

        def combo(row, label, items, cur):
            grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
            cb = Gtk.ComboBoxText()
            for k, lab in items:
                cb.append(k, lab)
            cb.set_active_id(cur)
            grid.attach(cb, 1, row, 2, 1)
            return cb
        self.proj = combo(0, "Map projection:", PROJECTIONS, m["projection"])
        self.area = combo(1, "Map area:", AREAS, m["area"])
        self.labels = combo(2, "Map labels:", LABELS, m["labels"])
        grid.attach(Gtk.Separator(), 0, 3, 3, 1)
        grid.attach(Gtk.Label(label="Series Color", xalign=0), 0, 4, 3, 1)
        cols = m["colors"]
        accent = win.book.theme_colors()[4]
        light = tuple(int(((accent >> s) & 255) * 0.18 + 255 * 0.82) for s in (16, 8, 0))
        dark = tuple(int(((accent >> s) & 255) * 0.7) for s in (16, 8, 0))
        lo = cols[0] if cols else (light[0] << 16 | light[1] << 8 | light[2])
        hi = cols[2] if cols else (dark[0] << 16 | dark[1] << 8 | dark[2])
        mid = cols[1] if cols and cols[1] is not None else 0xFFEB84
        self.kind = combo(5, "Colors:", [("auto", "Automatic"), ("two", "Sequential (2-color)"), ("three", "Diverging (3-color)")],
                          "auto" if not cols else ("three" if cols[1] is not None else "two"))
        self.btn = {}
        for row, (key, label, c) in enumerate((("min", "Minimum:", lo), ("mid", "Midpoint:", mid), ("max", "Maximum:", hi)), start=6):
            grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
            b = Gtk.ColorButton.new_with_rgba(_rgba(c))
            grid.attach(b, 1, row, 1, 1)
            self.btn[key] = b
        self.kind.connect("changed", lambda *_: self._sync())
        if m["unknown"]:
            note = Gtk.Label(label="Not found on the map: " + ", ".join(m["unknown"][:8]) + (" ..." if len(m["unknown"]) > 8 else ""), xalign=0)
            note.set_line_wrap(True)
            note.get_style_context().add_class("dim-label")
            grid.attach(note, 0, 9, 3, 1)
        if m["categories"]:
            self.kind.set_sensitive(False)
        d.show_all()
        self._sync()

    def _sync(self):
        k = self.kind.get_active_id()
        for key in ("min", "max"):
            self.btn[key].set_sensitive(k != "auto")
        self.btn["mid"].set_sensitive(k == "three")

    def apply(self):
        k = self.kind.get_active_id()
        colors = "auto" if k == "auto" else (_hex(self.btn["min"]), _hex(self.btn["mid"]) if k == "three" else None, _hex(self.btn["max"]))
        self.win.book.set_map_options(self.win.grid.sheet, self.i, projection=self.proj.get_active_id(), area=self.area.get_active_id(),
                                      labels=self.labels.get_active_id(), colors=colors)
        self.win.grid.refresh()
        self.win.update_title()

    def run(self):
        r = self.d.run()
        if r == Gtk.ResponseType.OK:
            self.apply()
        self.d.destroy()
        return r == Gtk.ResponseType.OK


def map_options(win, i):
    return MapOptions(win, i).run()
