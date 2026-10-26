"""The Draw tab (SH-26): ink drawn over the sheet with a pen, pencil or highlighter; the eraser
takes whole strokes away; Lasso Select picks ink up to move or delete. The ink is kept in the
file as Office ink (InkML), so it shows in Excel too.

While a drawing tool is on, the mouse draws instead of selecting cells; Select (or Esc) turns it
off again. Ruler, Ink to Shape, Ink to Math and Ink Replay are not available."""
from gi.repository import Gdk, GLib, Gtk

from . import APP_NAME

# the pens of a new workbook, as in the desktop spreadsheet: a black pen, a pencil, a yellow highlighter
DEFAULTS = {
    "pen": {"color": 0x000000, "mm": 0.5},
    "pencil": {"color": 0x404040, "mm": 1.0},
    "highlighter": {"color": 0xFFFF00, "mm": 6.0},
}
KIND = {"pen": 0, "pencil": 1, "highlighter": 2}
WIDTHS = {"pen": [0.25, 0.35, 0.5, 0.7, 1.0, 2.0, 3.5], "pencil": [0.5, 1.0, 1.5, 2.0, 3.0],
          "highlighter": [2.0, 4.0, 6.0, 8.0, 10.0]}
COLORS = [("Black", 0x000000), ("White", 0xFFFFFF), ("Red", 0xE71224), ("Orange", 0xF7630C), ("Yellow", 0xFFFF00),
          ("Green", 0x00A651), ("Turquoise", 0x00B7C3), ("Blue", 0x0078D4), ("Dark Blue", 0x00188F),
          ("Purple", 0x8E1AA2), ("Pink", 0xE3008C), ("Brown", 0x825E3F), ("Gray", 0x808080),
          ("Light Green", 0x66FF00), ("Light Blue", 0x00FFFF), ("Lavender", 0xB4A0FF)]
TOOLS = ("pen", "pencil", "highlighter", "eraser", "lasso")
NOT_HERE = "%s isn't available in %s."


def mm_to_px(mm):
    return mm / 25.4 * 96.0


