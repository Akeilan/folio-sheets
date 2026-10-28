"""Shapes, text boxes and WordArt (SH-21): the Shapes gallery, drawing a shape on the sheet,
typing in it, and the Shape Format tab (fill, outline, text, arrange, size)."""
import re

from gi.repository import Gdk, GLib, Gtk

from .core import Book

DEFAULT = 96.0        # a click without a drag: a one-inch shape


def pretty(prst):
    """Shape names as the gallery shows them: 'rightArrow' -> 'Arrow: Right'."""
    names = {
        "line": "Line", "straightConnector1": "Line Arrow", "arc": "Arc", "rect": "Rectangle",
        "roundRect": "Rectangle: Rounded Corners", "snip1Rect": "Rectangle: Single Corner Snipped", "textBox": "Text Box",
        "ellipse": "Oval", "triangle": "Isosceles Triangle", "rtTriangle": "Right Triangle", "rightArrow": "Arrow: Right",
        "leftArrow": "Arrow: Left", "upArrow": "Arrow: Up", "downArrow": "Arrow: Down", "leftRightArrow": "Arrow: Left-Right",
        "upDownArrow": "Arrow: Up-Down", "quadArrow": "Arrow: Quad", "notchedRightArrow": "Arrow: Notched Right",
        "homePlate": "Arrow: Pentagon", "chevron": "Arrow: Chevron", "mathPlus": "Plus Sign", "mathMinus": "Minus Sign",
        "mathMultiply": "Multiplication Sign", "mathDivide": "Division Sign", "mathEqual": "Equal", "mathNotEqual": "Not Equal",
        "wedgeRectCallout": "Speech Bubble: Rectangle", "wedgeRoundRectCallout": "Speech Bubble: Rectangle with Corners Rounded",
        "wedgeEllipseCallout": "Speech Bubble: Oval", "cloudCallout": "Thought Bubble: Cloud", "smileyFace": "Smiley Face",
        "noSmoking": "\"No\" Symbol", "foldedCorner": "Rectangle: Folded Corner", "lightningBolt": "Lightning Bolt",
        "bracketPair": "Double Bracket", "can": "Cylinder", "flowChartProcess": "Flowchart: Process",
        "flowChartAlternateProcess": "Flowchart: Alternate Process", "flowChartDecision": "Flowchart: Decision",
        "flowChartInputOutput": "Flowchart: Data", "flowChartPredefinedProcess": "Flowchart: Predefined Process",
        "flowChartDocument": "Flowchart: Document", "flowChartTerminator": "Flowchart: Terminator",
        "flowChartPreparation": "Flowchart: Preparation", "flowChartManualInput": "Flowchart: Manual Input",
        "flowChartManualOperation": "Flowchart: Manual Operation", "flowChartConnector": "Flowchart: Connector",
        "flowChartOffpageConnector": "Flowchart: Off-page Connector", "flowChartPunchedCard": "Flowchart: Card",
        "flowChartExtract": "Flowchart: Extract", "flowChartMerge": "Flowchart: Merge",
        "flowChartMagneticDisk": "Flowchart: Magnetic Disk",
    }
    if prst in names:
        return names[prst]
    m = re.match(r"star(\d+)", prst)
    if m:
        return "Star: %s Points" % m.group(1)
    return re.sub(r"(?<!^)([A-Z])", r" \1", prst).capitalize()


def _icon(prst, size=20):
    da = Gtk.DrawingArea()
    da.set_size_request(size, size)

    def draw(w, cr):
        from . import appearance
        ink = 0.85 if appearance.is_dark() else 0.25
        cr.set_source_rgb(ink, ink, ink)
        cr.set_line_width(1.0)
        if prst == "textBox":
            cr.rectangle(2.5, 4.5, size - 5, size - 9)
            cr.stroke()
            cr.move_to(5, size / 2 + 3)
            cr.set_font_size(8)
            cr.show_text("A")
            return
        Book.paint_shape_icon(cr, prst, 2, 2, size - 4, size - 4)
    da.connect("draw", draw)
    return da


def gallery(win, on_pick):
    """The Shapes gallery: every group's shapes as small pictures."""
    pop = Gtk.Popover()
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(380)
    sw.set_min_content_width(300)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    box.set_border_width(6)
    for group, names in Book.shape_gallery():
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>%s</b>" % GLib.markup_escape_text(group))
        box.pack_start(lab, False, False, 2)
        grid = Gtk.FlowBox()
        grid.set_selection_mode(Gtk.SelectionMode.NONE)
        grid.set_max_children_per_line(10)
        for p in names:
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.add(_icon(p))
            b.set_tooltip_text(pretty(p))
            b.set_name("shape-" + p)

            def picked(_b, p=p):
                pop.popdown()
                GLib.idle_add(lambda: (on_pick(p), False)[1])
            b.connect("clicked", picked)
            grid.add(b)
        box.pack_start(grid, False, False, 0)
    sw.add(box)
    pop.add(sw)
    sw.show_all()
    return pop


