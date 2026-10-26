"""Sort, Filter and Remove Duplicates: the commands, the Sort dialog, the AutoFilter
menu (a popover on the column's button) and the Custom AutoFilter dialog."""
from gi.repository import Gdk, Gtk

from .core import MAX_COLS, MAX_ROWS, col_name

OPS = [("equals", "equal"), ("does not equal", "notEqual"), ("is greater than", "greaterThan"),
       ("is greater than or equal to", "greaterThanOrEqual"), ("is less than", "lessThan"),
       ("is less than or equal to", "lessThanOrEqual"), ("begins with", "begins"), ("ends with", "ends"),
       ("contains", "contains"), ("does not contain", "notContains")]


def _data_range(win):
    """The range a data command works on: the selection, or the region around the cell."""
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    if rg[0] == rg[2] and rg[1] == rg[3]:
        rg = tuple(win.book.current_region(g.sheet, rg[0], rg[1]))
    elif rg[0] == 0 and rg[2] >= MAX_ROWS - 1 or rg[1] == 0 and rg[3] >= MAX_COLS - 1:
        ext = win.book.used_extent(g.sheet) or (0, 0)
        rg = (rg[0], rg[1], min(rg[2], ext[0]), min(rg[3], ext[1]))
    return rg


def _run(win, label, fn):
    g = win.grid
    if g.editing() and g.commit_edit() is False:
        return False
    try:
        fn()
    except ValueError as e:
        win.dialogs.message(str(e))
        return False
    g.refresh()
    win.update_title()
    return True


def _header_names(win, rg, header):
    b, s = win.book, win.grid.sheet
    out = []
    for c in range(rg[1], rg[3] + 1):
        t = b.display(s, rg[0], c) if header else ""
        out.append(t or "Column " + col_name(c))
    return out


# ------------------------------------------------------------------ sort
def sort_quick(win, ascending):
    g = win.grid
    rg = _data_range(win)
    if not win.book.has_content(g.sheet, rg[0], rg[1]) and rg[0] == rg[2]:
        win.dialogs.message("This can't be applied to the selected range. Select a single cell in a range and try again.")
        return
    header = win.book.guess_header(g.sheet, rg)
    col = g.sel.active[1]
    if not rg[1] <= col <= rg[3]:
        col = rg[1]
    if _run(win, "Sort", lambda: win.book.sort_range(g.sheet, rg, [(col, ascending)], header)):
        g.sel.ranges = [rg]
        g.refresh()


class SortDialog(Gtk.Dialog):
    """Sort levels: column and order, with My data has headers."""

    def __init__(self, win, rg):
        super().__init__(title="Sort", transient_for=win, modal=True)
        self.win, self.rg = win, rg
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        box = self.get_content_area()
        box.set_border_width(12)
        box.set_spacing(8)
        bar = Gtk.Box(spacing=6)
        self.add_btn = Gtk.Button(label="Add Level")
        self.del_btn = Gtk.Button(label="Delete Level")
        self.header = Gtk.CheckButton(label="My data has headers")
        self.header.set_active(win.book.guess_header(win.grid.sheet, rg))
        bar.pack_start(self.add_btn, False, False, 0)
        bar.pack_start(self.del_btn, False, False, 0)
        bar.pack_end(self.header, False, False, 0)
        box.pack_start(bar, False, False, 0)
        self.grid = Gtk.Grid(row_spacing=6, column_spacing=10)
        for i, t in enumerate(("", "Column", "Order")):
            self.grid.attach(Gtk.Label(label=t, xalign=0), i, 0, 1, 1)
        box.pack_start(self.grid, False, False, 0)
        self.levels = []
        self.add_btn.connect("clicked", lambda *_: self.add_level())
        self.del_btn.connect("clicked", lambda *_: self.del_level())
        self.header.connect("toggled", lambda *_: self.refresh_names())
        self.add_level(win.grid.sel.active[1])
        self.show_all()

    def names(self):
        return _header_names(self.win, self.rg, self.header.get_active())

    def add_level(self, col=None):
        if len(self.levels) >= 64:
            return
        n = len(self.levels)
        lab = Gtk.Label(label="Sort by" if n == 0 else "Then by", xalign=0)
        cb = Gtk.ComboBoxText()
        for t in self.names():
            cb.append_text(t)
        c = col if col is not None and self.rg[1] <= col <= self.rg[3] else self.rg[1]
        cb.set_active(c - self.rg[1])
        order = Gtk.ComboBoxText()
        for t in ("A to Z / Smallest to Largest", "Z to A / Largest to Smallest"):
            order.append_text(t)
        order.set_active(0)
        self.grid.attach(lab, 0, n + 1, 1, 1)
        self.grid.attach(cb, 1, n + 1, 1, 1)
        self.grid.attach(order, 2, n + 1, 1, 1)
        self.levels.append((lab, cb, order))
        self.grid.show_all()

    def del_level(self):
        if len(self.levels) <= 1:
            return
        for w in self.levels.pop():
            w.destroy()

    def refresh_names(self):
        names = self.names()
        for _, cb, _ in self.levels:
            i = cb.get_active()
            cb.remove_all()
            for t in names:
                cb.append_text(t)
            cb.set_active(i)

    def keys(self):
        return [(self.rg[1] + cb.get_active(), order.get_active() == 0) for _, cb, order in self.levels]


