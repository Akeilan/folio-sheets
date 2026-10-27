"""Format Cells (Ctrl+1): Number, Alignment, Font, Border, Fill and Protection, laid out
like the original. Only what you change is applied, so a selection of differently
formatted cells keeps everything you leave alone."""
import math

from gi.repository import Gdk, GLib, Gtk, Pango, PangoCairo

from .core import format_number
from .ribbon_defs import FONT_SIZES, STANDARD_COLORS, THEME_COLORS, palette_rows, system_fonts

# ---------------------------------------------------------------- number categories
DATE_CODES = ["m/d/yyyy", "[$-F800]dddd, mmmm dd, yyyy", "m/d", "m/d/yy", "mm/dd/yy", "d-mmm", "d-mmm-yy",
              "dd-mmm-yy", "mmm-yy", "mmmm-yy", "mmmm d, yyyy", "m/d/yy h:mm AM/PM", "m/d/yy h:mm", "mmmmm",
              "mmmmm-yy", "d-mmm-yyyy"]
TIME_CODES = ["[$-F400]h:mm:ss AM/PM", "h:mm;@", "h:mm AM/PM", "h:mm:ss AM/PM", "h:mm:ss", "[h]:mm:ss",
              "mm:ss.0", "m/d/yy h:mm AM/PM", "m/d/yy h:mm"]
FRACTION_CODES = [("Up to one digit (1/4)", "# ?/?"), ("Up to two digits (21/25)", "# ??/??"),
                  ("Up to three digits (312/943)", "# ???/???"), ("As halves (1/2)", "# ?/2"),
                  ("As quarters (2/4)", "# ?/4"), ("As eighths (4/8)", "# ?/8"),
                  ("As sixteenths (8/16)", "# ??/16"), ("As tenths (3/10)", "# ?/10"),
                  ("As hundredths (30/100)", "# ??/100")]
SPECIAL_CODES = [("Zip Code", "00000"), ("Zip Code + 4", "00000-0000"),
                 ("Phone Number", "[<=9999999]###-####;(###) ###-####"),
                 ("Social Security Number", "000-00-0000")]
SYMBOLS = [("None", ""), ("$", '"$"'), ("€", '"€"'), ("£", '"£"'), ("¥", '"¥"'), ("₹", '"₹"'), ("CHF", '"CHF"')]
CUSTOM_LIST = ["General", "0", "0.00", "#,##0", "#,##0.00", "#,##0_);(#,##0)", "#,##0_);[Red](#,##0)",
               "#,##0.00_);(#,##0.00)", "#,##0.00_);[Red](#,##0.00)", '"$"#,##0_);\\("$"#,##0\\)',
               '"$"#,##0_);[Red]\\("$"#,##0\\)', '"$"#,##0.00_);\\("$"#,##0.00\\)',
               '"$"#,##0.00_);[Red]\\("$"#,##0.00\\)', "0%", "0.00%", "0.00E+00", "##0.0E+0", "# ?/?",
               "# ??/??", "m/d/yyyy", "d-mmm-yy", "d-mmm", "mmm-yy", "h:mm AM/PM", "h:mm:ss AM/PM", "h:mm",
               "h:mm:ss", "m/d/yyyy h:mm", "mm:ss", "mm:ss.0", "@", "[h]:mm:ss",
               '_("$"* #,##0_);_("$"* \\(#,##0\\);_("$"* "-"_);_(@_)', '_(* #,##0_);_(* \\(#,##0\\);_(* "-"_);_(@_)',
               '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)',
               '_(* #,##0.00_);_(* \\(#,##0.00\\);_(* "-"??_);_(@_)']
CATEGORIES = ["General", "Number", "Currency", "Accounting", "Date", "Time", "Percentage", "Fraction",
              "Scientific", "Text", "Special", "Custom"]
NEGATIVE = [("-1,234.10", False), ("1,234.10", True), ("(1,234.10)", False), ("(1,234.10)", True)]

H_ALIGN = [("General", "general"), ("Left (Indent)", "left"), ("Center", "center"), ("Right (Indent)", "right"),
           ("Fill", "fill"), ("Justify", "justify"), ("Center Across Selection", "centerContinuous"),
           ("Distributed (Indent)", "distributed")]
V_ALIGN = [("Top", "top"), ("Center", "center"), ("Bottom", "bottom"), ("Justify", "justify"),
           ("Distributed", "distributed")]
UNDERLINES = [("None", None), ("Single", "single"), ("Double", "double"), ("Single Accounting", "singleAccounting"),
              ("Double Accounting", "doubleAccounting")]
LINE_STYLES = ["hair", "dotted", "dashDotDot", "dashDot", "dashed", "thin", "mediumDashDotDot", "slantDashDot",
               "mediumDashDot", "mediumDashed", "medium", "thick", "double"]
PATTERNS = [("Solid", "solid"), ("75% Gray", "darkGray"), ("50% Gray", "mediumGray"), ("25% Gray", "lightGray"),
            ("12.5% Gray", "gray125"), ("6.25% Gray", "gray0625"), ("Horizontal Stripe", "darkHorizontal"),
            ("Vertical Stripe", "darkVertical"), ("Reverse Diagonal Stripe", "darkDown"),
            ("Diagonal Stripe", "darkUp"), ("Diagonal Crosshatch", "darkGrid"),
            ("Thick Diagonal Crosshatch", "darkTrellis"), ("Thin Horizontal Stripe", "lightHorizontal"),
            ("Thin Vertical Stripe", "lightVertical"), ("Thin Reverse Diagonal Stripe", "lightDown"),
            ("Thin Diagonal Stripe", "lightUp"), ("Thin Horizontal Crosshatch", "lightGrid"),
            ("Thin Diagonal Crosshatch", "lightTrellis")]
def resolve_c(c):
    from .ribbon_defs import resolve
    return resolve(c)


def _T(slot, t=0.0):
    from .ribbon_defs import theme_ref
    return theme_ref(slot, t)


CELL_STYLES = [
    ("Good, Bad and Neutral", [
        ("Normal", {"reset": True}),
        ("Bad", {"font": {"color": 0x9C0006}, "fill": {"pattern": "solid", "fg": 0xFFC7CE}}),
        ("Good", {"font": {"color": 0x006100}, "fill": {"pattern": "solid", "fg": 0xC6EFCE}}),
        ("Neutral", {"font": {"color": 0x9C5700}, "fill": {"pattern": "solid", "fg": 0xFFEB9C}})]),
    ("Data and Model", [
        ("Calculation", {"font": {"bold": True, "color": 0xFA7D00}, "fill": {"pattern": "solid", "fg": 0xF2F2F2},
                         "border": {s: ("thin", 0x7F7F7F) for s in ("left", "right", "top", "bottom")}}),
        ("Check Cell", {"font": {"bold": True, "color": 0xFFFFFF}, "fill": {"pattern": "solid", "fg": 0xA5A5A5},
                        "border": {s: ("double", 0x3F3F3F) for s in ("left", "right", "top", "bottom")}}),
        ("Explanatory Text", {"font": {"italic": True, "color": 0x7F7F7F}}),
        ("Input", {"font": {"color": 0x3F3F76}, "fill": {"pattern": "solid", "fg": 0xFFCC99},
                   "border": {s: ("thin", 0x7F7F7F) for s in ("left", "right", "top", "bottom")}}),
        ("Linked Cell", {"font": {"color": 0xFA7D00}, "border": {"bottom": ("double", 0xFF8001)}}),
        ("Note", {"fill": {"pattern": "solid", "fg": 0xFFFFCC},
                  "border": {s: ("thin", 0xB2B2B2) for s in ("left", "right", "top", "bottom")}}),
        ("Output", {"font": {"bold": True, "color": 0x3F3F3F}, "fill": {"pattern": "solid", "fg": 0xF2F2F2},
                    "border": {s: ("thin", 0x3F3F3F) for s in ("left", "right", "top", "bottom")}}),
        ("Warning Text", {"font": {"color": 0xFF0000}})]),
    ("Titles and Headings", [
        ("Heading 1", {"font": {"bold": True, "size": 15, "color": _T(3)}, "border": {"bottom": ("thick", _T(4))}}),
        ("Heading 2", {"font": {"bold": True, "size": 13, "color": _T(3)}, "border": {"bottom": ("thick", _T(4, 0.5))}}),
        ("Heading 3", {"font": {"bold": True, "color": _T(3)}, "border": {"bottom": ("medium", _T(4, 0.4))}}),
        ("Heading 4", {"font": {"bold": True, "color": _T(3)}}),
        ("Title", {"font": {"bold": True, "size": 18, "color": _T(3), "name": "@major"}}),
        ("Total", {"font": {"bold": True}, "border": {"top": ("thin", _T(4)), "bottom": ("double", _T(4))}})]),
    ("Themed Cell Styles",
     [("20%% - Accent%d" % k, {"fill": {"pattern": "solid", "fg": _T(3 + k, 0.8)}}) for k in range(1, 7)]
     + [("40%% - Accent%d" % k, {"fill": {"pattern": "solid", "fg": _T(3 + k, 0.6)}}) for k in range(1, 7)]
     + [("60%% - Accent%d" % k, {"font": {"color": _T(0)}, "fill": {"pattern": "solid", "fg": _T(3 + k, 0.4)}}) for k in range(1, 7)]
     + [("Accent%d" % k, {"font": {"color": _T(0)}, "fill": {"pattern": "solid", "fg": _T(3 + k)}}) for k in range(1, 7)]),
    ("Number Format", [
        ("Comma", {"numfmt": '_(* #,##0.00_);_(* \\(#,##0.00\\);_(* "-"??_);_(@_)'}),
        ("Comma [0]", {"numfmt": '_(* #,##0_);_(* \\(#,##0\\);_(* "-"_);_(@_)'}),
        ("Currency", {"numfmt": '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)'}),
        ("Currency [0]", {"numfmt": '_("$"* #,##0_);_("$"* \\(#,##0\\);_("$"* "-"_);_(@_)'}),
        ("Percent", {"numfmt": "0%"})]),
]


