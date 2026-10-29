"""Form controls and cell check boxes (S3-07).

Developer > Insert > Form Controls draws a button, check box, option button, combo box, list
box, spin button, scroll bar, group box or label over the cells. Each can be linked to a cell
(Format Control > Control): a check box puts TRUE / FALSE there, option buttons the number of
the one chosen, a combo or list box the number of the item picked, a spinner or scroll bar its
value. Clicking a control uses it; Ctrl+click or Design Mode selects it to move, size, format or
delete it. ActiveX controls and macros are not available: a button keeps the macro it names but
doesn't run it.

Insert > Checkbox puts check boxes in the selected cells: the cells hold TRUE / FALSE, a click
or Space ticks them.
"""
from gi.repository import Gdk, GLib, Gtk

from . import APP_NAME

KINDS = [
    # (kind, label, default size in px)
    ("Button", "Button", (72, 26)),
    ("Drop", "Combo Box", (96, 20)),
    ("Checkbox", "Check Box", (96, 18)),
    ("Spin", "Spin Button", (18, 32)),
    ("List", "List Box", (96, 80)),
    ("Radio", "Option Button", (104, 18)),
    ("GBox", "Group Box", (144, 96)),
    ("Label", "Label", (64, 18)),
    ("Scroll", "Scroll Bar", (18, 96)),
]
LABEL = {k: l for k, l, _s in KINDS}
SIZE = {k: s for k, _l, s in KINDS}
TEXTED = ("Button", "Checkbox", "Radio", "GBox", "Label")

NO_MACROS = ("%s doesn't run macros: the button keeps the macro it's assigned (%%s) for the desktop "
             "spreadsheet, but clicking it here does nothing.") % APP_NAME
NO_ACTIVEX = "ActiveX controls aren't available in %s. Use the Form Controls, which work the same way with linked cells." % APP_NAME


