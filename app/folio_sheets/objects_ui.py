"""Objects on the sheet: the Selection Pane, Select Objects, Align, Group and Rotate, Alt Text."""
from gi.repository import Gdk, Gtk

ALIGN_MENU = [("Align Left", "align-objects:left"), ("Align Center", "align-objects:center"),
              ("Align Right", "align-objects:right"), None, ("Align Top", "align-objects:top"),
              ("Align Middle", "align-objects:middle"), ("Align Bottom", "align-objects:bottom"), None,
              ("Distribute Horizontally", "align-objects:dist-h"), ("Distribute Vertically", "align-objects:dist-v"), None,
              ("Snap to Grid", "snap-grid"), ("Snap to Shape", "snap-shape"), ("View Gridlines", "view-gridlines")]
GROUP_MENU = [("Group", "group-objects:group"), ("Regroup", "group-objects:regroup"), ("Ungroup", "group-objects:ungroup")]
ROTATE_MENU = [("Rotate Right 90°", "rotate-objects:right"), ("Rotate Left 90°", "rotate-objects:left"),
               ("Flip Vertical", "rotate-objects:flip-v"), ("Flip Horizontal", "rotate-objects:flip-h"), None,
               ("More Rotation Options...", "rotate-objects:more")]

NOTHING = "Select an object first: a picture, shape, chart or text box."


def _sel(win):
    g = win.grid
    n = len(win.book.objects(g.sheet))
    return [i for i in g.sel_objects if i < n]


def _after(win, keep=None):
    g = win.grid
    if keep is not None:
        g.sel_objects = list(keep)
    g.refresh()
    win.update_title()
    win.on_object_selection()
    pane = getattr(win, "selection_pane", None)
    if pane is not None and pane.get_visible():
        pane.load()


# ------------------------------------------------------------------ Align
def align(win, how):
    g, b = win.grid, win.book
    sel = _sel(win)
    if not sel:
        win.status.extra.set_text(NOTHING)
        return False
    rects = {i: tuple(b.objects(g.sheet)[i]["rect"]) for i in sel}
    if len(sel) == 1 and how not in ("dist-h", "dist-v"):
        # one object lines up with the sheet's edge (top or left), as the original does with the page
        x, y, w, h = rects[sel[0]]
        nx = {"left": 0.0}.get(how, x)
        ny = {"top": 0.0}.get(how, y)
        if (nx, ny) != (x, y):
            b.set_object_rect(g.sheet, sel[0], nx, ny, w, h, "Align")
        _after(win)
        return True
    xs1 = min(r[0] for r in rects.values())
    ys1 = min(r[1] for r in rects.values())
    xs2 = max(r[0] + r[2] for r in rects.values())
    ys2 = max(r[1] + r[3] for r in rects.values())
    new = {}
    if how in ("dist-h", "dist-v"):
        if len(sel) < 3:
            win.status.extra.set_text("Select three or more objects to distribute them.")
            return False
        k = 0 if how == "dist-h" else 1
        order = sorted(sel, key=lambda i: rects[i][k])
        total = sum(rects[i][k + 2] for i in order)
        span = (xs2 - xs1) if k == 0 else (ys2 - ys1)
        gap = (span - total) / (len(order) - 1)
        pos = xs1 if k == 0 else ys1
        for i in order:
            x, y, w, h = rects[i]
            new[i] = (pos, y, w, h) if k == 0 else (x, pos, w, h)
            pos += (w if k == 0 else h) + gap
    else:
        for i in sel:
            x, y, w, h = rects[i]
            if how == "left":
                x = xs1
            elif how == "center":
                x = (xs1 + xs2) / 2 - w / 2
            elif how == "right":
                x = xs2 - w
            elif how == "top":
                y = ys1
            elif how == "middle":
                y = (ys1 + ys2) / 2 - h / 2
            elif how == "bottom":
                y = ys2 - h
            new[i] = (x, y, w, h)
    b.begin_group("Align" if not how.startswith("dist") else "Distribute")
    try:
        for i, (x, y, w, h) in new.items():
            if (x, y, w, h) != rects[i]:
                b.set_object_rect(g.sheet, i, float(x), float(y), float(w), float(h), "Align")
    finally:
        b.end_group()
    _after(win)
    return True