def wordart_gallery(win):
    pop = Gtk.Popover()
    grid = Gtk.Grid(column_spacing=4, row_spacing=4)
    grid.set_border_width(8)
    styles = Book.wordart_styles()
    for k, name in enumerate(styles):
        b = Gtk.Button()
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.set_tooltip_text(name)
        da = Gtk.DrawingArea()
        da.set_size_request(54, 48)

        def draw(w, cr, k=k):
            theme = [0x000000, 0x4472C4, 0xF8CBAD, 0xFFFFFF, 0xFFC000, 0xA5A5A5, 0x000000, 0x70AD47]
            outline = {2: 0xED7D31, 3: 0x4472C4, 6: 0xFFFFFF}.get(k)
            cr.select_font_face("Sans", 0, 1)
            cr.set_font_size(34)
            cr.move_to(12, 38)
            cr.text_path("A")
            c = theme[k % len(theme)]
            cr.set_source_rgb(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255)
            cr.fill_preserve()
            if outline is not None:
                cr.set_source_rgb(((outline >> 16) & 255) / 255, ((outline >> 8) & 255) / 255, (outline & 255) / 255)
                cr.set_line_width(1.2)
                cr.stroke()
            else:
                cr.new_path()
        da.connect("draw", draw)
        b.add(da)

        def picked(_b, k=k):
            pop.popdown()
            GLib.idle_add(lambda: (win.run("wordart-insert:%d" % k), False)[1])
        b.connect("clicked", picked)
        grid.attach(b, k % 4, k // 4, 1, 1)
    pop.add(grid)
    grid.show_all()
    return pop


class Shapes:
    """Drawing a new shape with the mouse, and typing in shapes."""

    def __init__(self, win):
        self.win = win
        self.placing = None          # the preset being drawn
        self.start = None
        self.cur = None
        self.editor = None

    # ------------------------------------------------------------ drawing a new one
    def place(self, prst):
        if getattr(self.win, "ink", None) is not None and self.win.ink.active():
            self.win.ink.set_tool(None)
        self.placing = prst
        self.start = self.cur = None
        area = self.win.grid.area
        if area.get_window() is not None:
            area.get_window().set_cursor(Gdk.Cursor.new_from_name(area.get_display(), "crosshair"))
        self.win.status.extra.set_text("Drag on the sheet to draw the %s. Esc cancels." % pretty(prst).lower())

    def cancel(self):
        self.placing = self.start = self.cur = None
        area = self.win.grid.area
        if area.get_window() is not None:
            area.get_window().set_cursor(None)
        self.win.status.extra.set_text("")
        area.queue_draw()

    def press(self, ev):
        if not self.placing or ev.button != 1:
            return False
        self.start = self.cur = (ev.x, ev.y)
        return True

    def motion(self, ev):
        if not self.placing or self.start is None:
            return bool(self.placing)
        x, y = ev.x, ev.y
        if ev.get_state() & Gdk.ModifierType.SHIFT_MASK:
            dx, dy = x - self.start[0], y - self.start[1]
            if self.placing in ("line", "straightConnector1"):
                # 45° steps
                import math
                ang = round(math.atan2(dy, dx) / (math.pi / 4)) * (math.pi / 4)
                r = math.hypot(dx, dy)
                x, y = self.start[0] + r * math.cos(ang), self.start[1] + r * math.sin(ang)
            else:
                d = max(abs(dx), abs(dy))
                x, y = self.start[0] + (d if dx >= 0 else -d), self.start[1] + (d if dy >= 0 else -d)
        self.cur = (x, y)
        self.win.grid.area.queue_draw()
        return True

    def release(self, ev):
        if not self.placing or self.start is None:
            return False
        prst = self.placing
        g = self.win.grid
        b = self.win.book
        p = g.params()
        (x0, y0), (x1, y1) = self.start, self.cur or self.start
        sx0, sy0 = b.sheet_point(g.sheet, x0, y0, p)
        sx1, sy1 = b.sheet_point(g.sheet, x1, y1, p)
        self.cancel()
        line = prst in ("line", "straightConnector1")
        if abs(sx1 - sx0) < 3 and abs(sy1 - sy0) < 3:
            # a click: the default size
            w, h = (DEFAULT * 2, DEFAULT * 0.4) if prst == "textBox" else (DEFAULT, 0.0 if line else DEFAULT)
            sx1, sy1 = sx0 + w, sy0 + h
        flip_h, flip_v = sx1 < sx0, sy1 < sy0
        x, y, w, h = min(sx0, sx1), min(sy0, sy1), abs(sx1 - sx0), abs(sy1 - sy0)
        if prst == "textBox":
            i = b.insert_text_box(g.sheet, x, y, max(w, 20), max(h, 18), "")
        else:
            i = b.insert_shape(g.sheet, prst, x, y, max(w, 1), max(h, 1), line and flip_h, line and flip_v)
        g.select_object(i)
        self.win.on_object_selection(switch_tab=True)
        self.win.update_title()
        g.refresh()
        if prst == "textBox":
            GLib.idle_add(lambda: (self.edit(i), False)[1])
        return True

    def draw(self, cr):
        if self.placing and self.start and self.cur:
            (x0, y0), (x1, y1) = self.start, self.cur
            cr.set_source_rgb(0.3, 0.3, 0.3)
            cr.set_line_width(1)
            cr.set_dash([4, 3])
            if self.placing in ("line", "straightConnector1"):
                cr.move_to(x0, y0)
                cr.line_to(x1, y1)
            else:
                cr.rectangle(min(x0, x1) + 0.5, min(y0, y1) + 0.5, abs(x1 - x0), abs(y1 - y0))
            cr.stroke()
            cr.set_dash([])

    # ------------------------------------------------------------ typing in a shape
    def edit(self, i, first=None):
        g = self.win.grid
        b = self.win.book
        info = b.shape_info(g.sheet, i)
        if info is None or info["connector"]:
            return False
        if info.get("equation") is not None:
            from . import equation_ui
            return equation_ui.edit(self.win, i, first or None)
        self.finish()
        rect = b.object_screen_rect(g.sheet, i, g.params())
        if rect is None:
            return False
        pop = Gtk.Popover.new(g.area)
        r = Gdk.Rectangle()
        r.x, r.y, r.width, r.height = int(rect[0]), int(rect[1] + rect[3] / 2), int(max(rect[2], 1)), 1
        pop.set_pointing_to(r)
        pop.set_position(Gtk.PositionType.BOTTOM)
        tv = Gtk.TextView()
        tv.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        tv.get_buffer().set_text(info["text"] if first is None else first)
        tv.set_size_request(max(160, int(rect[2])), max(48, int(rect[3])))
        tv.set_name("shape-text")
        sw = Gtk.ScrolledWindow()
        sw.add(tv)
        pop.add(sw)
        self.editor = (pop, tv, i, g.sheet)

        def key(w, ev):
            if ev.keyval == Gdk.KEY_Escape:
                self.finish()
                return True
            return False
        tv.connect("key-press-event", key)
        pop.connect("closed", lambda *_: GLib.idle_add(lambda: (self.finish(), False)[1]))
        sw.show_all()
        pop.popup()
        tv.grab_focus()
        buf = tv.get_buffer()
        buf.place_cursor(buf.get_end_iter())
        return True

    def finish(self):
        """Put what was typed into the shape."""
        if not self.editor:
            return
        pop, tv, i, s = self.editor
        self.editor = None
        buf = tv.get_buffer()
        text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)
        b = self.win.book
        try:
            info = b.shape_info(s, i)
            if info is not None and info["text"] != text:
                b.shape_set_text(s, i, text)
                self.win.update_title()
        except Exception as e:
            self.win.status.extra.set_text(str(e))
        try:
            pop.popdown()
            pop.destroy()
        except Exception:
            pass
        self.win.grid.refresh()


