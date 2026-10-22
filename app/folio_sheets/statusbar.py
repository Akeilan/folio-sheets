"""The status bar: cell mode, selection statistics, view buttons, zoom."""
from gi.repository import GLib, Gtk

from . import icons

AGG_ITEMS = [("average", "Average"), ("count", "Count"), ("num_count", "Numerical Count"),
             ("min", "Minimum"), ("max", "Maximum"), ("sum", "Sum")]
DEFAULT_AGG = ["average", "count", "sum"]
ZOOM_STEPS = [10, 25, 50, 75, 100, 125, 150, 200, 300, 400]


class StatusBar(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, spacing=8)
        self.win = win
        self.get_style_context().add_class("folio-status")
        self.mode = Gtk.Label(label="Ready", xalign=0)
        self.mode.set_margin_start(8)
        self.pack_start(self.mode, False, False, 0)
        self.extra = Gtk.Label(label="", xalign=0)
        self.pack_start(self.extra, False, False, 0)
        # "Calculate" (manual mode, pending) / circular references
        self.calc = Gtk.Label(label="", xalign=0)
        self.pack_start(self.calc, False, False, 6)
        ev = Gtk.EventBox()
        ev.connect("button-press-event", self.on_menu)
        self.pack_start(ev, True, True, 0)
        self.agg_box = Gtk.Box(spacing=16)
        self.agg_labels = {}
        for key, name in AGG_ITEMS:
            lab = Gtk.Label(label="")
            lab.set_no_show_all(True)
            self.agg_labels[key] = lab
            self.agg_box.pack_start(lab, False, False, 0)
        aev = Gtk.EventBox()
        aev.add(self.agg_box)
        aev.connect("button-press-event", self.on_menu)
        self.pack_start(aev, False, False, 8)
        self.view_btns = {}
        for key, icon, tip in (("normal", "r-view-normal-sm", "Normal"),
                               ("layout", "r-view-page-layout-sm", "Page Layout"),
                               ("break", "r-view-page-break-sm", "Page Break Preview")):
            b = Gtk.Button()
            b.add(icons.image(icon, 16))
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_tooltip_text(tip)
            b.set_focus_on_click(False)
            b.get_style_context().add_class("folio-status-btn")
            b.connect("clicked", lambda _b, k=key: win.run({"normal": "view-normal", "layout": "view-page-layout",
                                                             "break": "view-page-break"}[k]))
            self.view_btns[key] = b
            self.pack_start(b, False, False, 0)
        minus = Gtk.Button(label="−")
        plus = Gtk.Button(label="+")
        for b in (minus, plus):
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_focus_on_click(False)
            b.get_style_context().add_class("folio-status-btn")
        minus.connect("clicked", lambda *_: self.step_zoom(-1))
        plus.connect("clicked", lambda *_: self.step_zoom(1))
        self.slider = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 10, 400, 1)
        self.slider.set_draw_value(False)
        self.slider.set_size_request(110, -1)
        self.slider.add_mark(100, Gtk.PositionType.BOTTOM, None)
        self.slider.connect("value-changed", self.on_slider)
        self.zoom_btn = Gtk.Button(label="100%")
        self.zoom_btn.set_relief(Gtk.ReliefStyle.NONE)
        self.zoom_btn.set_focus_on_click(False)
        self.zoom_btn.get_style_context().add_class("folio-status-btn")
        self.zoom_btn.set_tooltip_text("Zoom level")
        self.zoom_btn.connect("clicked", lambda *_: win.run("zoom-dialog"))
        self.pack_start(minus, False, False, 0)
        self.pack_start(self.slider, False, False, 0)
        self.pack_start(plus, False, False, 0)
        self.pack_start(self.zoom_btn, False, False, 4)
        self._quiet = False
        self._agg_id = 0

    def enabled(self):
        return self.win.app.settings.get("status_agg", DEFAULT_AGG)

    def set_calc(self, text):
        if self.calc.get_text() != text:
            self.calc.set_text(text)

    def set_mode(self, text):
        self.mode.set_text(text)

    def show_zoom(self, pct):
        self._quiet = True
        self.slider.set_value(pct)
        self._quiet = False
        self.zoom_btn.set_label("%d%%" % round(pct))

    def on_slider(self, s):
        if self._quiet:
            return
        v = s.get_value()
        # snap to 100 near the middle mark, as the original does
        if abs(v - 100) < 4:
            v = 100
        self.win.set_zoom(v)

    def step_zoom(self, d):
        # the − and + buttons step to the next multiple of 10 %
        cur = self.win.grid.state().zoom * 100
        base = int(round(cur))
        nxt = (base // 10 + 1) * 10 if d > 0 else ((base - 1) // 10) * 10
        self.win.set_zoom(max(10, min(400, nxt)))

    def update_aggregates(self):
        """Recompute after the selection settles (big selections are cheap
        in the engine, but not free)."""
        if self._agg_id:
            GLib.source_remove(self._agg_id)
        self._agg_id = GLib.timeout_add(60, self._agg)

    def _agg(self):
        self._agg_id = 0
        g = self.win.grid
        book = self.win.book
        for lab in self.agg_labels.values():
            lab.hide()
        if book is None or g.sel.is_single_cell():
            return False
        a = book.aggregates(g.sheet, g.sel.ranges)
        if a["count"] < 2:
            return False
        r, c = g.sel.active
        fmt = book.cell_style(g.sheet, r, c).get("numfmt", "General")
        on = self.enabled()

        def num(v):
            return book.format_number(v, fmt) if v is not None else ""
        vals = {"average": num(a["average"]) if a["num_count"] else None, "count": str(a["count"]),
                "num_count": str(a["num_count"]), "min": num(a["min"]) if a["num_count"] else None,
                "max": num(a["max"]) if a["num_count"] else None,
                "sum": num(a["sum"]) if a["num_count"] else None}
        for key, name in AGG_ITEMS:
            if key in on and vals[key] is not None:
                self.agg_labels[key].set_text("%s: %s" % (name, vals[key]))
                self.agg_labels[key].show()
        return False

    def on_menu(self, w, ev):
        if ev.button != 3:
            return False
        menu = Gtk.Menu()
        menu.get_style_context().add_class("folio-menu")
        head = Gtk.MenuItem(label="Customize Status Bar")
        head.set_sensitive(False)
        menu.append(head)
        menu.append(Gtk.SeparatorMenuItem())
        on = list(self.enabled())
        for key, name in AGG_ITEMS:
            it = Gtk.CheckMenuItem(label=name)
            it.set_active(key in on)

            def toggled(item, key=key):
                cur = list(self.enabled())
                if item.get_active() and key not in cur:
                    cur.append(key)
                elif not item.get_active() and key in cur:
                    cur.remove(key)
                self.win.app.settings["status_agg"] = cur
                self.win.app.save_settings()
                self.update_aggregates()
            it.connect("toggled", toggled)
            menu.append(it)
        menu.show_all()
        menu.popup_at_pointer(ev)
        return True