def snap(win, rect):
    """A rectangle being moved, put on cell edges (Snap to Grid) or other objects' edges
    (Snap to Shape) when they are near."""
    x, y, w, h = rect
    b, g = win.book, win.grid
    if getattr(win, "snap_grid", False):
        p = g.params()
        hit = b.hit(g.sheet, float(x + 1), float(y + 1), p)
        if hit[0] == "cell":
            cr = b.cell_rect(g.sheet, hit[1], hit[2], p)
            if cr:
                if abs(cr[0] - x) < 8:
                    x = cr[0]
                elif abs(cr[0] + cr[2] - x) < 8:
                    x = cr[0] + cr[2]
                if abs(cr[1] - y) < 8:
                    y = cr[1]
                elif abs(cr[1] + cr[3] - y) < 8:
                    y = cr[1] + cr[3]
    if getattr(win, "snap_shape", False):
        p = g.params()
        sel = set(g.sel_objects)
        for i in range(len(b.objects(g.sheet))):
            if i in sel:
                continue
            r = b.object_screen_rect(g.sheet, i, p)
            if not r:
                continue
            for ex in (r[0], r[0] + r[2]):
                if abs(ex - x) < 8:
                    x = ex
                elif abs(ex - (x + w)) < 8:
                    x = ex - w
            for ey in (r[1], r[1] + r[3]):
                if abs(ey - y) < 8:
                    y = ey
                elif abs(ey - (y + h)) < 8:
                    y = ey - h
    return (x, y, w, h)


# ------------------------------------------------------------------ Group
def group(win, how="group"):
    g, b = win.grid, win.book
    sel = _sel(win)
    if how == "regroup":
        names = getattr(win, "last_ungrouped", None)
        if not names or names[0] != g.sheet:
            win.status.extra.set_text("There is nothing to regroup.")
            return False
        objs = b.objects(g.sheet)
        sel = [i for i, o in enumerate(objs) if o["name"] in names[1]]
        how = "group"
    if how == "group":
        if len(sel) < 2:
            win.status.extra.set_text("Select two or more objects to group them.")
            return False
        try:
            i = b.group_objects(g.sheet, sel)
        except ValueError as e:
            win.dialogs.message(str(e), "Group")
            return False
        _after(win, [i])
        return True
    groups = [i for i in sel if b.objects(g.sheet)[i]["members"]]
    if not groups:
        win.status.extra.set_text("Select a group to ungroup it.")
        return False
    out = []
    b.begin_group("Ungroup")
    try:
        for i in sorted(groups, reverse=True):
            got = b.ungroup_object(g.sheet, i)
            out = [k + (len(got) - 1 if k > i else 0) for k in out] + list(got)
    finally:
        b.end_group()
    objs = b.objects(g.sheet)
    win.last_ungrouped = (g.sheet, [objs[k]["name"] for k in out if k < len(objs)])
    _after(win, out)
    return True


# ------------------------------------------------------------------ Rotate
def rotate(win, how):
    g, b = win.grid, win.book
    sel = [i for i in _sel(win) if b.objects(g.sheet)[i]["kind"] not in ("chart", "slicer", "timeline")]
    if not sel:
        win.status.extra.set_text("Select a picture or shape to rotate it.")
        return False
    if how == "more":
        return rotation_dialog(win, sel)
    props = {"right": {"rot_by": 90.0}, "left": {"rot_by": -90.0}, "flip-v": {"flip_v": True},
             "flip-h": {"flip_h": True}}[how]
    b.set_object_props(g.sheet, sel, props, {"right": "Rotate Right", "left": "Rotate Left", "flip-v": "Flip Vertical",
                                             "flip-h": "Flip Horizontal"}[how])
    _after(win)
    return True


