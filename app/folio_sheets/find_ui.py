"""Find and Replace (Ctrl+F / Ctrl+H) and Go To Special."""
from gi.repository import Gtk

from . import APP_NAME
from .core import MAX_COLS, MAX_ROWS, a1


class FindReplace(Gtk.Dialog):
    """The Find and Replace dialog; stays open while the user works in the sheet."""

    def __init__(self, win, replace=False):
        super().__init__(title="Find and Replace", transient_for=win, modal=False)
        self.win = win
        self.set_default_size(460, -1)
        self.connect("delete-event", lambda *_: self.hide() or True)
        box = self.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        tabs = Gtk.Box(spacing=0)
        tabs.get_style_context().add_class("linked")
        self.tab_find = Gtk.RadioButton.new_with_label_from_widget(None, "Find")
        self.tab_replace = Gtk.RadioButton.new_with_label_from_widget(self.tab_find, "Replace")
        for t in (self.tab_find, self.tab_replace):
            t.set_mode(False)
            tabs.pack_start(t, False, False, 0)
        box.pack_start(tabs, False, False, 0)
        self._switching = False
        self.tab_replace.connect("toggled", lambda b: None if self._switching else self.set_mode(b.get_active()))
        # one form shared by both tabs; Replace shows the second row
        self.form = Gtk.Grid(row_spacing=6, column_spacing=8)
        self.form.set_border_width(8)
        self.what = Gtk.Entry(hexpand=True)
        self.what.set_activates_default(True)
        self.with_ = Gtk.Entry(hexpand=True)
        self.with_label = Gtk.Label(label="Replace with:", xalign=0)
        self.form.attach(Gtk.Label(label="Find what:", xalign=0), 0, 0, 1, 1)
        self.form.attach(self.what, 1, 0, 1, 1)
        self.form.attach(self.with_label, 0, 1, 1, 1)
        self.form.attach(self.with_, 1, 1, 1, 1)
        self.opts = Gtk.Grid(row_spacing=4, column_spacing=8)
        self.within = Gtk.ComboBoxText()
        for t in ("Sheet", "Workbook"):
            self.within.append_text(t)
        self.within.set_active(0)
        self.search = Gtk.ComboBoxText()
        for t in ("By Rows", "By Columns"):
            self.search.append_text(t)
        self.search.set_active(0)
        self.look = Gtk.ComboBoxText()
        for t in ("Formulas", "Values"):
            self.look.append_text(t)
        self.look.set_active(0)
        self.case = Gtk.CheckButton(label="Match case")
        self.whole = Gtk.CheckButton(label="Match entire cell contents")
        for i, (lab, w) in enumerate((("Within:", self.within), ("Search:", self.search), ("Look in:", self.look))):
            self.opts.attach(Gtk.Label(label=lab, xalign=0), 0, i, 1, 1)
            self.opts.attach(w, 1, i, 1, 1)
        self.opts.attach(self.case, 2, 0, 1, 1)
        self.opts.attach(self.whole, 2, 1, 1, 1)
        self.opts_btn = Gtk.Button(label="Options >>")
        self.opts_btn.connect("clicked", lambda *_: self.toggle_options())
        self.form.attach(self.opts, 0, 2, 2, 1)
        self.form.attach(self.opts_btn, 1, 3, 1, 1)
        self.opts_btn.set_halign(Gtk.Align.END)
        box.pack_start(self.form, False, False, 0)
        # buttons
        bb = Gtk.Box(spacing=6)
        self.b_replace_all = Gtk.Button(label="Replace All")
        self.b_replace = Gtk.Button(label="Replace")
        self.b_find_all = Gtk.Button(label="Find All")
        self.b_find_next = Gtk.Button(label="Find Next")
        self.b_close = Gtk.Button(label="Close")
        for b in (self.b_replace_all, self.b_replace, self.b_find_all, self.b_find_next):
            bb.pack_start(b, False, False, 0)
        bb.pack_end(self.b_close, False, False, 0)
        box.pack_start(bb, False, False, 0)
        self.b_find_next.set_can_default(True)
        self.b_find_next.grab_default()
        self.b_find_next.connect("clicked", lambda *_: self.find_next())
        self.b_find_all.connect("clicked", lambda *_: self.find_all())
        self.b_replace.connect("clicked", lambda *_: self.replace_one())
        self.b_replace_all.connect("clicked", lambda *_: self.replace_all())
        self.b_close.connect("clicked", lambda *_: self.hide())
        # Find All results
        self.results = Gtk.ListStore(str, str, str, str, int, int, int)
        tv = Gtk.TreeView(model=self.results)
        for i, t in enumerate(("Sheet", "Cell", "Value", "Formula")):
            col = Gtk.TreeViewColumn(t, Gtk.CellRendererText(), text=i)
            col.set_resizable(True)
            tv.append_column(col)
        tv.get_selection().connect("changed", self._result_picked)
        self.res_sw = Gtk.ScrolledWindow()
        self.res_sw.set_size_request(-1, 150)
        self.res_sw.add(tv)
        box.pack_start(self.res_sw, True, True, 0)
        self.status = Gtk.Label(label="", xalign=0)
        box.pack_start(self.status, False, False, 0)
        box.show_all()
        self.opts.set_no_show_all(True)
        self.opts.hide()
        self.res_sw.hide()
        self.set_mode(replace)

    # ---- state
    def set_mode(self, replace):
        self.replace_mode = replace
        for w in (self.with_label, self.with_, self.b_replace, self.b_replace_all):
            w.set_visible(replace)
        if replace:
            self.look.set_active(0)
        self.look.set_sensitive(not replace)
        self._switching = True
        (self.tab_replace if replace else self.tab_find).set_active(True)
        self._switching = False

    def toggle_options(self):
        on = not self.opts.get_visible()
        self.opts.set_visible(on)
        if on:
            self.opts.show_all()
        self.opts_btn.set_label("Options <<" if on else "Options >>")

    def options(self):
        return dict(case=self.case.get_active(), whole=self.whole.get_active(),
                    formulas=self.look.get_active() == 0 or self.replace_mode, by_cols=self.search.get_active() == 1)

    def sheets(self):
        g = self.win.grid
        if self.within.get_active() == 1:
            n = self.win.book.sheet_count
            order = list(range(g.sheet, n)) + list(range(0, g.sheet))
            return [s for s in order if self.win.book.sheet_info(s)["kind"] == "work"]
        return [g.sheet]

    def hits(self):
        pat = self.what.get_text()
        if not pat:
            return []
        o = self.options()
        hits = self.win.book.find_all(self.sheets(), pat, o["case"], o["whole"], o["formulas"], o["by_cols"])
        # inside a multi-cell selection, only the selection is searched
        g = self.win.grid
        rgs = [r for r in g.sel.ranges]
        if self.within.get_active() == 0 and (len(rgs) > 1 or rgs[0][0:2] != rgs[0][2:4]):
            hits = [h for h in hits if any(r[0] <= h[1] <= r[2] and r[1] <= h[2] <= r[3] for r in rgs)]
        return hits

    def _go(self, s, r, c):
        g = self.win.grid
        if s != g.sheet:
            g.set_sheet(s)
            self.win.tabs.rebuild()
        multi = len(g.sel.ranges) > 1 or g.sel.ranges[0][0:2] != g.sel.ranges[0][2:4]
        if multi and self.within.get_active() == 0:
            g.sel.active = (r, c)
        else:
            g.select_cell(r, c, refresh=False)
        g.ensure_visible(r, c)
        g.refresh()

    def _not_found(self):
        self.status.set_text("")
        self.win.dialogs.message("%s cannot find the data you're searching for." % APP_NAME, "Find and Replace")

    # ---- actions
    def find_next(self, back=False):
        hits = self.hits()
        if not hits:
            self._not_found()
            return None
        g = self.win.grid
        cur = (g.sheet, g.sel.active[0], g.sel.active[1])
        by_cols = self.options()["by_cols"]
        order = self.sheets()

        def key(h):
            pos = (h[2], h[1]) if by_cols else (h[1], h[2])
            return (order.index(h[0]) if h[0] in order else 0,) + pos
        ck = key(cur)
        after = [h for h in hits if (key(h) < ck if back else key(h) > ck)]
        h = (after[-1] if back else after[0]) if after else (hits[-1] if back else hits[0])
        self._go(*h)
        self.status.set_text("")
        return h

    def find_all(self):
        hits = self.hits()
        self.results.clear()
        b = self.win.book
        for s, r, c in hits:
            name = b.sheet_info(s)["name"]
            f = b.input(s, r, c) if b.is_formula(s, r, c) else ""
            self.results.append([name, "$" + _abs(r, c), b.display(s, r, c), f, s, r, c])
        self.res_sw.show()
        self.status.set_text("%d cell(s) found" % len(hits))
        if not hits:
            self._not_found()
        return hits

    def _result_picked(self, sel):
        m, it = sel.get_selected()
        if it is not None:
            self._go(m[it][4], m[it][5], m[it][6])

    def replace_one(self):
        """Replace in the active cell if it matches, then find the next match."""
        g = self.win.grid
        b = self.win.book
        r, c = g.sel.active
        o = self.options()
        t = b.replaced_text(g.sheet, r, c, self.what.get_text(), self.with_.get_text(), o["case"], o["whole"]) \
            if self.what.get_text() else None
        if t is not None:
            try:
                b.set_input(g.sheet, r, c, t)
            except ValueError as e:
                self.win.dialogs.message(str(e))
                return
            g.refresh()
        self.find_next()

    def replace_all(self):
        pat = self.what.get_text()
        if not pat:
            return 0
        o = self.options()
        g = self.win.grid
        rgs = list(g.sel.ranges)
        b = self.win.book
        if self.within.get_active() == 0 and (len(rgs) > 1 or rgs[0][0:2] != rgs[0][2:4]):
            # only inside the selection: one cell at a time, one undo step
            n = 0
            with b.group("Replace"):
                for s, r, c in self.hits():
                    t = b.replaced_text(s, r, c, pat, self.with_.get_text(), o["case"], o["whole"])
                    if t is not None:
                        try:
                            b.set_input(s, r, c, t)
                            n += 1
                        except ValueError:
                            pass
        else:
            n = b.replace_all(self.sheets(), pat, self.with_.get_text(), o["case"], o["whole"], o["by_cols"])
        g.refresh()
        self.win.update_title()
        if n:
            self.win.dialogs.info("All done. We made %d replacements." % n, "Find and Replace")
        else:
            self._not_found()
        return n