def sort_custom(win):
    g = win.grid
    rg = _data_range(win)
    d = SortDialog(win, rg)
    r = d.run()
    keys, header = d.keys(), d.header.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return
    if _run(win, "Sort", lambda: win.book.sort_range(g.sheet, rg, keys, header)):
        g.sel.ranges = [rg]
        g.refresh()


# ------------------------------------------------------------------ remove duplicates
def remove_duplicates(win):
    g = win.grid
    rg = _data_range(win)
    d = Gtk.Dialog(title="Remove Duplicates", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="To delete duplicate values, select one or more columns that contain duplicates.",
                             xalign=0, wrap=True), False, False, 0)
    header = Gtk.CheckButton(label="My data has headers")
    header.set_active(win.book.guess_header(g.sheet, rg))
    box.pack_start(header, False, False, 0)
    checks = []
    lb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    sw = Gtk.ScrolledWindow()
    sw.set_size_request(300, 160)
    sw.add(lb)
    box.pack_start(sw, True, True, 0)

    def fill():
        for ch in lb.get_children():
            ch.destroy()
        checks.clear()
        for t in _header_names(win, rg, header.get_active()):
            cb = Gtk.CheckButton(label=t)
            cb.set_active(True)
            lb.pack_start(cb, False, False, 0)
            checks.append(cb)
        lb.show_all()
    header.connect("toggled", lambda *_: fill())
    fill()
    d.show_all()
    r = d.run()
    cols = [rg[1] + i for i, cb in enumerate(checks) if cb.get_active()]
    has_header = header.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK or not cols:
        return
    res = []
    if _run(win, "Remove Duplicates", lambda: res.append(win.book.remove_duplicates(g.sheet, rg, cols, has_header))):
        removed, left = res[0]
        if removed:
            win.dialogs.info("%d duplicate values found and removed; %d unique values remain." % (removed, left),
                             "Remove Duplicates")
        else:
            win.dialogs.info("No duplicate values found.", "Remove Duplicates")


# ------------------------------------------------------------------ AutoFilter
def toggle_filter(win):
    g = win.grid
    s = g.sheet
    if win.book.autofilter(s):
        _run(win, "Filter", lambda: win.book.set_autofilter(s, None))
        return
    rg = _data_range(win)
    if rg[0] == rg[2] and not win.book.has_content(s, rg[0], rg[1]):
        win.dialogs.message("This can't be applied to the selected range. Select a single cell in a range and try again.")
        return
    _run(win, "Filter", lambda: win.book.set_autofilter(s, rg))


def clear_filter(win):
    if win.book.autofilter(win.grid.sheet):
        _run(win, "Clear Filter", lambda: win.book.clear_filters(win.grid.sheet))


def reapply_filter(win):
    if win.book.autofilter(win.grid.sheet):
        _run(win, "Reapply", lambda: win.book.reapply_filter(win.grid.sheet))