def selected_shapes(win):
    g = win.grid
    b = win.book
    if b is None:
        return []
    return [i for i in g.sel_objects if b.shape_info(g.sheet, i) is not None]


def insert_wordart(win, style):
    g = win.grid
    b = win.book
    a = g.area.get_allocation()
    p = g.params()
    x, y = b.sheet_point(g.sheet, a.width / 2, a.height / 2, p)
    i = b.insert_wordart(g.sheet, int(style), "Your text here", x, y)
    g.select_object(i)
    win.on_object_selection(switch_tab=True)
    win.update_title()
    g.refresh()
    return i


def build_tab(win, B):
    """The Shape Format tab."""
    from .ribbon import Tab
    from .ribbon_defs import color_palette
    from . import objects_ui
    r = win.ribbon
    t = r.building(Tab("Shape Format", "JD", contextual=True))
    refs = {"fill": [None], "line": [None], "text": [None]}

    def palette(kind, cmd, none_label):
        def make():
            pop = Gtk.Popover()
            refs[kind][0] = pop
            box = color_palette(lambda c: win.run("%s:%s" % (cmd, "none" if c is None else "%06X" % c)),
                                none_label=none_label, popref=refs[kind])
            if kind == "line":
                ex = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
                ex.set_border_width(6)
                ex.pack_start(Gtk.Label(label="Weight", xalign=0), False, False, 0)
                row = Gtk.Box(spacing=2)
                for pt in (0.25, 0.5, 0.75, 1, 1.5, 2.25, 3, 4.5, 6):
                    b = Gtk.Button(label="%g pt" % pt)
                    b.set_relief(Gtk.ReliefStyle.NONE)
                    b.connect("clicked", lambda _b, pt=pt: (pop.popdown(), win.run("shape-weight:%g" % pt)))
                    row.pack_start(b, False, False, 0)
                ex.pack_start(row, False, False, 0)
                dash = Gtk.Box(spacing=4)
                for label, v in (("Solid", "off"), ("Dashed", "on")):
                    b = Gtk.Button(label=label)
                    b.set_relief(Gtk.ReliefStyle.NONE)
                    b.connect("clicked", lambda _b, v=v: (pop.popdown(), win.run("shape-dash:" + v)))
                    dash.pack_start(b, False, False, 0)
                ex.pack_start(Gtk.Label(label="Dashes", xalign=0), False, False, 0)
                ex.pack_start(dash, False, False, 0)
                box.pack_start(ex, False, False, 0)
            pop.add(box)
            return pop
        return make

    B.group(t, "Insert Shapes", [
        B.big("Shapes", "r-shapes", None, "SH", menu=lambda: gallery(win, lambda p: win.run("shape-insert:" + p))),
        [B.small("Edit Shape", "r-edit-shape-sm", None, "E",
                 menu=lambda: gallery(win, lambda p: win.run("shape-change:" + p))),
         B.small("Text Box", "r-text-box-sm", "text-box", "X")],
    ])
    B.group(t, "Shape Styles", [
        [B.small("Shape Fill", "r-shape-fill-sm", None, "SF", menu=palette("fill", "shape-fill", "No Fill")),
         B.small("Shape Outline", "r-shape-outline-sm", None, "SO", menu=palette("line", "shape-outline", "No Outline"))],
    ])
    B.group(t, "WordArt Styles", [
        B.big("Quick\nStyles", "r-wordart", None, "Q", menu=lambda: wordart_gallery(win)),
        [B.small("Text Fill", "r-text-fill-sm", None, "TF", menu=palette("text", "shape-text-color", None))],
    ])
    B.group(t, "Arrange", [
        [B.small("Bring Forward", "r-bring-forward-sm", "bring-forward", "AF", menu=[("Bring Forward", "bring-forward"),
                                                                                  ("Bring to Front", "bring-to-front")]),
         B.small("Send Backward", "r-send-backward-sm", "send-backward", "AE", menu=[("Send Backward", "send-backward"),
                                                                                  ("Send to Back", "send-to-back")]),
         B.small("Selection Pane", "r-selection-pane-sm", "selection-pane", "AP")],
        [B.small("Align", "r-align-sm", None, "AA", menu=objects_ui.ALIGN_MENU),
         B.small("Group", "r-group-sm", None, "AG", menu=objects_ui.GROUP_MENU),
         B.small("Rotate", "r-rotate-sm", None, "AY", menu=objects_ui.ROTATE_MENU)],
    ])
    B.group(t, "Accessibility", [B.big("Alt\nText", "r-alt-text", "alt-text", "AT")])
    win.shape_h = Gtk.SpinButton.new_with_range(0.01, 100, 0.01)
    win.shape_w = Gtk.SpinButton.new_with_range(0.01, 100, 0.01)
    size = Gtk.Grid(column_spacing=4, row_spacing=2)
    for k, (label, spin, what) in enumerate((("Height:", win.shape_h, "height"), ("Width:", win.shape_w, "width"))):
        spin.set_digits(2)
        spin.set_width_chars(5)
        spin.set_tooltip_text("Shape %s (inches)" % what.capitalize())
        spin.connect("activate", lambda s, what=what: win.run("shape-%s:%g" % (what, s.get_value())))
        spin.connect("value-changed", lambda s, what=what: (not getattr(win, "_shape_sync", False)) and win.run("shape-%s:%g" % (what, s.get_value())))
        size.attach(Gtk.Label(label=label, xalign=1), 0, k, 1, 1)
        size.attach(spin, 1, k, 1, 1)
        size.attach(Gtk.Label(label="\""), 2, k, 1, 1)
        t.controls.append(spin)
    B.group(t, "Size", [size])
    r.add_tab(t)
    r.show_contextual("Shape Format", False)


def sync_tab(win):
    """Show the Shape Format tab for a selected shape, with its size."""
    shapes = selected_shapes(win)
    from . import equation_ui
    equation_ui.sync_tab(win)
    win.ribbon.show_contextual("Shape Format", bool(shapes))
    if shapes:
        g = win.grid
        o = win.book.objects(g.sheet)[shapes[0]]
        x, y, w, h = o["rect"]
        win._shape_sync = True
        try:
            win.shape_h.set_value(h / 96.0)
            win.shape_w.set_value(w / 96.0)
        finally:
            win._shape_sync = False
    return shapes