def _abs(r, c):
    t = a1(r, c)
    i = next(k for k, ch in enumerate(t) if ch.isdigit())
    return t[:i] + "$" + t[i:]


def open_find(win, replace=False):
    d = getattr(win, "find_dialog", None)
    if d is None:
        d = FindReplace(win, replace)
        win.find_dialog = d
    d.set_mode(replace)
    d.present()
    d.what.grab_focus()
    return d


# ------------------------------------------------------------------ Go To Special
SPECIAL = [("Notes", "notes"), ("Blanks", "blanks"), ("Constants", "constants"), ("Formulas", "formulas"), ("Current region", "region"),
           ("Current array", "array"), ("Last cell", "lastcell"), ("Visible cells only", "visible"),
           ("Conditional formats", "cf"), ("Data validation", "validation")]


def goto_special(win, kind=None, mask=None):
    """Select the cells of a kind (inside a multi-cell selection, or the whole sheet)."""
    g = win.grid
    b = win.book
    if kind is None:
        d = Gtk.Dialog(title="Go To Special", transient_for=win, modal=True)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(12)
        box.set_spacing(4)
        box.pack_start(Gtk.Label(label="Select", xalign=0), False, False, 0)
        group = None
        radios = []
        for label, k in SPECIAL:
            rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
            group = group or rb
            radios.append((rb, k))
            box.pack_start(rb, False, False, 0)
        subs = []
        sb = Gtk.Box(spacing=8)
        sb.set_margin_start(24)
        for label, bit in (("Numbers", 1), ("Text", 2), ("Logicals", 4), ("Errors", 16)):
            cb = Gtk.CheckButton(label=label)
            cb.set_active(True)
            subs.append((cb, bit))
            sb.pack_start(cb, False, False, 0)
        box.pack_start(sb, False, False, 0)
        d.show_all()
        r = d.run()
        kind = next(k for rb, k in radios if rb.get_active())
        mask = sum(bit for cb, bit in subs if cb.get_active())
        d.destroy()
        if r != Gtk.ResponseType.OK:
            return None
    mask = 23 if mask is None else mask
    rg = g.sel.ranges[-1]
    ar, ac = g.sel.active
    if kind == "region":
        out = [tuple(b.current_region(g.sheet, ar, ac))]
    elif kind == "array":
        a = b.array_range(g.sheet, ar, ac) or b.spill_range(g.sheet, ar, ac)
        if a is None:
            win.dialogs.message("No cells were found.", "Go To Special")
            return []
        out = [tuple(a)]
    else:
        single = len(g.sel.ranges) == 1 and rg[0:2] == rg[2:4]
        area = (0, 0, MAX_ROWS - 1, MAX_COLS - 1) if single else rg
        out = [tuple(x) for x in b.special_cells(g.sheet, area, kind, mask)]
    if not out:
        win.dialogs.message("No cells were found.", "Go To Special")
        return []
    if len(out) > 2000:
        out = out[:2000]
    g.sel.ranges = out
    g.sel.active = out[0][0:2]
    g.ensure_visible(*out[0][0:2])
    g.refresh()
    return out