class FilterMenu(Gtk.Popover):
    """The menu of a column's filter button."""

    def __init__(self, win, col, row=None):
        super().__init__()
        self.win, self.col = win, col
        b, s = win.book, win.grid.sheet
        self.at = (row, col) if row is not None else None
        af = b.autofilter(s, self.at)
        self.ref = af["ref"]
        self.off = col - self.ref[1]
        cur = af["columns"].get(self.off)
        name = b.display(s, self.ref[0], col) or "Column " + col_name(col)
        self.name = name
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        box.set_border_width(8)
        self.add(box)

        def item(label, fn, sensitive=True):
            bt = Gtk.ModelButton(label=label)
            bt.get_child().set_xalign(0) if bt.get_child() else None
            bt.set_sensitive(sensitive)
            bt.connect("clicked", lambda *_: fn())
            box.pack_start(bt, False, False, 0)
            return bt
        self.sort_asc = item("Sort A to Z", lambda: self.sort(True))
        self.sort_desc = item("Sort Z to A", lambda: self.sort(False))
        box.pack_start(Gtk.Separator(), False, False, 4)
        self.clear_btn = item('Clear Filter From "%s"' % name, self.clear, cur is not None)
        self.custom_btn = item("Text and Number Filters...", self.custom)
        self.search = Gtk.SearchEntry()
        self.search.set_placeholder_text("Search")
        box.pack_start(self.search, False, False, 4)
        values, blank = b.filter_values(s, self.off, self.at)
        self.store = Gtk.ListStore(bool, str, str)   # shown, label, value ("" = blanks, None-like "\0" = all)
        shown_vals = None
        show_blank = True
        if cur and "values" in cur:
            shown_vals = {v.lower() for v in cur["values"]}
            show_blank = cur.get("blank", False)
        self.store.append([True, "(Select All)", "\0"])
        for v in values:
            self.store.append([shown_vals is None or v.lower() in shown_vals, v, v])
        if blank:
            self.store.append([show_blank, "(Blanks)", ""])
        self._sync_all()
        self.filtered = self.store.filter_new()
        self.filtered.set_visible_func(self._visible)
        tv = Gtk.TreeView(model=self.filtered, headers_visible=False)
        r = Gtk.CellRendererToggle()
        r.connect("toggled", self._toggled)
        tv.append_column(Gtk.TreeViewColumn("", r, active=0))
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=1))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(240, 200)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        self.search.connect("search-changed", lambda *_: self.filtered.refilter())
        bb = Gtk.Box(spacing=6)
        ok = Gtk.Button(label="OK")
        ok.get_style_context().add_class("suggested-action")
        cancel = Gtk.Button(label="Cancel")
        ok.connect("clicked", lambda *_: self.ok())
        cancel.connect("clicked", lambda *_: self.popdown())
        bb.pack_end(cancel, False, False, 0)
        bb.pack_end(ok, False, False, 0)
        box.pack_start(bb, False, False, 4)
        box.show_all()

    def _visible(self, model, it, data=None):
        q = self.search.get_text().strip().lower()
        return not q or model[it][2] == "\0" or q in model[it][1].lower()

    def _sync_all(self):
        rows = [r for r in self.store if r[2] != "\0"]
        self.store[0][0] = all(r[0] for r in rows)

    def _toggled(self, renderer, path):
        it = self.filtered.convert_iter_to_child_iter(self.filtered.get_iter(path))
        row = self.store[it]
        on = not row[0]
        if row[2] == "\0":
            for r in self.store:
                if self._visible(self.store, r.iter):
                    r[0] = on
        else:
            row[0] = on
        self._sync_all()

    def set_checked(self, values, blanks=None):
        """Tick exactly these values (tests and keyboard use)."""
        want = {v.lower() for v in values}
        for r in self.store:
            if r[2] == "":
                r[0] = bool(blanks)
            elif r[2] != "\0":
                r[0] = r[2].lower() in want
        self._sync_all()

    def ok(self):
        rows = [r for r in self.store if r[2] != "\0"]
        b, s = self.win.book, self.win.grid.sheet
        if all(r[0] for r in rows):
            spec = None
        else:
            spec = {"values": [r[2] for r in rows if r[0] and r[2] != ""],
                    "blank": any(r[0] for r in rows if r[2] == "")}
            if not spec["values"] and not spec["blank"]:
                self.win.dialogs.message("Select at least one value to show.")
                return
        self.popdown()
        from .protect_ui import protected, allows, refuse
        if protected(self.win) and not allows(self.win, "autoFilter"):
            refuse(self.win)
            return
        _run(self.win, "Filter", lambda: b.filter_column(s, self.off, spec, self.at))

    def clear(self):
        self.popdown()
        _run(self.win, "Clear Filter", lambda: self.win.book.filter_column(self.win.grid.sheet, self.off, None, self.at))

    def sort(self, asc):
        self.popdown()
        b, s = self.win.book, self.win.grid.sheet
        _run(self.win, "Sort", lambda: b.sort_range(s, self.ref, [(self.col, asc)], True))

    def custom(self):
        self.popdown()
        custom_filter(self.win, self.off, self.name, self.at)