def rgbf(c):
    from .ribbon_defs import resolve
    c = resolve(c)
    return ((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255


def plain_color(c, default=0):
    """A cell colour from the file (int, theme or indexed tuple) as a colour value: theme colours
    stay theme references (drawn with rgbf, which works them out)."""
    if isinstance(c, int):
        return c
    if isinstance(c, tuple) and c and c[0] == "theme":
        from .ribbon_defs import theme_ref
        return theme_ref(c[1], c[2] if len(c) > 2 and c[2] else 0.0)
    if isinstance(c, tuple) and c and c[0] == "indexed":
        idx = [0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF] * 2
        return idx[c[1]] if c[1] < len(idx) else default
    return default


def draw_line_style(cr, style, x0, x1, y, color=(0, 0, 0)):
    cr.save()
    cr.set_source_rgb(*color)
    w = {"hair": 0.6, "thin": 1, "dotted": 1, "dashed": 1, "dashDot": 1, "dashDotDot": 1, "medium": 2,
         "mediumDashed": 2, "mediumDashDot": 2, "mediumDashDotDot": 2, "slantDashDot": 2, "thick": 3,
         "double": 1}.get(style, 1)
    dashes = {"dotted": [1, 2], "hair": [1, 1], "dashed": [3, 1], "dashDot": [6, 2, 2, 2],
              "dashDotDot": [6, 2, 2, 2, 2, 2], "mediumDashed": [6, 3], "mediumDashDot": [8, 3, 3, 3],
              "mediumDashDotDot": [8, 3, 3, 3, 3, 3], "slantDashDot": [8, 2, 3, 2]}.get(style)
    cr.set_line_width(w)
    if dashes:
        cr.set_dash(dashes)
    if style == "double":
        for dy in (-1.5, 1.5):
            cr.move_to(x0, y + dy + 0.5)
            cr.line_to(x1, y + dy + 0.5)
        cr.stroke()
    else:
        off = 0.5 if w % 2 else 0
        cr.move_to(x0, y + off)
        cr.line_to(x1, y + off)
        cr.stroke()
    cr.restore()


class Swatch(Gtk.DrawingArea):
    def __init__(self, color, w=18, h=14, none_text=None):
        Gtk.DrawingArea.__init__(self)
        self.color = color
        self.none_text = none_text
        self.set_size_request(w, h)
        self.connect("draw", self._draw)

    def set_color(self, c):
        self.color = c
        self.queue_draw()

    def _draw(self, w, cr):
        a = w.get_allocation()
        if self.color is None:
            cr.set_source_rgb(1, 1, 1)
            cr.rectangle(0, 0, a.width, a.height)
            cr.fill()
            if self.none_text:
                cr.set_source_rgb(0.3, 0.3, 0.3)
                lay = PangoCairo.create_layout(cr)
                lay.set_text(self.none_text, -1)
                lay.set_font_description(Pango.FontDescription("Sans 8"))
                cr.move_to(4, (a.height - lay.get_pixel_size()[1]) / 2)
                PangoCairo.show_layout(cr, lay)
        else:
            cr.set_source_rgb(*rgbf(self.color))
            cr.rectangle(0, 0, a.width, a.height)
            cr.fill()
        cr.set_source_rgb(0.6, 0.6, 0.6)
        cr.set_line_width(1)
        cr.rectangle(0.5, 0.5, a.width - 1, a.height - 1)
        cr.stroke()


def palette_grid(on_pick, none_label=None, more=None):
    """Theme and standard colours as clickable swatches (the original's colour picker)."""
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(6)

    from .ribbon_defs import ACTIVE_THEME, color_name, palette_tints, theme_ref

    def sw(c):
        b = Gtk.Button()
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.get_style_context().add_class("folio-cswatch")
        b.add(Swatch(c, 14, 14))
        b.set_tooltip_text(color_name(c))
        b.connect("clicked", lambda *_: on_pick(c))
        return b
    if none_label:
        nb = Gtk.Button(label=none_label)
        nb.connect("clicked", lambda *_: on_pick(None))
        box.pack_start(nb, False, False, 0)
    grid = Gtk.Grid(column_spacing=0, row_spacing=0)
    for col, c in enumerate(ACTIVE_THEME[:10]):
        grid.attach(sw(theme_ref(col)), col, 0, 1, 1)
        for row, t in enumerate(palette_tints(c), start=1):
            grid.attach(sw(theme_ref(col, t)), col, row, 1, 1)
    box.pack_start(grid, False, False, 0)
    std = Gtk.Box(spacing=0)
    for c in STANDARD_COLORS:
        std.pack_start(sw(c), False, False, 0)
    box.pack_start(std, False, False, 2)
    if more:
        mb = Gtk.Button(label="More Colors...")
        mb.connect("clicked", lambda *_: more())
        box.pack_start(mb, False, False, 0)
    return box


class ColorButton(Gtk.MenuButton):
    """A colour box with a drop-down palette."""

    def __init__(self, color, on_change, none_label="Automatic", parent=None):
        Gtk.MenuButton.__init__(self)
        self.on_change = on_change
        self.parent_win = parent
        row = Gtk.Box(spacing=4)
        self.swatch = Swatch(color, 84, 14, none_text=none_label)
        row.pack_start(self.swatch, False, False, 0)
        row.pack_start(Gtk.Arrow(arrow_type=Gtk.ArrowType.DOWN, shadow_type=Gtk.ShadowType.NONE), False, False, 0)
        self.add(row)
        self.pop = Gtk.Popover()
        self.pop.add(palette_grid(self._pick, none_label, self._more))
        self.pop.get_child().show_all()
        self.set_popover(self.pop)
        self.color = color

    def set_color(self, c):
        self.color = c
        self.swatch.set_color(c)

    def _pick(self, c):
        self.pop.popdown()
        self.set_color(c)
        self.on_change(c)

    def _more(self):
        self.pop.popdown()
        d = Gtk.ColorChooserDialog(title="Colors", transient_for=self.parent_win)
        d.set_use_alpha(False)
        rgba = Gdk.RGBA()
        rgba.parse("#%06X" % resolve_c(self.color or 0))
        d.set_rgba(rgba)
        if d.run() == Gtk.ResponseType.OK:
            c = d.get_rgba()
            self._pick((round(c.red * 255) << 16) | (round(c.green * 255) << 8) | round(c.blue * 255))
        d.destroy()


def _list(items, height=120, width=-1):
    store = Gtk.ListStore(str)
    for it in items:
        store.append([it])
    tv = Gtk.TreeView(model=store)
    tv.set_headers_visible(False)
    tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(width, height)
    sw.add(tv)
    return sw, tv, store


def _select_row(tv, text):
    m = tv.get_model()
    for i, row in enumerate(m):
        if row[0] == text:
            tv.get_selection().select_path(Gtk.TreePath(i))
            tv.scroll_to_cell(Gtk.TreePath(i), None, False, 0, 0)
            return True
    tv.get_selection().unselect_all()
    return False


def _selected(tv):
    m, it = tv.get_selection().get_selected()
    return m[it][0] if it else None


def _frame(title, child):
    lab = Gtk.Label(label=title, xalign=0)
    lab.get_style_context().add_class("folio-fc-heading")
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.pack_start(lab, False, False, 0)
    box.pack_start(child, False, False, 0)
    return box


def guess_category(code):
    c = code or "General"
    if c == "General":
        return "General", {}
    if c == "@":
        return "Text", {}
    for name, sc in SPECIAL_CODES:
        if c == sc:
            return "Special", {"special": name}
    for name, fc in FRACTION_CODES:
        if c == fc:
            return "Fraction", {"fraction": name}
    if c in DATE_CODES:
        return "Date", {"date": c}
    if c in TIME_CODES:
        return "Time", {"time": c}
    first = c.split(";")[0]
    dec = len(first.split(".", 1)[1]) - len(first.split(".", 1)[1].lstrip("0")) if "." in first else 0
    if first.endswith("%") and set(first.replace(".", "").replace("%", "")) <= {"0"}:
        return "Percentage", {"dec": dec}
    if "E+" in first and first.startswith("0"):
        return "Scientific", {"dec": dec}
    if first.startswith("_(") and "*" in first:
        sym = next((s for s, q in SYMBOLS if q and q in first), "None")
        return "Accounting", {"dec": dec, "sym": sym}
    sym = next((s for s, q in SYMBOLS if q and first.startswith(q)), None)
    plain = first.replace(",", "").replace("_)", "")
    if sym:
        plain = plain[len(dict(SYMBOLS)[sym]):]
    if plain.replace(".", "").strip("0") == "" or plain.replace(".", "").strip("0#") == "":
        neg = 0
        parts = c.split(";")
        if len(parts) > 1:
            red = "[Red]" in parts[1]
            paren = "(" in parts[1]
            neg = (2 if paren else 0) + (1 if red else 0)
        d = {"dec": dec, "sep": "," in first, "neg": neg}
        if sym:
            d["sym"] = sym
            return "Currency", d
        return "Number", d
    return "Custom", {}


def number_code(cat, o):
    dec = int(o.get("dec", 2))
    frac = ("." + "0" * dec) if dec else ""
    if cat == "General":
        return "General"
    if cat == "Text":
        return "@"
    if cat in ("Number", "Currency"):
        sym = dict(SYMBOLS).get(o.get("sym", "$"), "") if cat == "Currency" else ""
        sep = cat == "Currency" or o.get("sep")
        base = sym + ("#,##0" if sep else "0") + frac
        neg = int(o.get("neg", 0))
        if neg == 0:
            return base
        if neg == 1:
            return base + ";[Red]" + base
        if neg == 2:
            return base + "_);(" + base + ")"
        return base + "_);[Red](" + base + ")"
    if cat == "Accounting":
        sym = dict(SYMBOLS).get(o.get("sym", "$"), "")
        s = (sym + "* ") if sym else "* "
        q = "?" * dec
        return '_(%s#,##0%s_);_(%s\\(#,##0%s\\);_(%s"-"%s_);_(@_)' % (
            s, frac, s, frac, s, q if q else "")
    if cat == "Percentage":
        return "0" + frac + "%"
    if cat == "Scientific":
        return "0" + frac + "E+00"
    if cat == "Date":
        return o.get("date", DATE_CODES[0])
    if cat == "Time":
        return o.get("time", TIME_CODES[0])
    if cat == "Fraction":
        return dict(FRACTION_CODES).get(o.get("fraction"), "# ?/?")
    if cat == "Special":
        return dict(SPECIAL_CODES).get(o.get("special"), "00000")
    return o.get("custom", "General")


class OrientationDial(Gtk.DrawingArea):
    """The half-circle orientation control: click or drag to set the angle."""

    def __init__(self, on_change):
        Gtk.DrawingArea.__init__(self)
        self.set_size_request(110, 150)
        self.angle = 0
        self.vertical = False
        self.on_change = on_change
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_MOTION_MASK)
        self.connect("draw", self._draw)
        self.connect("button-press-event", self._pick)
        self.connect("motion-notify-event", self._pick)

    def _center(self):
        a = self.get_allocation()
        return 18, a.height / 2, min(a.width - 30, a.height / 2 - 10)

    def _draw(self, w, cr):
        a = w.get_allocation()
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        cx, cy, rad = self._center()
        cr.set_source_rgb(0.75, 0.75, 0.75)
        cr.set_line_width(1)
        cr.arc(cx, cy, rad, -math.pi / 2, math.pi / 2)
        cr.stroke()
        for deg in range(-90, 91, 15):
            t = math.radians(deg)
            x, y = cx + rad * math.cos(t), cy - rad * math.sin(t)
            cr.set_source_rgb(0.85, 0.1, 0.1) if (deg == self.angle and not self.vertical) else cr.set_source_rgb(0.2, 0.2, 0.2)
            cr.arc(x, y, 2.5 if deg % 45 == 0 else 1.5, 0, 2 * math.pi)
            cr.fill()
        if not self.vertical:
            t = math.radians(self.angle)
            cr.save()
            cr.translate(cx, cy)
            cr.rotate(-t)
            lay = PangoCairo.create_layout(cr)
            lay.set_text("Text", -1)
            lay.set_font_description(Pango.FontDescription("Sans 9"))
            tw, th = lay.get_pixel_size()
            cr.set_source_rgb(0, 0, 0)
            cr.move_to(4, -th / 2)
            PangoCairo.show_layout(cr, lay)
            cr.set_source_rgb(0.85, 0.1, 0.1)
            cr.move_to(tw + 8, 0)
            cr.line_to(rad - 4, 0)
            cr.stroke()
            cr.restore()

    def _pick(self, w, ev):
        cx, cy, rad = self._center()
        dx, dy = ev.x - cx, cy - ev.y
        if dx <= 0 and abs(dy) < 4:
            return True
        deg = round(math.degrees(math.atan2(dy, max(dx, 0.01))))
        deg = max(-90, min(90, deg))
        self.vertical = False
        self.set_angle(deg)
        self.on_change(deg)
        return True

    def set_angle(self, deg):
        self.angle = deg
        self.queue_draw()


class BorderPreview(Gtk.DrawingArea):
    """The clickable border preview: click near an edge (or the middle lines when
    several cells are selected) to apply or remove the current line."""

    def __init__(self, dlg):
        Gtk.DrawingArea.__init__(self)
        self.dlg = dlg
        self.set_size_request(200, 140)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.connect("draw", self._draw)
        self.connect("button-press-event", self._click)

    def _box(self):
        a = self.get_allocation()
        return 20, 15, a.width - 20, a.height - 15

    def _draw(self, w, cr):
        a = w.get_allocation()
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        x0, y0, x1, y1 = self._box()
        mx, my = (x0 + x1) / 2, (y0 + y1) / 2
        cr.set_source_rgb(0.6, 0.6, 0.6)
        cr.set_line_width(1)
        for (ax, ay, bx, by) in ((x0 - 8, y0, x0 - 2, y0), (x0, y0 - 8, x0, y0 - 2), (x1 + 2, y0, x1 + 8, y0),
                                 (x1, y0 - 8, x1, y0 - 2), (x0 - 8, y1, x0 - 2, y1), (x0, y1 + 2, x0, y1 + 8),
                                 (x1 + 2, y1, x1 + 8, y1), (x1, y1 + 2, x1, y1 + 8)):
            cr.move_to(ax + 0.5, ay + 0.5)
            cr.line_to(bx + 0.5, by + 0.5)
        cr.stroke()
        lay = PangoCairo.create_layout(cr)
        lay.set_text("Text", -1)
        lay.set_font_description(Pango.FontDescription("Sans 9"))
        tw, th = lay.get_pixel_size()
        cr.set_source_rgb(0, 0, 0)
        cols = 2 if self.dlg.multi_cols else 1
        rows = 2 if self.dlg.multi_rows else 1
        for i in range(cols):
            for j in range(rows):
                cx = x0 + (x1 - x0) * (i + 0.5) / cols
                cy = y0 + (y1 - y0) * (j + 0.5) / rows
                cr.move_to(cx - tw / 2, cy - th / 2)
                PangoCairo.show_layout(cr, lay)
        st = self.dlg.border_state

        def edge(key, ax, ay, bx, by):
            pen = st.get(key)
            if not pen:
                return
            style, color = pen
            cr.save()
            if ay == by:
                draw_line_style(cr, style, ax, bx, ay, rgbf(plain_color(color)))
            else:
                cr.translate(ax, ay)
                cr.rotate(math.pi / 2)
                draw_line_style(cr, style, 0, by - ay, 0, rgbf(plain_color(color)))
            cr.restore()
        edge("top", x0, y0, x1, y0)
        edge("bottom", x0, y1, x1, y1)
        edge("left", x0, y0, x0, y1)
        edge("right", x1, y0, x1, y1)
        if self.dlg.multi_rows:
            edge("insideH", x0, my, x1, my)
        if self.dlg.multi_cols:
            edge("insideV", mx, y0, mx, y1)
        for key, (ax, ay, bx, by) in (("diagDown", (x0, y0, x1, y1)), ("diagUp", (x0, y1, x1, y0))):
            pen = st.get(key)
            if pen:
                cr.save()
                cr.set_source_rgb(*rgbf(plain_color(pen[1])))
                cr.set_line_width(2 if pen[0] in ("medium", "thick") else 1)
                cr.move_to(ax, ay)
                cr.line_to(bx, by)
                cr.stroke()
                cr.restore()

    def _click(self, w, ev):
        x0, y0, x1, y1 = self._box()
        mx, my = (x0 + x1) / 2, (y0 + y1) / 2
        cand = [("top", abs(ev.y - y0)), ("bottom", abs(ev.y - y1)), ("left", abs(ev.x - x0)),
                ("right", abs(ev.x - x1))]
        if self.dlg.multi_rows:
            cand.append(("insideH", abs(ev.y - my)))
        if self.dlg.multi_cols:
            cand.append(("insideV", abs(ev.x - mx)))
        key, dist = min(cand, key=lambda t: t[1])
        if dist <= 8:
            self.dlg.toggle_edge(key)
        return True


class LineStylePicker(Gtk.DrawingArea):
    """The Style box of the Border tab: None plus thirteen line styles."""
    ORDER = ["none", "hair", "dotted", "dashDotDot", "dashDot", "dashed", "thin", "mediumDashDotDot",
             "slantDashDot", "mediumDashDot", "mediumDashed", "medium", "thick", "double"]

    def __init__(self, style, on_change):
        Gtk.DrawingArea.__init__(self)
        self.style = style
        self.on_change = on_change
        self.set_size_request(150, 7 * 18 + 4)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.connect("draw", self._draw)
        self.connect("button-press-event", self._click)

    def _cells(self):
        a = self.get_allocation()
        cw = a.width / 2
        out = []
        for i, s in enumerate(self.ORDER):
            col, row = (0, i) if i < 7 else (1, i - 7)
            out.append((s, col * cw, 2 + row * 18, cw, 18))
        return out

    def _draw(self, w, cr):
        a = w.get_allocation()
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        for s, x, y, cw, ch in self._cells():
            if s == self.style:
                cr.set_source_rgb(0.85, 0.9, 1)
                cr.rectangle(x + 1, y, cw - 2, ch)
                cr.fill()
                cr.set_source_rgb(0.3, 0.45, 0.8)
                cr.rectangle(x + 1.5, y + 0.5, cw - 3, ch - 1)
                cr.stroke()
            if s == "none":
                lay = PangoCairo.create_layout(cr)
                lay.set_text("None", -1)
                lay.set_font_description(Pango.FontDescription("Sans 8"))
                cr.set_source_rgb(0, 0, 0)
                cr.move_to(x + 8, y + 2)
                PangoCairo.show_layout(cr, lay)
            else:
                draw_line_style(cr, s, x + 8, x + cw - 8, round(y + ch / 2))

    def _click(self, w, ev):
        for s, x, y, cw, ch in self._cells():
            if x <= ev.x < x + cw and y <= ev.y < y + ch:
                self.style = s
                self.queue_draw()
                self.on_change(s)
        return True


class FormatCells(Gtk.Dialog):
    PAGES = ["number", "alignment", "font", "border", "fill", "protection"]

    def __init__(self, win, spec, page="number", value=None):
        Gtk.Dialog.__init__(self, title="Format Cells", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.win = win
        self.spec = spec
        self.value = value
        self.changes = {}
        self.touched = set()
        g = win.grid
        ranges = win.style_ranges()
        self.multi_rows = any(rg[2] > rg[0] for rg in ranges)
        self.multi_cols = any(rg[3] > rg[1] for rg in ranges)
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        self.set_resizable(False)
        area = self.get_content_area()
        area.set_border_width(8)
        self.nb = Gtk.Notebook()
        area.pack_start(self.nb, True, True, 0)
        self._building = True
        for title, build in (("Number", self.build_number), ("Alignment", self.build_alignment),
                             ("Font", self.build_font), ("Border", self.build_border), ("Fill", self.build_fill),
                             ("Protection", self.build_protection)):
            w = build()
            w.set_border_width(10)
            self.nb.append_page(w, Gtk.Label(label=title))
        self._building = False
        self.show_all()
        self.on_category()
        self.nb.set_current_page(self.PAGES.index(page) if page in self.PAGES else 0)
        self.set_size_request(560, 470)

    def touch(self, key):
        if not self._building:
            self.touched.add(key)

    # ================================================================ Number
    def build_number(self):
        box = Gtk.Box(spacing=12)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        left.pack_start(Gtk.Label(label="Category:", xalign=0), False, False, 0)
        sw, self.cat_tv, _ = _list(CATEGORIES, 300, 130)
        left.pack_start(sw, True, True, 0)
        box.pack_start(left, False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.pack_start(right, True, True, 0)
        self.sample = Gtk.Label(xalign=0)
        self.sample.set_ellipsize(Pango.EllipsizeMode.END)
        fr = Gtk.Frame(label="Sample")
        self.sample.set_margin_start(6)
        self.sample.set_margin_top(4)
        self.sample.set_margin_bottom(4)
        fr.add(self.sample)
        right.pack_start(fr, False, False, 0)
        self.num_opts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        right.pack_start(self.num_opts, True, True, 0)
        self.num_desc = Gtk.Label(xalign=0)
        self.num_desc.set_line_wrap(True)
        self.num_desc.set_max_width_chars(48)
        right.pack_end(self.num_desc, False, False, 0)
        cat, opts = guess_category(self.spec["numfmt"])
        self.num_cat = cat
        self.num_o = dict(opts)
        self.num_o.setdefault("dec", 2)
        self.num_o.setdefault("sym", "$")
        self.num_o.setdefault("custom", self.spec["numfmt"])
        self.orig_code = self.spec["numfmt"]
        _select_row(self.cat_tv, cat)
        self.cat_tv.get_selection().connect("changed", lambda *_: self.on_category(user=True))
        return box

    def _clear_opts(self):
        for c in self.num_opts.get_children():
            self.num_opts.remove(c)

    def on_category(self, user=False):
        cat = _selected(self.cat_tv) or "General"
        if user:
            self.touch("numfmt")
        self.num_cat = cat
        self._clear_opts()
        o = self.num_o
        opts = self.num_opts

        def dec_row():
            row = Gtk.Box(spacing=6)
            row.pack_start(Gtk.Label(label="Decimal places:"), False, False, 0)
            sp = Gtk.SpinButton.new_with_range(0, 30, 1)
            sp.set_value(o.get("dec", 2))

            def ch(s):
                o["dec"] = int(s.get_value())
                self.touch("numfmt")
                self.update_sample()
            sp.connect("value-changed", ch)
            row.pack_start(sp, False, False, 0)
            opts.pack_start(row, False, False, 0)
            self.dec_spin = sp

        def sym_row():
            row = Gtk.Box(spacing=6)
            row.pack_start(Gtk.Label(label="Symbol:"), False, False, 0)
            cb = Gtk.ComboBoxText()
            for s, _ in SYMBOLS:
                cb.append_text(s)
            names = [s for s, _ in SYMBOLS]
            cb.set_active(names.index(o.get("sym", "$")) if o.get("sym", "$") in names else 1)

            def ch(c):
                o["sym"] = c.get_active_text()
                self.touch("numfmt")
                self.update_sample()
            cb.connect("changed", ch)
            row.pack_start(cb, False, False, 0)
            opts.pack_start(row, False, False, 0)

        def neg_list():
            opts.pack_start(Gtk.Label(label="Negative numbers:", xalign=0), False, False, 0)
            store = Gtk.ListStore(str, str)
            sym = dict(SYMBOLS).get(o.get("sym", "$"), "").strip('"') if cat == "Currency" else ""
            for text, red in NEGATIVE:
                t = text.replace("1,234", "1234") if (cat == "Number" and not o.get("sep")) else text
                t = t.replace("1", sym + "1", 1)
                store.append([t, "#FF0000" if red else "#000000"])
            tv = Gtk.TreeView(model=store)
            tv.set_headers_visible(False)
            tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0, foreground=1))
            tv.get_selection().select_path(Gtk.TreePath(int(o.get("neg", 0))))

            def ch(sel):
                m, it = sel.get_selected()
                if it:
                    o["neg"] = m.get_path(it).get_indices()[0]
                    self.touch("numfmt")
                    self.update_sample()
            tv.get_selection().connect("changed", ch)
            sw = Gtk.ScrolledWindow()
            sw.set_shadow_type(Gtk.ShadowType.IN)
            sw.set_size_request(-1, 90)
            sw.add(tv)
            opts.pack_start(sw, False, False, 0)

        def pick_list(label, items, key):
            opts.pack_start(Gtk.Label(label=label, xalign=0), False, False, 0)
            shown = [(self._sample_for(code) if key in ("date", "time") else name, code)
                     for name, code in items]
            sw, tv, store = _list([s for s, _ in shown], 170)
            codes = [c for _, c in shown]
            cur = o.get(key)
            if key in ("fraction", "special"):
                cur_i = [n for n, _ in items].index(cur) if cur in [n for n, _ in items] else 0
            else:
                cur_i = codes.index(cur) if cur in codes else 0
            tv.get_selection().select_path(Gtk.TreePath(cur_i))
            o[key] = items[cur_i][0] if key in ("fraction", "special") else codes[cur_i]

            def ch(sel):
                m, it = sel.get_selected()
                if it:
                    i = m.get_path(it).get_indices()[0]
                    o[key] = items[i][0] if key in ("fraction", "special") else codes[i]
                    self.touch("numfmt")
                    self.update_sample()
            tv.get_selection().connect("changed", ch)
            opts.pack_start(sw, False, False, 0)

        desc = {
            "General": "General format cells have no specific number format.",
            "Number": "Number is used for general display of numbers. Currency and Accounting offer specialized "
                      "formatting for monetary value.",
            "Currency": "Currency formats are used for general monetary values. Use Accounting formats to align "
                        "decimal points in a column.",
            "Accounting": "Accounting formats line up the currency symbols and decimal points in a column.",
            "Date": "Date formats display date and time serial numbers as date values.",
            "Time": "Time formats display date and time serial numbers as time values.",
            "Percentage": "Percentage formats multiply the cell value by 100 and display the result with a "
                          "percent symbol.",
            "Fraction": "", "Scientific": "",
            "Text": "Text format cells are treated as text even when a number is in the cell. The cell is "
                    "displayed exactly as entered.",
            "Special": "Special formats are useful for tracking list and database values.",
            "Custom": "Type the number format code, using one of the existing codes as a starting point.",
        }
        if cat in ("Number", "Currency", "Percentage", "Scientific", "Accounting"):
            dec_row()
        if cat == "Number":
            sep = Gtk.CheckButton(label="Use 1000 Separator (,)")
            sep.set_active(bool(o.get("sep")))

            def ch(b):
                o["sep"] = b.get_active()
                self.touch("numfmt")
                self.on_category()
            sep.connect("toggled", ch)
            opts.pack_start(sep, False, False, 0)
        if cat in ("Currency", "Accounting"):
            sym_row()
        if cat in ("Number", "Currency"):
            neg_list()
        if cat == "Date":
            pick_list("Type:", [(c, c) for c in DATE_CODES], "date")
        if cat == "Time":
            pick_list("Type:", [(c, c) for c in TIME_CODES], "time")
        if cat == "Fraction":
            pick_list("Type:", FRACTION_CODES, "fraction")
        if cat == "Special":
            pick_list("Type:", SPECIAL_CODES, "special")
        if cat == "Custom":
            opts.pack_start(Gtk.Label(label="Type:", xalign=0), False, False, 0)
            self.custom_entry = Gtk.Entry()
            self.custom_entry.set_text(o.get("custom") or "General")
            self.custom_entry.set_activates_default(True)

            def typed(e):
                o["custom"] = e.get_text()
                self.touch("numfmt")
                self.update_sample()
            self.custom_entry.connect("changed", typed)
            opts.pack_start(self.custom_entry, False, False, 0)
            items = list(CUSTOM_LIST)
            if self.orig_code not in items:
                items.append(self.orig_code)
            sw, tv, store = _list(items, 150)
            _select_row(tv, o.get("custom"))
            tv.get_selection().connect("changed", lambda s: self.custom_entry.set_text(_selected(tv) or ""))
            opts.pack_start(sw, False, False, 0)
        self.num_desc.set_text(desc.get(cat, ""))
        opts.show_all()
        self.update_sample()

    def _sample_for(self, code):
        v = self.value if isinstance(self.value, (int, float)) and not isinstance(self.value, bool) else 45000.5
        try:
            return format_number(float(v), code)
        except Exception:
            return code

    def current_code(self):
        return number_code(self.num_cat, self.num_o)

    def update_sample(self):
        code = self.current_code()
        v = self.value
        if isinstance(v, bool):
            txt = "TRUE" if v else "FALSE"
        elif isinstance(v, (int, float)):
            try:
                txt = format_number(float(v), code)
            except Exception:
                txt = str(v)
        elif isinstance(v, str):
            txt = v
        else:
            txt = ""
        self.sample.set_text(txt or " ")

    # ================================================================ Alignment
    def build_alignment(self):
        a = self.spec["align"]
        outer = Gtk.Box(spacing=16)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        outer.pack_start(left, True, True, 0)
        g = Gtk.Grid(row_spacing=6, column_spacing=8)
        g.attach(Gtk.Label(label="Horizontal:", xalign=0), 0, 0, 1, 1)
        self.h_combo = Gtk.ComboBoxText()
        for t, _ in H_ALIGN:
            self.h_combo.append_text(t)
        hv = [v for _, v in H_ALIGN]
        self.h_combo.set_active(hv.index(a["horizontal"]) if a["horizontal"] in hv else 0)
        g.attach(self.h_combo, 0, 1, 1, 1)
        g.attach(Gtk.Label(label="Indent:", xalign=0), 1, 0, 1, 1)
        self.indent_spin = Gtk.SpinButton.new_with_range(0, 250, 1)
        self.indent_spin.set_value(a["indent"])
        g.attach(self.indent_spin, 1, 1, 1, 1)
        g.attach(Gtk.Label(label="Vertical:", xalign=0), 0, 2, 1, 1)
        self.v_combo = Gtk.ComboBoxText()
        for t, _ in V_ALIGN:
            self.v_combo.append_text(t)
        vv = [v for _, v in V_ALIGN]
        self.v_combo.set_active(vv.index(a["vertical"]) if a["vertical"] in vv else 2)
        g.attach(self.v_combo, 0, 3, 1, 1)
        left.pack_start(_frame("Text alignment", g), False, False, 0)

        def h_changed(c):
            self.touch("horizontal")
            ok = hv[c.get_active()] in ("left", "right", "distributed")
            self.indent_spin.set_sensitive(ok)
        self.h_combo.connect("changed", h_changed)
        self.indent_spin.set_sensitive(a["horizontal"] in ("left", "right", "distributed"))
        self.v_combo.connect("changed", lambda *_: self.touch("vertical"))
        self.indent_spin.connect("value-changed", lambda *_: self.touch("indent"))

        tc = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        self.wrap_chk = Gtk.CheckButton(label="Wrap text")
        self.wrap_chk.set_active(a["wrap"])
        self.shrink_chk = Gtk.CheckButton(label="Shrink to fit")
        self.shrink_chk.set_active(a["shrink"])
        self.merge_chk = Gtk.CheckButton(label="Merge cells")
        self.merge_chk.set_active(self.spec.get("merged", False))
        for chk, key in ((self.wrap_chk, "wrap"), (self.shrink_chk, "shrink"), (self.merge_chk, "merge")):
            chk.connect("toggled", lambda b, k=key: self.touch(k))
            tc.pack_start(chk, False, False, 0)
        left.pack_start(_frame("Text control", tc), False, False, 0)
        rl = Gtk.Box(spacing=8)
        rl.pack_start(Gtk.Label(label="Text direction:"), False, False, 0)
        self.dir_combo = Gtk.ComboBoxText()
        for t in ("Context", "Left-to-Right", "Right-to-Left"):
            self.dir_combo.append_text(t)
        self.dir_combo.set_active(a["reading"] if a["reading"] in (0, 1, 2) else 0)
        self.dir_combo.connect("changed", lambda *_: self.touch("reading"))
        rl.pack_start(self.dir_combo, False, False, 0)
        left.pack_start(_frame("Right-to-left", rl), False, False, 0)

        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        rot = a["rotation"]
        deg = rot if rot <= 90 else (90 - rot if rot <= 180 else 0)
        row = Gtk.Box(spacing=6)
        self.vert_btn = Gtk.ToggleButton()
        vlab = Gtk.Label(label="T\ne\nx\nt")
        self.vert_btn.add(vlab)
        self.vert_btn.set_active(rot == 255)
        self.vert_btn.set_tooltip_text("Vertical text")
        row.pack_start(self.vert_btn, False, False, 0)
        self.dial = OrientationDial(self._dial_changed)
        self.dial.vertical = rot == 255
        self.dial.set_angle(deg)
        row.pack_start(self.dial, False, False, 0)
        right.pack_start(row, False, False, 0)
        dr = Gtk.Box(spacing=6)
        self.deg_spin = Gtk.SpinButton.new_with_range(-90, 90, 1)
        self.deg_spin.set_value(deg)
        dr.pack_start(self.deg_spin, False, False, 0)
        dr.pack_start(Gtk.Label(label="Degrees"), False, False, 0)
        right.pack_start(dr, False, False, 0)
        self.deg_spin.connect("value-changed", self._deg_changed)
        self.vert_btn.connect("toggled", self._vert_toggled)
        outer.pack_start(_frame("Orientation", right), False, False, 0)
        return outer

    def _dial_changed(self, deg):
        self.vert_btn.set_active(False)
        self.deg_spin.set_value(deg)
        self.touch("rotation")

    def _deg_changed(self, s):
        self.dial.set_angle(int(s.get_value()))
        if not self._building:
            self.vert_btn.set_active(False)
        self.touch("rotation")

    def _vert_toggled(self, b):
        self.dial.vertical = b.get_active()
        self.dial.queue_draw()
        self.touch("rotation")

    # ================================================================ Font
    def build_font(self):
        f = self.spec["font"]
        g = Gtk.Grid(row_spacing=4, column_spacing=10)
        g.attach(Gtk.Label(label="Font:", xalign=0), 0, 0, 1, 1)
        g.attach(Gtk.Label(label="Font style:", xalign=0), 1, 0, 1, 1)
        g.attach(Gtk.Label(label="Size:", xalign=0), 2, 0, 1, 1)
        self.font_entry = Gtk.Entry()
        self.font_entry.set_text(f["name"])
        g.attach(self.font_entry, 0, 1, 1, 1)
        fonts = system_fonts()
        if f["name"] not in fonts:
            fonts = [f["name"]] + fonts
        sw, self.font_tv, _ = _list(fonts, 110, 220)
        g.attach(sw, 0, 2, 1, 1)
        styles = ["Regular", "Italic", "Bold", "Bold Italic"]
        cur_style = styles[(2 if f["bold"] else 0) + (1 if f["italic"] else 0)]
        self.fstyle_entry = Gtk.Entry()
        self.fstyle_entry.set_text(cur_style)
        self.fstyle_entry.set_editable(False)
        g.attach(self.fstyle_entry, 1, 1, 1, 1)
        sw2, self.fstyle_tv, _ = _list(styles, 110, 120)
        g.attach(sw2, 1, 2, 1, 1)
        self.size_entry = Gtk.Entry()
        self.size_entry.set_width_chars(5)
        self.size_entry.set_text("%g" % f["size"])
        g.attach(self.size_entry, 2, 1, 1, 1)
        sw3, self.size_tv, _ = _list(FONT_SIZES, 110, 60)
        g.attach(sw3, 2, 2, 1, 1)
        _select_row(self.font_tv, f["name"])
        _select_row(self.fstyle_tv, cur_style)
        _select_row(self.size_tv, "%g" % f["size"])

        def font_sel(sel):
            v = _selected(self.font_tv)
            if v and not self._syncing:
                self._syncing = True
                self.font_entry.set_text(v)
                self._syncing = False
                self.touch("font_name")
                self.update_preview()

        def font_typed(e):
            if self._syncing:
                return
            self._syncing = True
            _select_row(self.font_tv, e.get_text()) or self._prefix_select(self.font_tv, e.get_text())
            self._syncing = False
            self.touch("font_name")
            self.update_preview()

        def style_sel(sel):
            v = _selected(self.fstyle_tv)
            if v:
                self.fstyle_entry.set_text(v)
                self.touch("font_style")
                self.update_preview()

        def size_sel(sel):
            v = _selected(self.size_tv)
            if v and not self._syncing:
                self._syncing = True
                self.size_entry.set_text(v)
                self._syncing = False
                self.touch("font_size")
                self.update_preview()

        def size_typed(e):
            if self._syncing:
                return
            self._syncing = True
            _select_row(self.size_tv, e.get_text().strip())
            self._syncing = False
            self.touch("font_size")
            self.update_preview()
        self._syncing = False
        self.font_tv.get_selection().connect("changed", font_sel)
        self.font_entry.connect("changed", font_typed)
        self.fstyle_tv.get_selection().connect("changed", style_sel)
        self.size_tv.get_selection().connect("changed", size_sel)
        self.size_entry.connect("changed", size_typed)

        g2 = Gtk.Grid(row_spacing=4, column_spacing=10)
        g2.attach(Gtk.Label(label="Underline:", xalign=0), 0, 0, 1, 1)
        self.ul_combo = Gtk.ComboBoxText()
        for t, _ in UNDERLINES:
            self.ul_combo.append_text(t)
        uv = [v for _, v in UNDERLINES]
        self.ul_combo.set_active(uv.index(f["underline"]) if f["underline"] in uv else 0)
        self.ul_combo.connect("changed", lambda *_: (self.touch("underline"), self.update_preview()))
        g2.attach(self.ul_combo, 0, 1, 1, 1)
        g2.attach(Gtk.Label(label="Color:", xalign=0), 1, 0, 1, 1)
        self.font_color = f["color"]
        self.fc_btn = ColorButton(plain_color(f["color"]) if f["color"] is not None else None,
                                  self._font_color_changed, "Automatic", self)
        g2.attach(self.fc_btn, 1, 1, 1, 1)
        self.normal_chk = Gtk.CheckButton(label="Normal font")
        self.normal_chk.connect("toggled", self._normal_font)
        g2.attach(self.normal_chk, 2, 1, 1, 1)
        eff = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        self.strike_chk = Gtk.CheckButton(label="Strikethrough")
        self.strike_chk.set_active(f["strike"])
        self.sup_chk = Gtk.CheckButton(label="Superscript")
        self.sup_chk.set_active(f["vert"] == "superscript")
        self.sub_chk = Gtk.CheckButton(label="Subscript")
        self.sub_chk.set_active(f["vert"] == "subscript")
        self.strike_chk.connect("toggled", lambda *_: (self.touch("strike"), self.update_preview()))
        self.sup_chk.connect("toggled", lambda b: (self.touch("vert"), b.get_active() and self.sub_chk.set_active(False),
                                                   self.update_preview()))
        self.sub_chk.connect("toggled", lambda b: (self.touch("vert"), b.get_active() and self.sup_chk.set_active(False),
                                                   self.update_preview()))
        for w in (self.strike_chk, self.sup_chk, self.sub_chk):
            eff.pack_start(w, False, False, 0)
        self.preview = Gtk.DrawingArea()
        self.preview.set_size_request(240, 70)
        self.preview.connect("draw", self._draw_preview)
        bottom = Gtk.Box(spacing=12)
        bottom.pack_start(_frame("Effects", eff), False, False, 0)
        bottom.pack_start(_frame("Preview", self.preview), True, True, 0)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.pack_start(g, False, False, 0)
        box.pack_start(g2, False, False, 0)
        box.pack_start(bottom, False, False, 0)
        return box

    def _prefix_select(self, tv, text):
        t = text.lower()
        for i, row in enumerate(tv.get_model()):
            if row[0].lower().startswith(t) and t:
                tv.get_selection().select_path(Gtk.TreePath(i))
                tv.scroll_to_cell(Gtk.TreePath(i), None, False, 0, 0)
                return True
        return False

    def _font_color_changed(self, c):
        self.font_color = c
        self.touch("font_color")
        self.update_preview()

    def _normal_font(self, b):
        if not b.get_active():
            return
        self.font_entry.set_text("Calibri")
        self.fstyle_entry.set_text("Regular")
        _select_row(self.fstyle_tv, "Regular")
        self.size_entry.set_text("11")
        self.ul_combo.set_active(0)
        self.fc_btn.set_color(None)
        self.font_color = None
        for c in (self.strike_chk, self.sup_chk, self.sub_chk):
            c.set_active(False)
        self.touched |= {"font_name", "font_style", "font_size", "underline", "font_color", "strike", "vert"}
        self.update_preview()

    def update_preview(self):
        if hasattr(self, "preview"):
            self.preview.queue_draw()

    def _draw_preview(self, w, cr):
        a = w.get_allocation()
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        cr.set_source_rgb(0.75, 0.75, 0.75)
        cr.set_line_width(1)
        cr.rectangle(0.5, 0.5, a.width - 1, a.height - 1)
        cr.stroke()
        lay = PangoCairo.create_layout(cr)
        fd = Pango.FontDescription()
        fd.set_family(self.font_entry.get_text() or "Calibri")
        st = self.fstyle_entry.get_text()
        fd.set_weight(Pango.Weight.BOLD if "Bold" in st else Pango.Weight.NORMAL)
        fd.set_style(Pango.Style.ITALIC if "Italic" in st else Pango.Style.NORMAL)
        try:
            size = float(self.size_entry.get_text())
        except ValueError:
            size = 11
        fd.set_size(int(min(size, 36) * Pango.SCALE))
        lay.set_font_description(fd)
        attrs = Pango.AttrList()
        ul = UNDERLINES[max(0, self.ul_combo.get_active())][1]
        if ul:
            attrs.insert(Pango.attr_underline_new(Pango.Underline.DOUBLE if "double" in ul else Pango.Underline.SINGLE))
        if self.strike_chk.get_active():
            attrs.insert(Pango.attr_strikethrough_new(True))
        lay.set_attributes(attrs)
        lay.set_text(self.font_entry.get_text() or "AaBbCcYyZz", -1)
        tw, th = lay.get_pixel_size()
        cr.set_source_rgb(*rgbf(plain_color(self.font_color, 0) if self.font_color is not None else 0))
        y = (a.height - th) / 2 + (-6 if self.sup_chk.get_active() else 6 if self.sub_chk.get_active() else 0)
        cr.move_to(max(4, (a.width - tw) / 2), y)
        PangoCairo.show_layout(cr, lay)

    # ================================================================ Border
    def build_border(self):
        b = self.spec["border"]
        self.border_state = {k: b.get(k) for k in ("left", "right", "top", "bottom")}
        if b.get("diagonal"):
            if b.get("diagonalUp"):
                self.border_state["diagUp"] = b["diagonal"]
            if b.get("diagonalDown"):
                self.border_state["diagDown"] = b["diagonal"]
        self.border_touched = set()
        self.pen_style = "thin"
        self.pen_color = None
        outer = Gtk.Box(spacing=14)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        left.pack_start(Gtk.Label(label="Style:", xalign=0), False, False, 0)
        self.style_picker = LineStylePicker(self.pen_style, self._pen_style)
        fr = Gtk.Frame()
        fr.add(self.style_picker)
        left.pack_start(fr, False, False, 0)
        left.pack_start(Gtk.Label(label="Color:", xalign=0), False, False, 0)
        self.pen_btn = ColorButton(None, self._pen_color, "Automatic", self)
        left.pack_start(self.pen_btn, False, False, 0)
        outer.pack_start(_frame("Line", left), False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        presets = Gtk.Box(spacing=18)
        for name, key in (("None", "none"), ("Outline", "outline"), ("Inside", "inside")):
            col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
            bt = Gtk.Button()
            bt.add(self._preset_icon(key))
            bt.connect("clicked", lambda _b, k=key: self.border_preset(k))
            if key == "inside" and not (self.multi_rows or self.multi_cols):
                bt.set_sensitive(False)
            col.pack_start(bt, False, False, 0)
            col.pack_start(Gtk.Label(label=name), False, False, 0)
            presets.pack_start(col, False, False, 0)
        right.pack_start(_frame("Presets", presets), False, False, 0)
        g = Gtk.Grid(row_spacing=4, column_spacing=4)
        self.preview_b = BorderPreview(self)
        g.attach(self.preview_b, 1, 0, 3, 3)
        self.edge_btns = {}
        for key, x, y in (("top", 0, 0), ("insideH", 0, 1), ("bottom", 0, 2), ("diagUp", 0, 3),
                          ("left", 1, 3), ("insideV", 2, 3), ("right", 3, 3), ("diagDown", 4, 3)):
            bt = Gtk.ToggleButton()
            bt.add(self._edge_icon(key))
            bt.set_tooltip_text({"insideH": "Inside horizontal", "insideV": "Inside vertical",
                                 "diagUp": "Diagonal up", "diagDown": "Diagonal down"}.get(key, key.capitalize()))
            bt.set_active(bool(self.border_state.get(key)))
            bt.connect("toggled", lambda b, k=key: None if self._syncing_edges else self.toggle_edge(k, b.get_active()))
            if key == "insideH" and not self.multi_rows or key == "insideV" and not self.multi_cols:
                bt.set_sensitive(False)
            g.attach(bt, x, y, 1, 1)
            self.edge_btns[key] = bt
        self._syncing_edges = False
        right.pack_start(_frame("Border", g), False, False, 0)
        note = Gtk.Label(label="The selected border style can be applied by clicking the presets, preview "
                               "diagram or the buttons above.", xalign=0)
        note.set_line_wrap(True)
        note.set_max_width_chars(40)
        right.pack_start(note, False, False, 0)
        outer.pack_start(right, True, True, 0)
        return outer

    def _icon_area(self, draw):
        da = Gtk.DrawingArea()
        da.set_size_request(26, 26)
        da.connect("draw", lambda w, cr: draw(cr, w.get_allocated_width(), w.get_allocated_height()))
        return da

    def _preset_icon(self, key):
        def draw(cr, w, h):
            cr.set_source_rgb(0.55, 0.55, 0.55)
            cr.set_line_width(1)
            cr.rectangle(3.5, 3.5, w - 7, h - 7)
            cr.stroke()
            cr.set_source_rgb(0, 0, 0)
            cr.set_line_width(2)
            if key == "outline":
                cr.rectangle(4, 4, w - 8, h - 8)
                cr.stroke()
            elif key == "inside":
                cr.move_to(w / 2, 4)
                cr.line_to(w / 2, h - 4)
                cr.move_to(4, h / 2)
                cr.line_to(w - 4, h / 2)
                cr.stroke()
        return self._icon_area(draw)

    def _edge_icon(self, key):
        def draw(cr, w, h):
            cr.set_source_rgb(0.7, 0.7, 0.7)
            cr.set_line_width(1)
            cr.rectangle(4.5, 4.5, w - 9, h - 9)
            cr.stroke()
            cr.set_source_rgb(0, 0, 0)
            cr.set_line_width(2)
            x0, y0, x1, y1 = 4, 4, w - 4, h - 4
            seg = {"top": (x0, y0, x1, y0), "bottom": (x0, y1, x1, y1), "left": (x0, y0, x0, y1),
                   "right": (x1, y0, x1, y1), "insideH": (x0, h / 2, x1, h / 2), "insideV": (w / 2, y0, w / 2, y1),
                   "diagUp": (x0, y1, x1, y0), "diagDown": (x0, y0, x1, y1)}[key]
            cr.move_to(seg[0], seg[1])
            cr.line_to(seg[2], seg[3])
            cr.stroke()
        return self._icon_area(draw)

    def _pen_style(self, s):
        self.pen_style = s

    def _pen_color(self, c):
        self.pen_color = c

    def _pen(self):
        if self.pen_style == "none":
            return None
        return (self.pen_style, self.pen_color)

    def toggle_edge(self, key, on=None):
        cur = self.border_state.get(key)
        pen = self._pen()
        if on is None:
            # clicking an edge that already has this exact line removes it, otherwise draws it
            on = not (cur and pen and tuple(cur) == tuple(pen))
        self.border_state[key] = pen if on else None
        self.border_touched.add(key)
        self.touch("border")
        self._sync_edges()

    def border_preset(self, key):
        pen = self._pen()
        if key == "none":
            for k in ("left", "right", "top", "bottom", "insideH", "insideV", "diagUp", "diagDown"):
                self.border_state[k] = None
                self.border_touched.add(k)
        elif key == "outline":
            for k in ("left", "right", "top", "bottom"):
                self.border_state[k] = pen
                self.border_touched.add(k)
        elif key == "inside":
            for k in ("insideH", "insideV"):
                if (k == "insideH" and self.multi_rows) or (k == "insideV" and self.multi_cols):
                    self.border_state[k] = pen
                    self.border_touched.add(k)
        self.touch("border")
        self._sync_edges()

    def _sync_edges(self):
        self._syncing_edges = True
        for k, bt in self.edge_btns.items():
            bt.set_active(bool(self.border_state.get(k)))
        self._syncing_edges = False
        self.preview_b.queue_draw()

    # ================================================================ Fill
    def build_fill(self):
        fl = self.spec["fill"]
        self.fill_bg = None
        self.fill_pattern = None
        self.fill_pcolor = None
        if fl["pattern"] == "solid":
            self.fill_bg = plain_color(fl["fg"], 0xFFFFFF) if fl["fg"] is not None else None
        elif fl["pattern"] not in ("none", None):
            self.fill_pattern = fl["pattern"]
            self.fill_bg = plain_color(fl["bg"], 0xFFFFFF) if fl["bg"] is not None else None
            self.fill_pcolor = plain_color(fl["fg"], 0) if fl["fg"] is not None else None
        outer = Gtk.Box(spacing=16)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        left.pack_start(Gtk.Label(label="Background Color:", xalign=0), False, False, 0)
        left.pack_start(palette_grid(self._bg_pick, "No Color", self._bg_more), False, False, 0)
        outer.pack_start(left, False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        right.pack_start(Gtk.Label(label="Pattern Color:", xalign=0), False, False, 0)
        self.pcolor_btn = ColorButton(self.fill_pcolor, self._pcolor, "Automatic", self)
        right.pack_start(self.pcolor_btn, False, False, 0)
        right.pack_start(Gtk.Label(label="Pattern Style:", xalign=0), False, False, 0)
        self.pattern_combo = Gtk.ComboBoxText()
        self.pattern_combo.append_text("(None)")
        for t, _ in PATTERNS[1:]:
            self.pattern_combo.append_text(t)
        pv = [None] + [v for _, v in PATTERNS[1:]]
        self.pattern_combo.set_active(pv.index(self.fill_pattern) if self.fill_pattern in pv else 0)
        self.pattern_combo.connect("changed", self._pattern_changed)
        right.pack_start(self.pattern_combo, False, False, 0)
        right.pack_start(Gtk.Label(label="Sample", xalign=0), False, False, 8)
        self.fill_sample = Gtk.DrawingArea()
        self.fill_sample.set_size_request(220, 60)
        self.fill_sample.connect("draw", self._draw_fill)
        right.pack_start(self.fill_sample, False, False, 0)
        outer.pack_start(right, True, True, 0)
        return outer

    def _bg_pick(self, c):
        self.fill_bg = c
        if c is None:
            self.fill_pattern = None
            self.pattern_combo.set_active(0)
        self.touch("fill")
        self.fill_sample.queue_draw()

    def _bg_more(self):
        d = Gtk.ColorChooserDialog(title="Colors", transient_for=self)
        d.set_use_alpha(False)
        if d.run() == Gtk.ResponseType.OK:
            c = d.get_rgba()
            self._bg_pick((round(c.red * 255) << 16) | (round(c.green * 255) << 8) | round(c.blue * 255))
        d.destroy()

    def _pcolor(self, c):
        self.fill_pcolor = c
        self.touch("fill")
        self.fill_sample.queue_draw()

    def _pattern_changed(self, c):
        i = c.get_active()
        self.fill_pattern = None if i <= 0 else PATTERNS[i][1]
        self.touch("fill")
        self.fill_sample.queue_draw()

    def fill_value(self):
        if self.fill_pattern:
            return {"pattern": self.fill_pattern, "fg": self.fill_pcolor if self.fill_pcolor is not None else 0,
                    "bg": self.fill_bg}
        if self.fill_bg is None:
            return None
        return {"pattern": "solid", "fg": self.fill_bg}

    def _draw_fill(self, w, cr):
        a = w.get_allocation()
        bg = self.fill_bg if self.fill_bg is not None else 0xFFFFFF
        cr.set_source_rgb(*rgbf(bg))
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        if self.fill_pattern:
            cr.set_source_rgb(*rgbf(self.fill_pcolor or 0))
            dens = {"darkGray": 0.75, "mediumGray": 0.5, "lightGray": 0.25, "gray125": 0.125, "gray0625": 0.0625}
            p = self.fill_pattern
            if p in dens:
                step = max(1, int(round(1 / dens[p])))
                for y in range(0, a.height, 2):
                    for x in range((y // 2) % 2, a.width, step):
                        cr.rectangle(x, y, 1, 1)
                cr.fill()
            else:
                lw = 2 if p.startswith("dark") else 1
                cr.set_line_width(lw)
                gap = 4
                if "Horizontal" in p or "Grid" in p:
                    for y in range(0, a.height, gap):
                        cr.move_to(0, y + 0.5)
                        cr.line_to(a.width, y + 0.5)
                if "Vertical" in p or "Grid" in p:
                    for x in range(0, a.width, gap):
                        cr.move_to(x + 0.5, 0)
                        cr.line_to(x + 0.5, a.height)
                if "Down" in p or "Trellis" in p:
                    for x in range(-a.height, a.width, gap):
                        cr.move_to(x, 0)
                        cr.line_to(x + a.height, a.height)
                if "Up" in p or "Trellis" in p:
                    for x in range(0, a.width + a.height, gap):
                        cr.move_to(x, 0)
                        cr.line_to(x - a.height, a.height)
                cr.stroke()
        cr.set_source_rgb(0.7, 0.7, 0.7)
        cr.set_line_width(1)
        cr.rectangle(0.5, 0.5, a.width - 1, a.height - 1)
        cr.stroke()

    # ================================================================ Protection
    def build_protection(self):
        p = self.spec["protection"]
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self.locked_chk = Gtk.CheckButton(label="Locked")
        self.locked_chk.set_active(p["locked"])
        self.hidden_chk = Gtk.CheckButton(label="Hidden")
        self.hidden_chk.set_active(p["hidden"])
        self.locked_chk.connect("toggled", lambda *_: self.touch("locked"))
        self.hidden_chk.connect("toggled", lambda *_: self.touch("hidden"))
        box.pack_start(self.locked_chk, False, False, 0)
        box.pack_start(self.hidden_chk, False, False, 0)
        note = Gtk.Label(label="Locking cells or hiding formulas has no effect until you protect the worksheet "
                               "(Review tab, Protect group, Protect Sheet button).", xalign=0)
        note.set_line_wrap(True)
        note.set_max_width_chars(60)
        box.pack_start(note, False, False, 8)
        return box

    # ================================================================ result
    def result(self):
        t = self.touched
        ch = {}
        if "numfmt" in t:
            ch["numfmt"] = self.current_code()
        font = {}
        if "font_name" in t and self.font_entry.get_text().strip():
            font["name"] = self.font_entry.get_text().strip()
        if "font_style" in t:
            s = self.fstyle_entry.get_text()
            font["bold"] = "Bold" in s
            font["italic"] = "Italic" in s
        if "font_size" in t:
            try:
                v = float(self.size_entry.get_text())
                if 1 <= v <= 409:
                    font["size"] = round(v * 2) / 2
            except ValueError:
                pass
        if "underline" in t:
            font["underline"] = UNDERLINES[max(0, self.ul_combo.get_active())][1]
        if "font_color" in t:
            font["color"] = self.font_color
        if "strike" in t:
            font["strike"] = self.strike_chk.get_active()
        if "vert" in t:
            font["vert"] = "superscript" if self.sup_chk.get_active() else "subscript" if self.sub_chk.get_active() else None
        if font:
            ch["font"] = font
        al = {}
        if "horizontal" in t:
            al["horizontal"] = H_ALIGN[self.h_combo.get_active()][1]
        if "vertical" in t:
            al["vertical"] = V_ALIGN[self.v_combo.get_active()][1]
        if "indent" in t or "horizontal" in t:
            if H_ALIGN[self.h_combo.get_active()][1] in ("left", "right", "distributed"):
                al["indent"] = int(self.indent_spin.get_value())
            elif "horizontal" in t:
                al["indent"] = 0
        if "wrap" in t:
            al["wrap"] = self.wrap_chk.get_active()
        if "shrink" in t:
            al["shrink"] = self.shrink_chk.get_active()
        if "reading" in t:
            al["reading"] = self.dir_combo.get_active()
        if "rotation" in t:
            if self.vert_btn.get_active():
                al["rotation"] = 255
            else:
                d = int(self.deg_spin.get_value())
                al["rotation"] = d if d >= 0 else 90 - d
        if al:
            ch["align"] = al
        if "fill" in t:
            ch["fill"] = self.fill_value()
        prot = {}
        if "locked" in t:
            prot["locked"] = self.locked_chk.get_active()
        if "hidden" in t:
            prot["hidden"] = self.hidden_chk.get_active()
        if prot:
            ch["protection"] = prot
        if "border" in t and self.border_touched:
            ch["border_layout"] = {k: self.border_state.get(k) for k in self.border_touched}
        if "merge" in t:
            ch["merge"] = self.merge_chk.get_active()
        return ch

    def run_dialog(self):
        r = self.run()
        if r == Gtk.ResponseType.OK:
            for sp in (self.indent_spin, self.deg_spin):
                sp.update()
            out = self.result()
        else:
            out = None
        self.destroy()
        return out


# ---------------------------------------------------------------- small pickers

def pick_line_style(win, cur):
    d = Gtk.Dialog(title="Line Style", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    chosen = [cur]
    p = LineStylePicker(cur, lambda s: chosen.__setitem__(0, s))
    d.get_content_area().set_border_width(10)
    d.get_content_area().pack_start(p, False, False, 0)
    d.show_all()
    r = d.run()
    d.destroy()
    if r != Gtk.ResponseType.OK or chosen[0] == "none":
        return None
    return chosen[0]


def pick_cell_style(win):
    """The Cell Styles gallery: each style is a set of formats applied at once."""
    d = Gtk.Dialog(title="Cell Styles", transient_for=win, modal=True)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_resizable(False)
    box = d.get_content_area()
    box.set_border_width(10)
    box.set_spacing(6)
    picked = [None]
    for heading, items in CELL_STYLES:
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>%s</b>" % GLib.markup_escape_text(heading))
        box.pack_start(lab, False, False, 2)
        flow = Gtk.FlowBox()
        flow.set_max_children_per_line(6)
        flow.set_selection_mode(Gtk.SelectionMode.NONE)
        for name, change in items:
            b = Gtk.Button()
            b.add(_style_chip(name, change))
            b.set_tooltip_text(name)

            def go(_b, n=name, c=change):
                picked[0] = (n, c)
                d.response(Gtk.ResponseType.OK)
            b.connect("clicked", go)
            flow.add(b)
        box.pack_start(flow, False, False, 0)
    d.show_all()
    d.run()
    d.destroy()
    return picked[0]


def _style_chip(name, change):
    da = Gtk.DrawingArea()
    da.set_size_request(104, 24)

    def draw(w, cr):
        a = w.get_allocation()
        fill = (change.get("fill") or {}).get("fg")
        cr.set_source_rgb(*rgbf(fill if fill is not None else 0xFFFFFF))
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        f = change.get("font", {})
        lay = PangoCairo.create_layout(cr)
        fd = Pango.FontDescription("Sans 8")
        if f.get("bold"):
            fd.set_weight(Pango.Weight.BOLD)
        if f.get("italic"):
            fd.set_style(Pango.Style.ITALIC)
        lay.set_font_description(fd)
        lay.set_text(name, -1)
        cr.set_source_rgb(*rgbf(f.get("color", 0)))
        cr.move_to(4, (a.height - lay.get_pixel_size()[1]) / 2)
        PangoCairo.show_layout(cr, lay)
        for side, pen in (change.get("border") or {}).items():
            if not pen:
                continue
            cr.set_source_rgb(*rgbf(pen[1] or 0))
            cr.set_line_width(2 if pen[0] in ("medium", "thick", "double") else 1)
            y = {"top": 1, "bottom": a.height - 1}.get(side)
            if y is not None:
                cr.move_to(0, y)
                cr.line_to(a.width, y)
            else:
                x = 1 if side == "left" else a.width - 1
                cr.move_to(x, 0)
                cr.line_to(x, a.height)
            cr.stroke()
    da.connect("draw", draw)
    return da