class Controls:
    """Drawing, selecting and using the form controls of the shown sheet."""

    def __init__(self, win):
        self.win = win
        self.placing = None      # the kind being drawn
        self.start = self.cur = None
        self.sel = None          # the selected control
        self.design = False      # Design Mode: clicks select instead of using
        self._drag = None

    # ------------------------------------------------------------ helpers
    def book(self):
        return self.win.book

    def sheet(self):
        return self.win.grid.sheet

    def list(self):
        b = self.book()
        return b.controls(self.sheet()) if b is not None else []

    def rect(self, i):
        g = self.win.grid
        return self.book().control_screen_rect(g.sheet, i, g.params())

    def at(self, x, y):
        """The topmost control under a screen point (group boxes only at their frame)."""
        b = self.book()
        if b is None:
            return None
        lst = self.list()
        for i in range(len(lst) - 1, -1, -1):
            r = self.rect(i)
            if r is None:
                continue
            rx, ry, rw, rh = r
            if rx - 2 <= x <= rx + rw + 2 and ry - 2 <= y <= ry + rh + 2:
                if lst[i]["kind"] == "GBox" and rx + 6 < x < rx + rw - 6 and ry + 14 < y < ry + rh - 6 and self.sel != i:
                    continue
                return i
        return None

    def stale(self):
        """The selection belongs to another sheet or book state: dropped."""
        if self.sel is not None and (getattr(self, "sel_sheet", None) != self.sheet() or self.book() is None
                                     or self.sel >= len(self.list())):
            self.sel = None
        return self.sel is None

    def handle_at(self, x, y):
        if self.stale():
            return None
        r = self.rect(self.sel)
        if r is None:
            return None
        for name, (hx, hy) in _handles(r).items():
            if abs(x - hx) <= 4 and abs(y - hy) <= 4:
                return name
        return None

    def refresh(self):
        self.win.grid.refresh()
        self.win.update_title()

    def save(self, lst, label):
        self.book().set_controls(self.sheet(), lst, label)
        self.refresh()

    # ------------------------------------------------------------ drawing a new one
    def place(self, kind):
        shp = getattr(self.win, "shapes", None)
        if shp is not None and shp.placing:
            shp.cancel()
        self.placing = kind
        self.start = self.cur = None
        area = self.win.grid.area
        if area.get_window() is not None:
            area.get_window().set_cursor(Gdk.Cursor.new_from_name(area.get_display(), "crosshair"))
        self.win.status.extra.set_text("Drag on the sheet to draw the %s. Esc cancels." % LABEL[kind].lower())

    def cancel(self):
        self.placing = self.start = self.cur = None
        area = self.win.grid.area
        if area.get_window() is not None:
            area.get_window().set_cursor(None)
        self.win.status.extra.set_text("")
        area.queue_draw()

    def add(self, kind, x, y, w, h, **fields):
        """A new control at sheet pixels; its index."""
        b = self.book()
        s = self.sheet()
        lst = self.list()
        d = {"kind": kind, "rect": (x, y, w, h), "name": b.control_name(s, kind)}
        if kind in TEXTED:
            d["text"] = d["name"]
        if kind == "Radio" and not any(c["kind"] == "Radio" for c in lst):
            d["first_button"] = True
        d.update(fields)
        lst.append(d)
        self.save(lst, "Insert " + LABEL[kind])
        return len(lst) - 1

    # ------------------------------------------------------------ mouse
    def press(self, ev):
        """From the grid; True when the control took the press."""
        if self.book() is None:
            return False
        if self.placing:
            if ev.button != 1:
                return False
            self.start = self.cur = (ev.x, ev.y)
            return True
        h = self.handle_at(ev.x, ev.y)
        if h is not None and ev.button == 1:
            self._drag = ("size", self.sel, h, ev.x, ev.y, self.rect(self.sel))
            return True
        i = self.at(ev.x, ev.y)
        if i is None:
            if self.sel is not None:
                self.select(None)
            return False
        g = self.win.grid
        if g.editing() and g.commit_edit() is False:
            return True
        ctrl = bool(ev.get_state() & Gdk.ModifierType.CONTROL_MASK)
        if ev.button == 3:
            self.select(i)
            self.menu(ev, i)
            return True
        if ev.button != 1:
            return True
        if ctrl or self.design or self.sel == i:
            if ev.type == Gdk.EventType._2BUTTON_PRESS:
                self.select(i)
                self.format(i)
                return True
            self.select(i)
            self._drag = ("move", i, None, ev.x, ev.y, self.rect(i))
            return True
        if self.sel is not None:
            self.select(None)
        self.use(i, ev)
        return True

    def motion(self, ev):
        if self.placing:
            if self.start is not None:
                self.cur = (ev.x, ev.y)
                self.win.grid.area.queue_draw()
            return True
        if self._drag is None:
            area = self.win.grid.area
            h = self.handle_at(ev.x, ev.y)
            if h is not None and area.get_window() is not None:
                area.get_window().set_cursor(Gdk.Cursor.new_from_name(area.get_display(), _CURSORS.get(h, "default")))
                return True
            return False
        self.cur = (ev.x, ev.y)
        self.win.grid.area.queue_draw()
        return True

    def release(self, ev):
        b = self.book()
        g = self.win.grid
        p = g.params()
        if self.placing:
            if self.start is None:
                return False
            kind = self.placing
            (x0, y0), (x1, y1) = self.start, self.cur or self.start
            self.cancel()
            sx0, sy0 = b.sheet_point(g.sheet, x0, y0, p)
            sx1, sy1 = b.sheet_point(g.sheet, x1, y1, p)
            if abs(sx1 - sx0) < 3 and abs(sy1 - sy0) < 3:
                w, h = SIZE[kind]
                sx1, sy1 = sx0 + w, sy0 + h
            x, y, w, h = min(sx0, sx1), min(sy0, sy1), max(abs(sx1 - sx0), 6), max(abs(sy1 - sy0), 6)
            i = self.add(kind, x, y, w, h)
            self.select(i)
            if kind == "Button":
                GLib.idle_add(lambda: (self.assign_macro(i), False)[1])
            return True
        if self._drag is None:
            return False
        what, i, handle, ox, oy, r0 = self._drag
        self._drag = None
        cur = self.cur or (ox, oy)
        self.cur = None
        dx, dy = cur[0] - ox, cur[1] - oy
        if r0 is None or (abs(dx) < 2 and abs(dy) < 2):
            g.area.queue_draw()
            return True
        nx, ny, nw, nh = _moved(r0, what, handle, dx, dy)
        sx0, sy0 = b.sheet_point(g.sheet, nx, ny, p)
        sx1, sy1 = b.sheet_point(g.sheet, nx + nw, ny + nh, p)
        lst = self.list()
        lst[i]["rect"] = (sx0, sy0, max(sx1 - sx0, 4), max(sy1 - sy0, 4))
        self.save(lst, "Move Control" if what == "move" else "Size Control")
        return True

    def draw(self, cr):
        if self.placing and self.start and self.cur:
            (x0, y0), (x1, y1) = self.start, self.cur
            cr.set_source_rgb(0.3, 0.3, 0.3)
            cr.set_line_width(1)
            cr.set_dash([4, 3])
            cr.rectangle(min(x0, x1) + 0.5, min(y0, y1) + 0.5, abs(x1 - x0), abs(y1 - y0))
            cr.stroke()
            cr.set_dash([])
        if self.stale():
            return
        r = self.rect(self.sel)
        if r is None:
            return
        if self._drag is not None and self.cur is not None:
            what, _i, handle, ox, oy, r0 = self._drag
            r = _moved(r0, what, handle, self.cur[0] - ox, self.cur[1] - oy)
        x, y, w, h = r
        # the dotted frame and the round handles, as for a selected control
        cr.set_source_rgb(0.35, 0.35, 0.35)
        cr.set_line_width(1)
        cr.set_dash([2, 2])
        cr.rectangle(int(x) - 1.5, int(y) - 1.5, int(w) + 3, int(h) + 3)
        cr.stroke()
        cr.set_dash([])
        for hx, hy in _handles((x, y, w, h)).values():
            cr.arc(hx, hy, 3.5, 0, 6.2832)
            cr.set_source_rgb(1, 1, 1)
            cr.fill_preserve()
            cr.set_source_rgb(0.35, 0.35, 0.35)
            cr.stroke()

    def select(self, i):
        self.sel = i
        self.sel_sheet = self.sheet()
        g = self.win.grid
        if i is not None and g.sel_objects:
            g.sel_objects = []
            self.win.on_object_selection()
        if i is not None:
            c = self.list()[i]
            self.win.status.extra.set_text("%s selected. Drag to move it, Delete removes it, double-click to format it." % c["name"])
        else:
            self.win.status.extra.set_text("")
        g.area.queue_draw()

    def key(self, ev):
        """Keys while a control is selected (Delete, Esc, arrows nudge)."""
        if self.placing and ev.keyval == Gdk.KEY_Escape:
            self.cancel()
            return True
        if self.stale():
            return False
        k = ev.keyval
        if k in (Gdk.KEY_Delete, Gdk.KEY_BackSpace):
            self.delete(self.sel)
            return True
        if k == Gdk.KEY_Escape:
            self.select(None)
            return True
        steps = {Gdk.KEY_Left: (-1, 0), Gdk.KEY_Right: (1, 0), Gdk.KEY_Up: (0, -1), Gdk.KEY_Down: (0, 1)}
        if k in steps:
            dx, dy = steps[k]
            lst = self.list()
            x, y, w, h = lst[self.sel]["rect"]
            lst[self.sel]["rect"] = (max(0, x + dx), max(0, y + dy), w, h)
            self.save(lst, "Move Control")
            return True
        return False

    # ------------------------------------------------------------ using a control
    def use(self, i, ev):
        b = self.book()
        s = self.sheet()
        c = self.list()[i]
        k = c["kind"]
        v = b.control_value(s, i)
        x, y, w, h = self.rect(i)
        if k == "Checkbox":
            self.set_value(i, 0 if v == 1 else 1)
        elif k == "Radio":
            self.set_value(i, 1)
        elif k == "Spin":
            horiz = c["horiz"] or w > h * 1.5
            up = (ev.x > x + w / 2) if horiz else (ev.y < y + h / 2)
            self.set_value(i, v + (c["inc"] if up else -c["inc"]))
        elif k == "Scroll":
            horiz = c["horiz"] or w > h
            pos, length, thick = ((ev.x - x), w, h) if horiz else ((ev.y - y), h, w)
            arrow = min(thick, length / 3)
            lo, hi = min(c["min"], c["max"]), max(c["min"], c["max"])
            span = max(hi - lo, 1)
            track = length - 2 * arrow
            thumb_at = arrow + (track - max(track * 0.15, 8)) * (v - lo) / span
            if pos < arrow:
                nv = v - c["inc"]
            elif pos > length - arrow:
                nv = v + c["inc"]
            elif pos < thumb_at:
                nv = v - c["page"]
            elif pos > thumb_at + max(track * 0.15, 8):
                nv = v + c["page"]
            else:
                nv = v
            self.set_value(i, nv)
        elif k == "Drop":
            self.drop_menu(i, ev)
        elif k == "List":
            items = b.control_items(s, i)
            z = self.win.grid.params()["zoom"]
            lh = max(1.0, 15.0 * z)
            rows = max(1, int(h // lh))
            first = v - rows if v > rows else 0
            row = first + int((ev.y - y) // lh)
            if 0 <= row < len(items):
                self.set_value(i, row + 1)
        elif k == "Button":
            if c["macro"]:
                self.win.dialogs.info(NO_MACROS % c["macro"], c["name"])

    def set_value(self, i, v):
        try:
            self.book().set_control_value(self.sheet(), i, int(v))
        except ValueError as e:
            self.win.dialogs.message(str(e))
        self.refresh()

    def drop_menu(self, i, ev):
        items = self.book().control_items(self.sheet(), i)
        c = self.list()[i]
        menu = Gtk.Menu()
        for k, t in enumerate(items[:max(1, 1000)]):
            mi = Gtk.CheckMenuItem(label=t or " ")
            mi.set_draw_as_radio(True)
            mi.set_active(self.book().control_value(self.sheet(), i) == k + 1)
            mi.connect("activate", lambda _m, k=k: self.set_value(i, k + 1))
            menu.append(mi)
        if not items:
            mi = Gtk.MenuItem(label="(no items: set the input range in Format Control)")
            mi.set_sensitive(False)
            menu.append(mi)
        menu.show_all()
        x, y, w, h = self.rect(i)
        rect = Gdk.Rectangle()
        rect.x, rect.y, rect.width, rect.height = int(x), int(y), int(w), int(h)
        menu.set_size_request(int(w), -1)
        _ = c
        menu.popup_at_rect(self.win.grid.area.get_window(), rect, Gdk.Gravity.SOUTH_WEST, Gdk.Gravity.NORTH_WEST, ev)

    # ------------------------------------------------------------ editing
    def menu(self, ev, i):
        c = self.list()[i]
        m = Gtk.Menu()

        def item(label, fn, ok=True):
            mi = Gtk.MenuItem(label=label)
            mi.set_sensitive(ok)
            mi.connect("activate", lambda *_: fn())
            m.append(mi)
        item("Cut", lambda: (self.copy(i), self.delete(i)))
        item("Copy", lambda: self.copy(i))
        item("Paste", self.paste, ok=bool(getattr(self.win, "_control_clip", None)))
        m.append(Gtk.SeparatorMenuItem())
        item("Edit Text", lambda: self.edit_text(i), ok=c["kind"] in TEXTED)
        item("Assign Macro...", lambda: self.assign_macro(i), ok=c["kind"] == "Button")
        item("Format Control...", lambda: self.format(i))
        item("Delete", lambda: self.delete(i))
        m.show_all()
        m.popup_at_pointer(ev)

    def delete(self, i):
        lst = self.list()
        name = lst[i]["name"]
        del lst[i]
        self.sel = None
        self.save(lst, "Delete " + name)
        self.win.status.extra.set_text("")

    def copy(self, i):
        self.win._control_clip = dict(self.list()[i])

    def paste(self):
        d = dict(getattr(self.win, "_control_clip", None) or {})
        if not d:
            return
        x, y, w, h = d.pop("rect")
        d.pop("name", None)
        kind = d.pop("kind")
        i = self.add(kind, x + 12, y + 12, w, h, **{k: v for k, v in d.items() if k != "text" or kind in TEXTED})
        self.select(i)

    def edit_text(self, i):
        c = self.list()[i]
        t = self.win.dialogs.ask_text("Edit Text", "Text:", c["text"])
        if t is None:
            return
        lst = self.list()
        lst[i]["text"] = t
        self.save(lst, "Edit Text")

    def assign_macro(self, i):
        """Assign Macro: the macros of this workbook (kept, not run)."""
        c = self.list()[i]
        try:
            names = [shown for shown, _m, _s in self.book().macros()]
        except (AttributeError, ValueError):
            names = []
        dlg = Gtk.Dialog(title="Assign Macro", transient_for=self.win, modal=True)
        dlg.add_button("Cancel", Gtk.ResponseType.CANCEL)
        dlg.add_button("OK", Gtk.ResponseType.OK)
        box = dlg.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        box.pack_start(Gtk.Label(label="Macro name:", xalign=0), False, False, 0)
        entry = Gtk.Entry()
        entry.set_text(c["macro"] or "")
        box.pack_start(entry, False, False, 0)
        store = Gtk.ListStore(str)
        for n in names:
            store.append([n])
        tv = Gtk.TreeView(model=store, headers_visible=False)
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        tv.get_selection().connect("changed", lambda sel: entry.set_text(sel.get_selected()[0][sel.get_selected()[1]][0]) if sel.get_selected()[1] else None)
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(300, 140)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        note = Gtk.Label(label="%s keeps the macro a button is assigned but doesn't run it." % APP_NAME, xalign=0)
        note.set_line_wrap(True)
        note.get_style_context().add_class("dim-label")
        box.pack_start(note, False, False, 0)
        dlg.show_all()
        r = dlg.run()
        name = entry.get_text().strip()
        dlg.destroy()
        if r != Gtk.ResponseType.OK:
            return
        lst = self.list()
        lst[i]["macro"] = name
        self.save(lst, "Assign Macro")

    def format(self, i):
        dlg = FormatControl(self.win, self, i)
        dlg.run_and_apply()


def _handles(r):
    x, y, w, h = r
    return {"nw": (x, y), "n": (x + w / 2, y), "ne": (x + w, y), "e": (x + w, y + h / 2),
            "se": (x + w, y + h), "s": (x + w / 2, y + h), "sw": (x, y + h), "w": (x, y + h / 2)}


_CURSORS = {"nw": "nw-resize", "n": "n-resize", "ne": "ne-resize", "e": "e-resize", "se": "se-resize",
            "s": "s-resize", "sw": "sw-resize", "w": "w-resize"}


def _moved(r0, what, handle, dx, dy):
    x, y, w, h = r0
    if what == "move":
        return (x + dx, y + dy, w, h)
    if "w" in handle:
        x, w = x + dx, w - dx
    if "e" in handle:
        w = w + dx
    if "n" in handle:
        y, h = y + dy, h - dy
    if "s" in handle:
        h = h + dy
    if w < 4:
        w = 4
    if h < 4:
        h = 4
    return (x, y, w, h)


class FormatControl(Gtk.Dialog):
    """Format Control > Control: the linked cell, input range, value and limits."""

    def __init__(self, win, ctl, i):
        super().__init__(title="Format Control", transient_for=win, modal=True)
        self.win, self.ctl, self.i = win, ctl, i
        self.c = c = ctl.list()[i]
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("OK", Gtk.ResponseType.OK)
        nb = Gtk.Notebook()
        self.get_content_area().pack_start(nb, True, True, 0)
        grid = Gtk.Grid(column_spacing=8, row_spacing=6, border_width=12)
        self.fields = {}
        row = 0

        def entry(key, label, value):
            nonlocal row
            grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
            e = Gtk.Entry()
            e.set_text(str(value))
            e.set_hexpand(True)
            grid.attach(e, 1, row, 1, 1)
            self.fields[key] = e
            row += 1
            return e
        k = c["kind"]
        if k in ("Checkbox", "Radio"):
            grid.attach(Gtk.Label(label="Value", xalign=0), 0, row, 2, 1)
            row += 1
            val = win.book.control_value(win.grid.sheet, i)
            group = None
            self.value_buttons = []
            names = ["Unchecked", "Checked"] + (["Mixed"] if k == "Checkbox" else [])
            for n, label in enumerate(names):
                b = Gtk.RadioButton.new_with_label_from_widget(group, label)
                group = group or b
                b.set_active(val == n)
                grid.attach(b, 0, row, 2, 1)
                self.value_buttons.append(b)
                row += 1
        if k in ("Drop", "List"):
            entry("range", "Input range:", c["range"])
        if k in ("Spin", "Scroll"):
            entry("val", "Current value:", win.book.control_value(win.grid.sheet, i))
            entry("min", "Minimum value:", c["min"])
            entry("max", "Maximum value:", c["max"])
            entry("inc", "Incremental change:", c["inc"])
            if k == "Scroll":
                entry("page", "Page change:", c["page"])
        if k not in ("Button", "GBox", "Label"):
            entry("link", "Cell link:", c["link"])
        if k == "Drop":
            entry("drop_lines", "Drop down lines:", c["drop_lines"])
        if k in TEXTED:
            entry("text", "Text:", c["text"])
        if k == "Button":
            entry("macro", "Macro (kept, not run):", c["macro"])
        if row == 0:
            grid.attach(Gtk.Label(label="This control has no settings to change.", xalign=0), 0, 0, 2, 1)
        nb.append_page(grid, Gtk.Label(label="Control"))
        # Size and Properties
        g2 = Gtk.Grid(column_spacing=8, row_spacing=6, border_width=12)
        x, y, w, h = c["rect"]
        self.w_spin = Gtk.SpinButton.new_with_range(4, 4000, 1)
        self.w_spin.set_value(round(w))
        self.h_spin = Gtk.SpinButton.new_with_range(4, 4000, 1)
        self.h_spin.set_value(round(h))
        g2.attach(Gtk.Label(label="Height (px):", xalign=0), 0, 0, 1, 1)
        g2.attach(self.h_spin, 1, 0, 1, 1)
        g2.attach(Gtk.Label(label="Width (px):", xalign=0), 0, 1, 1, 1)
        g2.attach(self.w_spin, 1, 1, 1, 1)
        self.name_e = Gtk.Entry()
        self.name_e.set_text(c["name"])
        g2.attach(Gtk.Label(label="Name:", xalign=0), 0, 2, 1, 1)
        g2.attach(self.name_e, 1, 2, 1, 1)
        nb.append_page(g2, Gtk.Label(label="Size"))
        self.show_all()

    def run_and_apply(self):
        while True:
            r = self.run()
            if r != Gtk.ResponseType.OK:
                self.destroy()
                return False
            err = self.apply()
            if err is None:
                self.destroy()
                return True
            self.win.dialogs.message(err, "Format Control")

    def apply(self):
        b = self.win.book
        s = self.win.grid.sheet
        lst = self.ctl.list()
        d = lst[self.i]
        f = {k: e.get_text().strip() for k, e in self.fields.items()}
        for key in ("link", "range"):
            if key in f:
                t = f[key].lstrip("=")
                if t and b.control_ref_ok(s, t) is False:
                    return "The reference '%s' isn't valid." % t
                d[key] = t
        for key in ("min", "max", "inc", "page", "val", "drop_lines"):
            if key in f:
                try:
                    d[key] = int(float(f[key]))
                except ValueError:
                    return "Enter a whole number for %s." % key.replace("_", " ")
        if "min" in d and "max" in d and d["kind"] in ("Spin", "Scroll"):
            if not 0 <= d["min"] <= 30000 or not 0 <= d["max"] <= 30000:
                return "Enter a value between 0 and 30000."
        for key in ("text", "macro"):
            if key in f:
                d[key] = f[key]
        if self.name_e.get_text().strip():
            d["name"] = self.name_e.get_text().strip()
        x, y, _w, _h = d["rect"]
        d["rect"] = (x, y, self.w_spin.get_value(), self.h_spin.get_value())
        value = None
        if getattr(self, "value_buttons", None):
            value = [k for k, bt in enumerate(self.value_buttons) if bt.get_active()][0]
        elif d["kind"] in ("Spin", "Scroll") and "val" in d:
            value = d["val"]
        with b.group("Format Control"):
            b.set_controls(s, lst, "Format Control")
            if value is not None and (d["kind"] != "Radio" or value == 1):
                b.set_control_value(s, self.i, value)
        self.ctl.refresh()
        return None


# ------------------------------------------------------------ cell check boxes
def insert_cell_checkboxes(win):
    g = win.grid
    b = win.book
    ranges = [tuple(rg) for rg in g.sel.ranges]
    b.cell_checkboxes(g.sheet, ranges, True)
    g.refresh()
    win.update_title()


def clear_cell_checkboxes(win, ranges=None):
    g = win.grid
    ranges = ranges or [tuple(rg) for rg in g.sel.ranges]
    win.book.cell_checkboxes(g.sheet, ranges, False)
    g.refresh()
    win.update_title()


def toggle_selected(win):
    """Space on check-box cells: all of them flip (all ticked when any was clear)."""
    g = win.grid
    b = win.book
    s = g.sheet
    cells = []
    for r1, c1, r2, c2 in g.sel.ranges:
        if (r2 - r1 + 1) * (c2 - c1 + 1) > 20000:
            continue
        for r in range(r1, r2 + 1):
            for c in range(c1, c2 + 1):
                if b.is_checkbox(s, r, c):
                    cells.append((r, c))
    if not cells:
        return False
    target = "FALSE" if all(b.display(s, r, c) == "TRUE" for r, c in cells) else "TRUE"
    with b.group("Checkbox"):
        for r, c in cells:
            try:
                b.set_input(s, r, c, target)
            except ValueError as e:
                win.dialogs.message(str(e))
                break
    g.refresh()
    win.update_title()
    return True


def click_cell(win, r, c, ev, rect):
    """A click on a check-box cell: ticks it when the click is on the box."""
    b = win.book
    s = win.grid.sheet
    if not b.is_checkbox(s, r, c):
        return False
    v = b.display(s, r, c)
    if v not in ("TRUE", "FALSE", ""):
        return False
    x, y, w, h = rect
    # the box: centred unless the cell says otherwise; a click anywhere near it counts
    k = min(h - 2, 16)
    if not (x <= ev.x <= x + w and y <= ev.y <= y + h):
        return False
    if abs(ev.x - (x + w / 2)) > max(k, 10) and win.book.cell_style(s, r, c).get("halign", "") in ("", "center"):
        return False
    try:
        b.toggle_checkbox(s, r, c)
    except ValueError as e:
        win.dialogs.message(str(e))
    win.grid.refresh()
    win.update_title()
    return True