def custom_filter(win, off, name, at=None):
    """Custom AutoFilter: two conditions joined by And/Or."""
    b, s = win.book, win.grid.sheet
    af = b.autofilter(s, at)
    cur = af["columns"].get(off) if af else None
    d = Gtk.Dialog(title="Custom AutoFilter", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="Show rows where: %s" % name, xalign=0), False, False, 0)
    rows = []
    for i in range(2):
        hb = Gtk.Box(spacing=6)
        op = Gtk.ComboBoxText()
        op.append_text("")
        for label, _ in OPS:
            op.append_text(label)
        op.set_active(1 if i == 0 else 0)
        val = Gtk.Entry()
        hb.pack_start(op, False, False, 0)
        hb.pack_start(val, True, True, 0)
        rows.append((op, val))
        box.pack_start(hb, False, False, 0)
        if i == 0:
            jb = Gtk.Box(spacing=12)
            and_b = Gtk.RadioButton.new_with_mnemonic_from_widget(None, "_And")
            or_b = Gtk.RadioButton.new_with_mnemonic_from_widget(and_b, "_Or")
            jb.pack_start(and_b, False, False, 0)
            jb.pack_start(or_b, False, False, 0)
            box.pack_start(jb, False, False, 0)
    if cur and "custom" in cur:
        for (op, val), (o, e) in zip(cur["custom"], rows):
            label, v = _from_engine(op, val)
            o.set_active([x[0] for x in OPS].index(label) + 1 if label in [x[0] for x in OPS] else 0)
            e.set_text(v)
        (and_b if cur.get("and", True) else or_b).set_active(True)
    box.pack_start(Gtk.Label(label="Use ? to represent any single character\nUse * to represent any series of characters",
                             xalign=0), False, False, 4)
    d.show_all()
    r = d.run()
    conds = []
    for o, e in rows:
        i = o.get_active()
        if i > 0 and (e.get_text() != "" or OPS[i - 1][1] in ("equal", "notEqual")):
            conds.append(_to_engine(OPS[i - 1][1], e.get_text()))
    is_and = and_b.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return
    spec = {"custom": conds, "and": is_and} if conds else None
    _run(win, "Filter", lambda: b.filter_column(s, off, spec, at))


def _to_engine(op, val):
    """begins/ends/contains are equal/notEqual with wildcards in the file format."""
    if op == "begins":
        return ("equal", val + "*")
    if op == "ends":
        return ("equal", "*" + val)
    if op == "contains":
        return ("equal", "*" + val + "*")
    if op == "notContains":
        return ("notEqual", "*" + val + "*")
    return (op, val)


def _from_engine(op, val):
    names = {k: lab for lab, k in OPS}
    if op in ("equal", "notEqual") and len(val) > 1:
        if val.startswith("*") and val.endswith("*") and len(val) > 2:
            return ("contains" if op == "equal" else "does not contain"), val[1:-1]
        if val.endswith("*") and op == "equal":
            return "begins with", val[:-1]
        if val.startswith("*") and op == "equal":
            return "ends with", val[1:]
    return names.get(op, "equals"), val


def filter_menu(win, col, rect, row=None):
    if not win.book.autofilter(win.grid.sheet, (row, col) if row is not None else None):
        return None
    m = FilterMenu(win, col, row)
    m.set_relative_to(win.grid.area)
    if rect:
        x, y, w, h = rect
        r = Gdk.Rectangle()
        r.x, r.y, r.width, r.height = int(x + w - 18), int(y), 18, int(h)
        m.set_pointing_to(r)
    m.set_position(Gtk.PositionType.BOTTOM)
    m.popup()
    win.filter_popover = m
    return m