def rotation_dialog(win, sel):
    g, b = win.grid, win.book
    p = b.object_props(g.sheet, sel[0])
    d = Gtk.Dialog(title="Size and Properties", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    row = Gtk.Box(spacing=8)
    row.pack_start(Gtk.Label(label="Rotation:"), False, False, 0)
    spin = Gtk.SpinButton.new_with_range(-3600, 3600, 1)
    spin.set_value(round(p["rot"]))
    row.pack_start(spin, False, False, 0)
    row.pack_start(Gtk.Label(label="°"), False, False, 0)
    box.pack_start(row, False, False, 0)
    d.spin = spin
    d.show_all()
    r = d.run()
    v = spin.get_value()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return False
    b.set_object_props(g.sheet, sel, {"rot": float(v)}, "Rotate")
    _after(win)
    return True


# ------------------------------------------------------------------ Selection Pane
class SelectionPane(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.win = win
        self.set_size_request(260, -1)
        self.get_style_context().add_class("folio-pane")
        self.set_name("selection-pane")
        head = Gtk.Box(spacing=6)
        head.set_border_width(6)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<span size='large'>Selection</span>")
        lab.get_style_context().add_class("folio-pane-title")
        head.pack_start(lab, True, True, 0)
        close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda _b: self.hide())
        head.pack_end(close, False, False, 0)
        self.pack_start(head, False, False, 0)
        tools = Gtk.Box(spacing=4)
        tools.set_margin_start(6)
        for label, fn in (("Show All", lambda *_: self.show_all_objects(True)),
                          ("Hide All", lambda *_: self.show_all_objects(False))):
            bt = Gtk.Button(label=label)
            bt.connect("clicked", fn)
            tools.pack_start(bt, False, False, 0)
        for icon, tip, by in (("go-up-symbolic", "Bring Forward", 1), ("go-down-symbolic", "Send Backward", -1)):
            bt = Gtk.Button.new_from_icon_name(icon, Gtk.IconSize.MENU)
            bt.set_tooltip_text(tip)
            bt.connect("clicked", lambda _b, by=by: self.move(by))
            tools.pack_end(bt, False, False, 0)
        self.pack_start(tools, False, False, 0)
        # index, name, hidden
        self.store = Gtk.ListStore(int, str, bool)
        self.tree = Gtk.TreeView(model=self.store)
        self.tree.set_headers_visible(False)
        self.tree.get_selection().set_mode(Gtk.SelectionMode.MULTIPLE)
        name = Gtk.CellRendererText()
        name.set_property("editable", True)
        name.connect("edited", self._renamed)
        col = Gtk.TreeViewColumn("", name, text=1)
        col.set_expand(True)
        self.tree.append_column(col)
        eye = Gtk.CellRendererToggle()
        eye.connect("toggled", self._toggled)
        ecol = Gtk.TreeViewColumn("", eye)
        ecol.set_cell_data_func(eye, lambda c, cell, m, it, _d: cell.set_property("active", not m[it][2]))
        self.tree.append_column(ecol)
        self._loading = False
        self.tree.get_selection().connect("changed", self._picked)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_vexpand(True)
        sw.add(self.tree)
        self.pack_start(sw, True, True, 0)

    def load(self):
        b, g = self.win.book, self.win.grid
        self._loading = True
        self.store.clear()
        objs = b.objects(g.sheet)
        for i in reversed(range(len(objs))):       # the front object first, as in the original
            o = objs[i]
            self.store.append([i, o["name"] or o["kind"].capitalize(), bool(o.get("hidden"))])
        sel = self.tree.get_selection()
        sel.unselect_all()
        for row in self.store:
            if row[0] in g.sel_objects:
                sel.select_iter(row.iter)
        self._loading = False

    def _picked(self, sel):
        if self._loading:
            return
        m, paths = sel.get_selected_rows()
        g = self.win.grid
        g.sel_objects = [m[p][0] for p in paths if not m[p][2]]
        g.refresh()
        self.win.on_object_selection()

    def _renamed(self, cell, path, text):
        i = self.store[path][0]
        g = self.win.grid
        try:
            self.win.book.set_object_props(g.sheet, [i], {"name": text}, "Rename Object")
        except ValueError as e:
            self.win.dialogs.message(str(e))
        _after(self.win)

    def _toggled(self, cell, path):
        i, hidden = self.store[path][0], self.store[path][2]
        self.set_hidden([i], not hidden)

    def set_hidden(self, which, hide):
        g = self.win.grid
        self.win.book.set_object_props(g.sheet, list(which), {"hidden": bool(hide)}, "Hide Object" if hide else "Show Object")
        if hide:
            g.sel_objects = [k for k in g.sel_objects if k not in which]
        _after(self.win)

    def show_all_objects(self, show):
        n = len(self.win.book.objects(self.win.grid.sheet))
        if n:
            self.set_hidden(range(n), not show)

    def move(self, by):
        self.win.run("bring-forward" if by > 0 else "send-backward")
        self.load()


def selection_pane(win):
    pane = getattr(win, "selection_pane", None)
    if pane is None:
        pane = win.selection_pane = SelectionPane(win)
        win.mid.pack_start(pane, False, False, 0)
    if pane.get_visible() and not getattr(win, "_pane_force", False):
        pane.hide()
        return pane
    pane.show_all()
    pane.load()
    return pane


# ------------------------------------------------------------------ Select Objects
def select_objects_mode(win, on=None):
    """Home > Find & Select > Select Objects: dragging on the sheet selects the objects inside
    the rectangle; Esc (or choosing it again) goes back to cells."""
    on = (not getattr(win, "selecting_objects", False)) if on is None else on
    win.selecting_objects = on
    win.ribbon.update("select-objects", "true" if on else "false")
    w = win.grid.area.get_window()
    if w is not None:
        w.set_cursor(Gdk.Cursor.new_from_name(win.grid.area.get_display(), "default") if on else None)
    win.status.extra.set_text("Drag a rectangle round the objects to select them. Esc to stop." if on else "")
    return on


def objects_in(win, rect):
    """Objects wholly inside a screen rectangle (x, y, w, h)."""
    g, b = win.grid, win.book
    x, y, w, h = rect
    p = g.params()
    out = []
    for i, o in enumerate(b.objects(g.sheet)):
        if o.get("hidden"):
            continue
        r = b.object_screen_rect(g.sheet, i, p)
        if r and r[0] >= x and r[1] >= y and r[0] + r[2] <= x + w and r[1] + r[3] <= y + h:
            out.append(i)
    return out


def alt_text(win):
    sel = _sel(win)
    if not sel:
        win.status.extra.set_text(NOTHING)
        return False
    from .review_ui import alt_text as dialog
    r = dialog(win, win.grid.sheet, sel[0])
    _after(win)
    return r