class InkTools:
    def __init__(self, win):
        self.win = win
        self.tool = None
        self.pens = {k: dict(v) for k, v in DEFAULTS.items()}
        self.stroke = None          # screen points of the stroke being drawn
        self.lasso = None
        self.erased = []

    # ---------------------------------------------------------------- tools
    def set_tool(self, tool):
        if tool not in TOOLS:
            tool = None
        if tool == self.tool and tool is not None:
            return
        self.tool = tool
        self.stroke = self.lasso = None
        r = self.win.ribbon
        for t in TOOLS + ("select",):
            r.update("ink:" + t, "true" if (t == tool or (t == "select" and tool is None)) else "false")
        area = self.win.grid.area
        win = area.get_window()
        if win is not None:
            name = {"eraser": "cell", "lasso": "crosshair"}.get(tool, "crosshair") if tool else None
            win.set_cursor(Gdk.Cursor.new_from_name(area.get_display(), name) if name else None)
        if tool:
            what = {"eraser": "Drag over ink to erase it.", "lasso": "Draw round the ink to select it."}.get(
                tool, "Drag on the sheet to draw.")
            self.win.status.extra.set_text("%s Select or Esc stops drawing." % what)
        else:
            self.win.status.extra.set_text("")
        self.win.grid.area.queue_draw()

    def active(self):
        return self.tool is not None

    def width_px(self, tool):
        return mm_to_px(self.pens[tool]["mm"])

    # ---------------------------------------------------------------- input from the grid
    def press(self, ev):
        if not self.tool or ev.button != 1:
            return False
        g = self.win.grid
        g.area.grab_focus()
        if g.sel_objects and self.tool != "lasso":
            g.deselect_objects()
            self.win.on_object_selection()
        if self.tool in KIND:
            self.stroke = [(ev.x, ev.y)]
        elif self.tool == "eraser":
            self.erased = []
            self._erase_at(ev.x, ev.y)
        elif self.tool == "lasso":
            self.lasso = [(ev.x, ev.y)]
        g.area.queue_draw()
        return True

    def motion(self, ev):
        if not self.tool:
            return False
        if self.stroke is not None:
            lx, ly = self.stroke[-1]
            if abs(ev.x - lx) + abs(ev.y - ly) >= 1.0:
                self.stroke.append((ev.x, ev.y))
                self.win.grid.area.queue_draw()
        elif self.lasso is not None:
            self.lasso.append((ev.x, ev.y))
            self.win.grid.area.queue_draw()
        elif self.tool == "eraser" and ev.get_state() & Gdk.ModifierType.BUTTON1_MASK:
            self._erase_at(ev.x, ev.y)
        return True

    def release(self, ev):
        if not self.tool:
            return False
        g = self.win.grid
        b = self.win.book
        p = g.params()
        if self.stroke is not None:
            pts = [b.sheet_point(g.sheet, x, y, p) for x, y in self.stroke]
            self.stroke = None
            pen = self.pens[self.tool]
            try:
                b.add_ink(g.sheet, [(pts, pen["color"], self.width_px(self.tool), KIND[self.tool])])
            except Exception as e:
                self.win.status.extra.set_text(str(e))
            self.win.update_title()
        elif self.lasso is not None:
            poly = self.lasso
            self.lasso = None
            self.select_in(poly)
        self.erased = []
        g.refresh()
        return True

    def _erase_at(self, x, y):
        g = self.win.grid
        b = self.win.book
        p = g.params()
        sx, sy = b.sheet_point(g.sheet, x, y, p)
        hits = b.ink_at(g.sheet, sx, sy, 6.0 / max(p.get("zoom", 1.0), 0.1))
        if hits:
            b.delete_objects(g.sheet, hits)
            self.erased += hits
            g.refresh()
            self.win.update_title()

    def select_in(self, poly):
        """Lasso: the ink with most of its points inside the shape drawn."""
        if len(poly) < 3:
            return []
        g = self.win.grid
        b = self.win.book
        p = g.params()
        spoly = [b.sheet_point(g.sheet, x, y, p) for x, y in poly]
        picked = []
        for i, o in enumerate(b.objects(g.sheet)):
            if o.get("kind") != "ink":
                continue
            strokes = b.ink_strokes(g.sheet, i) or []
            pts = [pt for st in strokes for pt in st[0]]
            if pts and sum(1 for pt in pts if inside(pt, spoly)) >= 0.6 * len(pts):
                picked.append(i)
        g.deselect_objects()
        for k, i in enumerate(picked):
            g.select_object(i, add=k > 0)
        self.win.on_object_selection()
        return picked

    def draw(self, cr):
        """The stroke or lasso being drawn, over the sheet."""
        if self.stroke:
            pen = self.pens[self.tool]
            c = pen["color"]
            z = self.win.grid.params().get("zoom", 1.0)
            a = {"pencil": 0.85}.get(self.tool, 1.0)
            if self.tool == "highlighter":
                import cairo
                cr.set_operator(cairo.OPERATOR_MULTIPLY)
            cr.set_source_rgba(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255, a)
            cr.set_line_width(max(0.5, self.width_px(self.tool) * z))
            cr.set_line_cap(2 if self.tool == "highlighter" else 1)
            cr.set_line_join(1)
            x, y = self.stroke[0]
            cr.move_to(x, y)
            for x, y in self.stroke[1:]:
                cr.line_to(x, y)
            if len(self.stroke) == 1:
                cr.rel_line_to(0.01, 0)
            cr.stroke()
            import cairo
            cr.set_operator(cairo.OPERATOR_OVER)
        if self.lasso and len(self.lasso) > 1:
            cr.set_source_rgb(0.25, 0.25, 0.25)
            cr.set_line_width(1)
            cr.set_dash([4, 3])
            x, y = self.lasso[0]
            cr.move_to(x, y)
            for x, y in self.lasso[1:]:
                cr.line_to(x, y)
            cr.stroke()
            cr.set_dash([])

    # ---------------------------------------------------------------- pen settings
    def set_width(self, tool, mm):
        self.pens[tool]["mm"] = float(mm)
        self.set_tool(tool)

    def set_color(self, tool, color):
        self.pens[tool]["color"] = int(color)
        self.set_tool(tool)

    def menu_items(self, tool):
        """The pen's drop-down: thickness, then colours."""
        items = [("#", "Thickness")]
        for mm in WIDTHS[tool]:
            items.append(("%g mm" % mm, "ink-width:%s:%g" % (tool, mm)))
        items.append(None)
        items.append(("#", "Colors"))
        for name, c in COLORS:
            items.append((name, "ink-color:%s:%06X" % (tool, c)))
        return items


def inside(pt, poly):
    x, y = pt
    n = len(poly)
    c = False
    j = n - 1
    for i in range(n):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / ((yj - yi) or 1e-12) + xi:
            c = not c
        j = i
    return c


def not_available(win, what):
    win.dialogs.info(NOT_HERE % (what, APP_NAME), what)
