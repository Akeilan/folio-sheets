"""The ribbon's tabs, groups and controls, laid out as the original's
desktop ribbon (cloud, sign-in and AI commands are left out)."""
import colorsys

from gi.repository import GLib, Gtk, Pango, PangoCairo

from . import icons
from .ribbon import (ComboControl, Group, MenuButton, RibbonButton, SplitButton, Tab,
                     menu_popover)

FONT_SIZES = ["8", "9", "10", "11", "12", "14", "16", "18", "20", "22", "24", "26", "28", "36",
              "48", "72"]
NUMBER_FORMATS = [("General", "General"), ("Number", "0.00"), ("Currency", '"$"#,##0.00'),
                  ("Accounting", '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)'),
                  ("Short Date", "m/d/yyyy"), ("Long Date", "[$-F800]dddd, mmmm dd, yyyy"),
                  ("Time", "[$-F400]h:mm:ss AM/PM"), ("Percentage", "0.00%"), ("Fraction", "# ?/?"),
                  ("Scientific", "0.00E+00"), ("Text", "@")]

# the theme colours of a new workbook (Office 2013–2022 default theme)
THEME_COLORS = [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5,
                0xFFC000, 0x5B9BD5, 0x70AD47]
STANDARD_COLORS = [0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050, 0x00B0F0,
                   0x0070C0, 0x002060, 0x7030A0]


def _remember(win, attr, w):
    setattr(win, attr, w)
    return w


def _color_button(win, btn):
    if not hasattr(win, "color_buttons"):
        win.color_buttons = []
    win.color_buttons.append(btn)
    return btn

def tint(rgb, t):
    r, g, b = ((rgb >> 16) & 255) / 255, ((rgb >> 8) & 255) / 255, (rgb & 255) / 255
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    l = l * (1 + t) if t < 0 else l * (1 - t) + t
    r, g, b = colorsys.hls_to_rgb(h, max(0, min(1, l)), s)
    return (round(r * 255) << 16) | (round(g * 255) << 8) | round(b * 255)


def palette_tints(c):
    """The tints of the five rows under a theme colour (lighter and darker), as the desktop
    spreadsheet picks them for how light the colour is."""
    if c == 0xFFFFFF:
        return (-0.05, -0.15, -0.25, -0.35, -0.5)
    if c == 0x000000:
        return (0.5, 0.35, 0.25, 0.15, 0.05)
    l = colorsys.rgb_to_hls(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255)[1]
    if l < 0.2:
        return (0.9, 0.75, 0.5, 0.25, 0.1)
    if l > 0.8:
        return (-0.1, -0.25, -0.5, -0.75, -0.9)
    return (0.8, 0.6, 0.4, -0.25, -0.5)


def palette_rows(c):
    """The five tint/shade rows under each theme colour."""
    return [tint(c, t) for t in palette_tints(c)]


# the theme of the workbook being worked on: the palette's theme columns follow it
ACTIVE_THEME = list(THEME_COLORS) + [0x0563C1, 0x954F72]
THEME_BIT = 0x80000000
TINT_NAMES = {0.8: "Lighter 80%", 0.6: "Lighter 60%", 0.4: "Lighter 40%", 0.25: "Lighter 25%", 0.35: "Lighter 35%",
              0.15: "Lighter 15%", 0.05: "Lighter 5%", 0.5: "Lighter 50%", 0.9: "Lighter 90%", 0.75: "Lighter 75%",
              0.1: "Lighter 10%", -0.25: "Darker 25%", -0.5: "Darker 50%", -0.05: "Darker 5%", -0.15: "Darker 15%",
              -0.35: "Darker 35%", -0.1: "Darker 10%", -0.75: "Darker 75%", -0.9: "Darker 90%"}
SLOT_NAMES = ["Background 1", "Text 1", "Background 2", "Text 2", "Accent 1", "Accent 2", "Accent 3", "Accent 4",
              "Accent 5", "Accent 6"]


def set_active_theme(colors):
    """The palette shows this theme (light 1, dark 1, light 2, dark 2, accents, hyperlinks)."""
    ACTIVE_THEME[:] = list(colors)[:12]
    THEME_COLORS[:] = ACTIVE_THEME[:10]


def theme_ref(slot, t=0.0):
    """A colour kept as a theme reference (it follows the theme): slot and tint."""
    return THEME_BIT | ((slot & 15) << 24) | ((int(round(max(-1, min(1, t)) * 10000)) + 10000) & 0xFFFF)


def split_ref(c):
    if isinstance(c, int) and c & THEME_BIT:
        return (c >> 24) & 15, ((c & 0xFFFF) - 10000) / 10000.0
    return None


def resolve(c):
    """A colour as it shows: a theme reference in the active theme's colours."""
    r = split_ref(c)
    if r is None:
        return c
    slot, t = r
    base = ACTIVE_THEME[slot] if slot < len(ACTIVE_THEME) else 0
    return tint(base, t) if t else base


def color_name(c):
    r = split_ref(c)
    if r is None:
        return "#%06X" % c
    slot, t = r
    name = SLOT_NAMES[slot] if slot < len(SLOT_NAMES) else "Hyperlink"
    near = [v for k, v in TINT_NAMES.items() if abs(k - t) < 0.001]
    return name + (", " + near[0] if near else "")


def color_palette(on_pick, none_label=None, more=None, popref=None):
    """The colour picker: the theme's colours (kept as theme references, so they follow the theme),
    the standard colours, More Colors."""
    def swatch(c):
        b = Gtk.Button()
        b.get_style_context().add_class("folio-swatch")
        css = Gtk.CssProvider()
        css.load_from_data(("button { background: #%06x; }" % resolve(c)).encode())
        b.get_style_context().add_provider(css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
        b.set_tooltip_text(color_name(c))
        b.value = c
        b.connect("clicked", lambda *_: pick(c))
        return b

    def pick(c):
        if popref and popref[0]:
            popref[0].popdown()
        on_pick(c)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(8)
    if none_label:
        nb = Gtk.Button(label=none_label)
        nb.set_relief(Gtk.ReliefStyle.NONE)
        nb.connect("clicked", lambda *_: pick(None))
        box.pack_start(nb, False, False, 0)
    box.pack_start(Gtk.Label(label="Theme Colors", xalign=0), False, False, 0)
    grid = Gtk.Grid(column_spacing=3, row_spacing=1)
    grid.set_name("theme-colors")
    for col, c in enumerate(ACTIVE_THEME[:10]):
        grid.attach(swatch(theme_ref(col)), col, 0, 1, 1)
        for row, t in enumerate(palette_tints(c), start=1):
            grid.attach(swatch(theme_ref(col, t)), col, row + 0, 1, 1)
    box.pack_start(grid, False, False, 0)
    box.pack_start(Gtk.Label(label="Standard Colors", xalign=0), False, False, 4)
    std = Gtk.Box(spacing=3)
    for c in STANDARD_COLORS:
        std.pack_start(swatch(c), False, False, 0)
    box.pack_start(std, False, False, 0)
    if more:
        mb = Gtk.Button(label="More Colors...")
        mb.set_relief(Gtk.ReliefStyle.NONE)
        mb.connect("clicked", lambda *_: (popref[0].popdown() if popref and popref[0] else None, more()))
        box.pack_start(mb, False, False, 0)
    return box


def system_fonts():
    fm = PangoCairo.FontMap.get_default()
    return sorted({f.get_name() for f in fm.list_families()}, key=str.lower)


def _fn_list(win, names):
    """A scrolling column of function buttons for a category menu."""
    from . import fncatalog
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    for n in names:
        b = Gtk.Button(label=n)
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.get_style_context().add_class("folio-fnitem")
        b.get_child().set_xalign(0)
        b.set_tooltip_markup("<b>%s</b>\n%s" % (GLib.markup_escape_text(fncatalog.syntax(n)),
                                                  GLib.markup_escape_text(fncatalog.describe(n))))

        def go(btn, n=n):
            pop = btn.get_ancestor(Gtk.Popover)
            if pop:
                pop.popdown()
            GLib.idle_add(lambda: (win.run("fn-insert:" + n), False)[1])
        b.connect("clicked", go)
        box.pack_start(b, False, False, 0)
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_propagate_natural_height(True)
    sw.set_max_content_height(440)
    sw.set_min_content_width(190)
    sw.add(box)
    sw.show_all()
    return sw


def function_menu(win, cat):
    from . import fncatalog
    if cat == "recent":
        from .fn_dialogs import recent_functions
        names = recent_functions(win)
    else:
        names = fncatalog.in_category(cat)
    return [_fn_list(win, names), None, ("Insert Function...", "insert-function", "fx-insert-sm")]


def more_functions_menu(win):
    """Statistical, Engineering, … : a category column and its functions."""
    from . import fncatalog
    cats = ("Statistical", "Engineering", "Information", "Compatibility", "Database")
    stack = Gtk.Stack()
    stack.set_transition_type(Gtk.StackTransitionType.NONE)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    for cat in cats:
        stack.add_named(_fn_list(win, fncatalog.in_category(cat)), cat)
        b = Gtk.Button(label=cat + "  ›")
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.get_child().set_xalign(0)
        b.connect("clicked", lambda _b, c=cat: stack.set_visible_child_name(c))
        b.connect("enter-notify-event", lambda _b, _e, c=cat: (stack.set_visible_child_name(c), False)[1])
        side.pack_start(b, False, False, 0)
    row = Gtk.Box(spacing=0)
    row.pack_start(side, False, False, 0)
    row.pack_start(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL), False, False, 2)
    row.pack_start(stack, True, True, 0)
    row.show_all()
    return [row, None, ("Insert Function...", "insert-function", "fx-insert-sm")]


class Cmd:
    """A menu callback that remembers its command (the audit reads it)."""
    def __init__(self, win, cmd):
        self.win = win
        self.cmd = cmd

    def __call__(self, *_):
        self.win.run(self.cmd)


class Builder:
    """Shorthands for building groups."""
    def __init__(self, win):
        self.win = win
        self.r = win.ribbon

    def run(self, cmd):
        return Cmd(self.win, cmd)

    def menu(self, items):
        out = []
        for it in items:
            if it is None or isinstance(it, Gtk.Widget) or (isinstance(it, tuple) and it[0] == "#"):
                out.append(it)
            else:
                label, cmd = it[0], it[1]
                if isinstance(cmd, list):
                    out.append((label, self.menu(cmd)))     # a submenu
                    continue
                icon = it[2] if len(it) > 2 else None
                out.append((label, self.run(cmd), icon) if icon else (label, self.run(cmd)))
        return out

    def keyed(self, key, btn):
        """A menu button the command `key` opens (KeyTips, search, the command walk)."""
        self.win.menu_keys[key] = btn
        return btn

    def big(self, label, icon, cmd, keytip, menu=None, tip=None, key=None):
        if key is not None:
            return self.keyed(key, self.big(label, icon, cmd, keytip, menu=menu, tip=tip))
        if menu is not None:
            if cmd is None:
                return MenuButton(self.r, label, icon, self.menu(menu) if isinstance(menu, list) else menu,
                                  size="large", keytip=keytip, tooltip=tip or label)
            return SplitButton(self.r, label, icon, cmd, self.menu(menu) if isinstance(menu, list) else menu,
                               size="large", keytip=keytip, tooltip=tip or label)
        return RibbonButton(self.r, label, icon, cmd, size="large", keytip=keytip, tooltip=tip or label)

    def small(self, label, icon, cmd, keytip, menu=None, tip=None, state=None, key=None):
        if key is not None:
            return self.keyed(key, self.small(label, icon, cmd, keytip, menu=menu, tip=tip, state=state))
        if menu is not None:
            if cmd is None:
                return MenuButton(self.r, label, icon, self.menu(menu) if isinstance(menu, list) else menu,
                                  size="small", keytip=keytip, tooltip=tip or label)
            return SplitButton(self.r, label, icon, cmd, self.menu(menu) if isinstance(menu, list) else menu,
                               size="small", keytip=keytip, tooltip=tip or label, state=state)
        return RibbonButton(self.r, label, icon, cmd, size="small", keytip=keytip, tooltip=tip or label, state=state)

    def icon(self, icon, cmd, keytip, tip, menu=None, state=None):
        if menu is not None:
            b = SplitButton(self.r, None, icon, cmd, self.menu(menu) if isinstance(menu, list) else menu,
                            size="icon", keytip=keytip, tooltip=tip, state=state)
            return b
        return RibbonButton(self.r, None, icon, cmd, size="icon", keytip=keytip, tooltip=tip, state=state)

    def group(self, tab, caption, columns, launcher=None, launcher_keytip=None):
        g = Group(caption, columns, launcher=launcher, ribbon=self.r, launcher_keytip=launcher_keytip)
        tab.add(g)
        return g


def _menu_pen(tool):
    from .ink_ui import WIDTHS, COLORS
    items = [("#", "Thickness")] + [("%g mm" % mm, "ink-width:%s:%g" % (tool, mm)) for mm in WIDTHS[tool]]
    items += [None, ("#", "Colors")] + [(name, "ink-color:%s:%06X" % (tool, c)) for name, c in COLORS]
    return items


def _objects_ui():
    from . import objects_ui
    return objects_ui


def _themes_ui():
    from . import themes_ui
    return themes_ui


def _screenshot_menu(win):
    from . import screenshot_ui
    return screenshot_ui.menu(win)


def _equation_menu(win):
    from . import equation_ui
    return equation_ui.equation_menu(win)


def build_ribbon(win):
    from . import shapes_ui as _shapes_ui
    B = Builder(win)
    r = win.ribbon
    run = B.run

    # ================================================================ Home
    t = r.building(Tab("Home", "H"))
    paste_menu = [("#", "Paste"), ("Paste", "paste", "r-paste-sm"), ("Formulas", "paste-formulas"),
                  ("Keep Source Column Widths", "paste-keep-widths"), ("No Borders", "paste-no-borders"),
                  ("Transpose", "paste-transpose"), ("#", "Paste Values"), ("Values", "paste-values"),
                  ("#", "Other Paste Options"), ("Formatting", "paste-formatting"), ("Paste Link", "paste-link"),
                  ("Picture", "paste-picture"), None, ("Paste Special...", "paste-special")]
    B.group(t, "Clipboard", [
        B.big("Paste", "r-paste", "paste", "V", menu=paste_menu),
        [B.small("Cut", "r-cut-sm", "cut", "X"),
         B.small("Copy", "r-copy-sm", "copy", "C", menu=[("Copy", "copy", "r-copy-sm"), ("Copy as Picture...", "copy-picture")]),
         B.small("Format Painter", "r-format-painter-sm", "format-painter", "FP")],
    ], launcher="clipboard-pane", launcher_keytip="FO")

    fonts = system_fonts()
    win.font_combo = ComboControl(r, fonts, lambda v: win.run("font-name:" + v), 15, state="font-name",
                                  keytip="FF", tooltip="Font")
    win.size_combo = ComboControl(r, FONT_SIZES, lambda v: win.run("font-size:" + v), 3, state="font-size",
                                  keytip="FS", tooltip="Font Size")
    fill_ref, fc_ref = [None], [None]

    def fill_menu():
        pop = Gtk.Popover()
        fill_ref[0] = pop
        pop.add(color_palette(lambda c: win.run("fill-color:%s" % ("none" if c is None else "%06X" % c)),
                              none_label="No Fill", more=lambda: win.run("fill-color:more"), popref=fill_ref))
        return pop

    def fc_menu():
        pop = Gtk.Popover()
        fc_ref[0] = pop
        pop.add(color_palette(lambda c: win.run("font-color:%s" % ("auto" if c is None else "%06X" % c)),
                              none_label="Automatic", more=lambda: win.run("font-color:more"), popref=fc_ref))
        return pop

    border_menu = [("#", "Borders"), ("Bottom Border", "border-bottom"), ("Top Border", "border-top"),
                   ("Left Border", "border-left"), ("Right Border", "border-right"), None,
                   ("No Border", "border-none"), ("All Borders", "border-all"),
                   ("Outside Borders", "border-outside"), ("Thick Outside Borders", "border-thick-outside"), None,
                   ("Bottom Double Border", "border-bottom-double"), ("Thick Bottom Border", "border-bottom-thick"),
                   ("Top and Bottom Border", "border-top-bottom"),
                   ("Top and Thick Bottom Border", "border-top-thick-bottom"),
                   ("Top and Double Bottom Border", "border-top-double-bottom"), ("#", "Draw Borders"),
                   ("Draw Border", "border-draw"), ("Draw Border Grid", "border-draw-grid"),
                   ("Erase Border", "border-erase"), ("Line Color", "border-line-color"),
                   ("Line Style", "border-line-style"), None, ("More Borders...", "border-more")]
    B.group(t, "Font", [
        [[win.font_combo, win.size_combo,
          B.icon("r-grow-sm", "font-grow", "FG", "Increase Font Size (Ctrl+Shift+>)"),
          B.icon("r-shrink-sm", "font-shrink", "FK", "Decrease Font Size (Ctrl+Shift+<)")],
         [B.icon("r-bold-sm", "bold", "1", "Bold (Ctrl+B)", state="bold"),
          B.icon("r-italic-sm", "italic", "2", "Italic (Ctrl+I)", state="italic"),
          B.icon("r-underline-sm", "underline", "3", "Underline (Ctrl+U)", state="underline",
                 menu=[("Underline", "underline", "r-underline-sm"), ("Double Underline", "double-underline")]),
          _remember(win, "borders_button", B.icon("r-borders-sm", "border-last", "B", "Borders: Bottom Border",
                                                   menu=border_menu)),
          _color_button(win, SplitButton(r, None, "r-fill-color-sm", "fill-color:last", fill_menu, size="icon",
                                         keytip="H", tooltip="Fill Color")),
          _color_button(win, SplitButton(r, None, "r-font-color-sm", "font-color:last", fc_menu, size="icon",
                                         keytip="FC", tooltip="Font Color"))]],
    ], launcher="font-dialog", launcher_keytip="FN")

    orient_menu = [("Angle Counterclockwise", "orientation:45"), ("Angle Clockwise", "orientation:135"),
                   ("Vertical Text", "orientation:255"), ("Rotate Text Up", "orientation:90"),
                   ("Rotate Text Down", "orientation:180"), None,
                   ("Format Cell Alignment", "alignment-dialog")]
    merge_menu = [("Merge & Center", "merge-center"), ("Merge Across", "merge-across"),
                  ("Merge Cells", "merge-cells"), ("Unmerge Cells", "unmerge")]
    B.group(t, "Alignment", [
        [[B.icon("r-align-top-sm", "align-top", "AT", "Top Align", state="align-top"),
          B.icon("r-align-middle-sm", "align-middle", "AM", "Middle Align", state="align-middle"),
          B.icon("r-align-bottom-sm", "align-bottom", "AB", "Bottom Align", state="align-bottom"),
          B.icon("r-orientation-sm", None, "FQ", "Orientation", menu=orient_menu),
          B.small("Wrap Text", "r-wrap-text-sm", "wrap-text", "W", state="wrap-text")],
         [B.icon("r-align-left-sm", "align-left", "AL", "Align Left", state="align-left"),
          B.icon("r-align-center-sm", "align-center", "AC", "Center", state="align-center"),
          B.icon("r-align-right-sm", "align-right", "AR", "Align Right", state="align-right"),
          B.icon("r-indent-decrease-sm", "indent-decrease", "5", "Decrease Indent"),
          B.icon("r-indent-increase-sm", "indent-increase", "6", "Increase Indent"),
          B.small("Merge & Center", "r-merge-center-sm", "merge-center", "M", menu=merge_menu,
                  state="merge-center")]],
    ], launcher="alignment-dialog", launcher_keytip="FA")

    win.numfmt_combo = ComboControl(r, [n for n, _ in NUMBER_FORMATS] + ["More Number Formats..."],
                                    lambda v: win.run("number-format:" + v), 12, state="number-format",
                                    keytip="N", tooltip="Number Format")
    acct_menu = [("English (United States)", "accounting:$"), ("English (United Kingdom)", "accounting:£"),
                 ("Euro (€ 123)", "accounting:€"), None, ("More Accounting Formats...", "number-dialog")]
    B.group(t, "Number", [
        [win.numfmt_combo,
         [B.icon("r-accounting-sm", "accounting", "AN", "Accounting Number Format", menu=acct_menu),
          B.icon("r-percent-sm", "percent", "P", "Percent Style (Ctrl+Shift+%)"),
          B.icon("r-comma-sm", "comma", "K", "Comma Style"),
          B.icon("r-decimal-increase-sm", "decimal-increase", "0", "Increase Decimal"),
          B.icon("r-decimal-decrease-sm", "decimal-decrease", "9", "Decrease Decimal")]],
    ], launcher="number-dialog", launcher_keytip="FM")

    def table_menu():
        from . import table_ui
        return table_ui.gallery(win, lambda n: win.run("format-as-table:" + n))

    def cf_menu():
        from . import cf_ui
        return cf_ui.popover(win)
    B.group(t, "Styles", [
        B.big("Conditional\nFormatting", "r-conditional-formatting", None, "L", menu=cf_menu),
        B.big("Format as\nTable", "r-format-as-table", None, "T", menu=table_menu),
        B.big("Cell\nStyles", "r-cell-styles", None, "J", menu=[("Cell styles gallery...", "cell-styles")]),
    ])
    format_menu = [("#", "Cell Size"), ("Row Height...", "row-height"), ("AutoFit Row Height", "autofit-rows"),
                   ("Column Width...", "column-width"), ("AutoFit Column Width", "autofit-columns"),
                   ("Default Width...", "default-width"), ("#", "Visibility"),
                   ("Hide Rows", "hide-rows"), ("Hide Columns", "hide-columns"), ("Hide Sheet", "hide-sheet"),
                   ("Unhide Rows", "unhide-rows"), ("Unhide Columns", "unhide-columns"),
                   ("Unhide Sheet...", "unhide-sheet"), ("#", "Organize Sheets"),
                   ("Rename Sheet", "rename-sheet"), ("Move or Copy Sheet...", "move-copy-sheet"),
                   ("Tab Color", "tab-color"), ("#", "Protection"), ("Protect Sheet...", "protect-sheet"),
                   ("Lock Cell", "lock-cell"), ("Format Cells...", "format-cells")]
    B.group(t, "Cells", [
        B.big("Insert", "r-insert-cells", "insert-cells", "I",
              menu=[("Insert Cells...", "insert-cells"), ("Insert Sheet Rows", "insert-rows"),
                    ("Insert Sheet Columns", "insert-columns"), ("Insert Sheet", "insert-sheet")]),
        B.big("Delete", "r-delete-cells", "delete-cells", "D",
              menu=[("Delete Cells...", "delete-cells"), ("Delete Sheet Rows", "delete-rows"),
                    ("Delete Sheet Columns", "delete-columns"), ("Delete Sheet", "delete-sheet")]),
        B.big("Format", "r-format-cells", None, "O", menu=format_menu),
    ])
    B.group(t, "Editing", [
        [B.small("AutoSum", "r-autosum-sm", "autosum", "U",
                 menu=[("Sum", "autosum"), ("Average", "autosum-average"), ("Count Numbers", "autosum-count"),
                       ("Max", "autosum-max"), ("Min", "autosum-min"), None,
                       ("More Functions...", "insert-function")]),
         B.small("Fill", "r-fill-sm", None, "FI",
                 menu=[("Down", "fill-down"), ("Right", "fill-right"), ("Up", "fill-up"), ("Left", "fill-left"),
                       None, ("Series...", "fill-series"), ("Justify", "fill-justify"),
                       ("Flash Fill", "flash-fill")]),
         B.small("Clear", "r-clear-sm", None, "E",
                 menu=[("Clear All", "clear-all"), ("Clear Formats", "clear-formats"),
                       ("Clear Contents", "clear-contents"), ("Clear Comments and Notes", "clear-comments"),
                       ("Clear Hyperlinks", "clear-hyperlinks"), ("Remove Hyperlinks", "remove-hyperlinks")])],
        B.big("Sort &\nFilter", "r-sort-filter", None, "S",
              menu=[("Sort A to Z", "sort-asc", "r-sort-asc-sm"), ("Sort Z to A", "sort-desc", "r-sort-desc-sm"),
                    ("Custom Sort...", "sort-custom"), None, ("Filter", "filter", "r-filter-sm"),
                    ("Clear", "filter-clear"), ("Reapply", "filter-reapply")]),
        B.big("Find &\nSelect", "r-find-select", None, "FD",
              menu=[("Find...", "find"), ("Replace...", "replace"), ("Go To...", "goto"),
                    ("Go To Special...", "goto-special"), None, ("Formulas", "select-formulas"),
                    ("Notes", "select-comments"), ("Conditional Formatting", "select-cf"),
                    ("Constants", "select-constants"), ("Data Validation", "select-validation"), None,
                    ("Select Objects", "select-objects"), ("Selection Pane...", "selection-pane")]),
    ])
    r.add_tab(t)

    # ================================================================ Insert
    t = r.building(Tab("Insert", "N"))
    B.group(t, "Tables", [
        B.big("PivotTable", "r-pivot-table", "pivot-table", "V", menu=[("From Table/Range", "pivot-table"),
                                                                     ("From External Data Source", "pivot-table:external"),
                                                                     ("From Data Model", "pivot-table:model")]),
        B.big("Recommended\nPivotTables", "r-recommended-pivots", "recommended-pivots", "SP"),
        B.big("Table", "r-table", "table", "T"),
    ])
    B.group(t, "Illustrations", [
        B.big("Pictures", "r-pictures", "pictures", "P"),
        B.big("Shapes", "r-shapes", None, "SH", menu=lambda: _shapes_ui.gallery(win, lambda p: win.run("shape-insert:" + p))),
        B.big("SmartArt", "r-smartart", "smartart", "M"),
        B.big("Screenshot", "r-screenshot", None, "SC", menu=lambda: _screenshot_menu(win)),
    ])
    B.group(t, "Charts", [
        B.big("Recommended\nCharts", "r-recommended-charts", "recommended-charts", "R"),
        [[B.icon("r-chart-column-sm", None, "C", "Insert Column or Bar Chart", menu=[
            ("#", "2-D Column"), ("Clustered Column", "chart-column:col"), ("Stacked Column", "chart-column:col-stacked"),
            ("100% Stacked Column", "chart-column:col-percent"), ("#", "2-D Bar"), ("Clustered Bar", "chart-column:bar"),
            ("Stacked Bar", "chart-column:bar-stacked"), ("100% Stacked Bar", "chart-column:bar-percent"), None,
            ("More Column Charts...", "chart-column")]),
          B.icon("r-chart-line-sm", None, "N", "Insert Line or Area Chart", menu=[
            ("#", "2-D Line"), ("Line", "chart-line:line"), ("Stacked Line", "chart-line:line-stacked"),
            ("100% Stacked Line", "chart-line:line-percent"), ("Line with Markers", "chart-line:line-markers"),
            ("#", "2-D Area"), ("Area", "chart-line:area"), ("Stacked Area", "chart-line:area-stacked"),
            ("100% Stacked Area", "chart-line:area-percent"), None, ("More Line Charts...", "chart-line")]),
          B.icon("r-chart-pie-sm", None, "Q", "Insert Pie or Doughnut Chart", menu=[
            ("#", "2-D Pie"), ("Pie", "chart-pie:pie"), ("#", "Doughnut"), ("Doughnut", "chart-pie:doughnut"), None,
            ("More Pie Charts...", "chart-pie")])],
         [B.icon("r-chart-hierarchy-sm", None, "I", "Insert Hierarchy Chart", menu=[("#", "Treemap"), ("Treemap", "chartex:treemap"), ("#", "Sunburst"), ("Sunburst", "chartex:sunburst"), None,
                                                                                          ("More Hierarchy Charts...", "chart-hierarchy")]),
          B.icon("r-chart-statistic-sm", None, "S1", "Insert Statistic Chart", menu=[("#", "Histogram"), ("Histogram", "chartex:histogram"), ("Pareto", "chartex:pareto"), ("#", "Box and Whisker"),
                                                                                           ("Box and Whisker", "chartex:boxwhisker"), None, ("More Statistical Charts...", "chart-statistic")]),
          B.icon("r-chart-scatter-sm", None, "D", "Insert Scatter (X, Y) Chart", menu=[
            ("#", "Scatter"), ("Scatter", "chart-scatter:scatter"), ("Scatter with Smooth Lines", "chart-scatter:scatter-smooth"),
            ("Scatter with Straight Lines", "chart-scatter:scatter-lines"), None, ("More Scatter Charts...", "chart-scatter")])],
         [B.icon("r-chart-waterfall-sm", None, "S2", "Insert Waterfall, Funnel, Stock, Surface or Radar Chart",
                 menu=[("#", "Waterfall"), ("Waterfall", "chartex:waterfall"), ("#", "Funnel"), ("Funnel", "chartex:funnel"),
                       ("#", "Stock"), ("High-Low-Close", "chart-column:stock"), ("#", "Surface"), ("Contour", "chart-column:contour"),
                       ("#", "Radar"), ("Radar", "chart-column:radar"), ("Radar with Markers", "chart-column:radar-markers"),
                       ("Filled Radar", "chart-column:radar-filled"), None,
                       ("More Waterfall and Funnel Charts...", "chart-waterfall")]),
          B.icon("r-chart-combo-sm", None, "O", "Insert Combo Chart", menu=[
            ("Clustered Column - Line", "chart-combo:combo"),
            ("Clustered Column - Line on Secondary Axis", "chart-combo:combo-secondary"), None,
            ("Create Custom Combo Chart...", "chart-combo")])]],
        B.big("Maps", "r-chart-map", None, "MP", menu=[("#", "Filled Map"), ("Filled Map", "chartex:regionmap"), None,
                                                      ("More Map Charts...", "chart-map")]),
        B.big("PivotChart", "r-pivot-chart", "pivot-chart", "V"),
    ], launcher="charts-dialog", launcher_keytip="K")
    B.group(t, "Sparklines", [
        [B.small("Line", "r-sparkline-line-sm", "sparkline-line", "SL"),
         B.small("Column", "r-sparkline-column-sm", "sparkline-column", "SO"),
         B.small("Win/Loss", "r-sparkline-winloss-sm", "sparkline-winloss", "SW")],
    ])
    B.group(t, "Filters", [
        B.big("Slicer", "r-slicer", "slicer", "SF"),
        B.big("Timeline", "r-timeline", "timeline", "ST"),
    ])
    B.group(t, "Links", [B.big("Link", "r-link", "link", "I")])
    B.group(t, "Comments", [B.big("Comment", "r-comment", "comment", "M")])
    B.group(t, "Text", [
        B.big("Text\nBox", "r-text-box", "text-box", "X"),
        B.big("Header\n& Footer", "r-header-footer", "header-footer", "H"),
        [B.small("WordArt", "r-wordart-sm", None, "W", menu=lambda: _shapes_ui.wordart_gallery(win)),
         B.small("Signature Line", "r-signature-line-sm", "signature-line", "G"),
         B.small("Object", "r-object-sm", "object", "J")],
    ])
    B.group(t, "Cell Controls", [B.big("Checkbox", "r-checkbox", "cell-checkbox", "CB",
                                       tip="Checkbox\nPut check boxes in the selected cells. The cells hold TRUE or FALSE.")])
    B.group(t, "Symbols", [
        SplitButton(B.r, "Equation", "r-equation", "equation", lambda: _equation_menu(win), size="large", keytip="E",
                    tooltip="Equation\nAdd an equation, or choose one of the common ones."),
        B.big("Symbol", "r-symbol", "symbol", "U"),
    ])
    r.add_tab(t)

    # ================================================================ Draw
    t = r.building(Tab("Draw", "JI"))
    B.group(t, "Drawing Tools", [
        RibbonButton(B.r, "Select", "r-draw-select", "draw-select", size="large", state="ink:select", keytip="S",
                     tooltip="Select\nStop drawing and select cells and objects again (Esc)."),
        RibbonButton(B.r, "Lasso\nSelect", "r-draw-lasso", "draw-lasso", size="large", state="ink:lasso", keytip="L",
                     tooltip="Lasso Select\nDraw round ink to select it, then move or delete it."),
        RibbonButton(B.r, "Eraser", "r-draw-eraser", "draw-eraser", size="large", state="ink:eraser", keytip="E",
                     tooltip="Eraser\nDrag over ink to erase whole strokes."),
        SplitButton(B.r, "Pen", "r-draw-pen", "draw-pen", B.menu(_menu_pen("pen")), size="large", state="ink:pen", keytip="P1",
                    tooltip="Pen\nDraw on the sheet. The arrow changes the pen's thickness and colour."),
        SplitButton(B.r, "Pencil", "r-draw-pencil", "draw-pencil", B.menu(_menu_pen("pencil")), size="large", state="ink:pencil", keytip="P2",
                    tooltip="Pencil\nSketch on the sheet."),
        SplitButton(B.r, "Highlighter", "r-draw-highlighter", "draw-highlighter", B.menu(_menu_pen("highlighter")), size="large",
                    state="ink:highlighter", keytip="H", tooltip="Highlighter\nMark cells with see-through ink."),
        B.big("Ruler", "r-draw-ruler", "draw-ruler", "R"),
    ])
    B.group(t, "Convert", [
        B.big("Ink to\nShape", "r-ink-to-shape", "ink-to-shape", "IS"),
        B.big("Ink to\nMath", "r-ink-to-math", "ink-to-math", "IM"),
    ])
    B.group(t, "Replay", [B.big("Ink\nReplay", "r-ink-replay", "ink-replay", "IR")])
    r.add_tab(t)

    # ================================================================ Page Layout
    t = r.building(Tab("Page Layout", "P"))
    B.group(t, "Themes", [
        B.big("Themes", "r-themes", None, "TH", menu=lambda: _themes_ui().themes_menu(win), key="themes"),
        [B.small("Colors", "r-colors-sm", None, "TC", menu=lambda: _themes_ui().colors_menu(win), key="theme-colors"),
         B.small("Fonts", "r-fonts-sm", None, "TF", menu=lambda: _themes_ui().fonts_menu(win), key="theme-fonts"),
         B.small("Effects", "r-effects-sm", None, "TE", menu=lambda: _themes_ui().effects_menu(win), key="theme-effects")],
    ])
    B.group(t, "Page Setup", [
        B.big("Margins", "r-margins", None, "M", menu=[("Normal", "margins:normal"), ("Wide", "margins:wide"),
                                                        ("Narrow", "margins:narrow"), None,
                                                        ("Custom Margins...", "page-setup-dialog")]),
        B.big("Orientation", "r-orientation", None, "O", menu=[("Portrait", "page-orientation:portrait"),
                                                             ("Landscape", "page-orientation:landscape")]),
        B.big("Size", "r-size", None, "SZ", menu=[("Letter", "page-size:letter"), ("Legal", "page-size:legal"),
                                                      ("Executive", "page-size:executive"), ("Tabloid", "page-size:tabloid"),
                                                      ("A3", "page-size:a3"), ("A4", "page-size:a4"), ("A5", "page-size:a5"),
                                                      ("B5 (JIS)", "page-size:b5"),
                                                      None, ("More Paper Sizes...", "page-setup-dialog")]),
        B.big("Print\nArea", "r-print-area", None, "R", menu=[("Set Print Area", "print-area:set"),
                                                                 ("Add to Print Area", "print-area:add"),
                                                                 ("Clear Print Area", "print-area:clear")]),
        B.big("Breaks", "r-breaks", None, "B", menu=[("Insert Page Break", "breaks:insert"),
                                                        ("Remove Page Break", "breaks:remove"),
                                                        ("Reset All Page Breaks", "breaks:reset")]),
        B.big("Background", "r-background", "background", "G"),
        B.big("Print\nTitles", "r-print-titles", "print-titles", "I"),
    ], launcher="page-setup-dialog", launcher_keytip="SP")
    B.group(t, "Scale to Fit", [
        [B.small("Width: Automatic", "r-width-sm", None, "W", menu=[("Automatic", "scale-width:0"),
                                                                       ("1 page", "scale-width:1"),
                                                                       ("2 pages", "scale-width:2")]),
         B.small("Height: Automatic", "r-height-sm", None, "H", menu=[("Automatic", "scale-height:0"),
                                                                     ("1 page", "scale-height:1"),
                                                                     ("2 pages", "scale-height:2")]),
         B.small("Scale: 100%", "r-scale-sm", "scale", "SC")],
    ], launcher="page-setup-dialog", launcher_keytip="SC")
    win.chk_gridlines_view = Gtk.CheckButton(label="View")
    win.chk_headings_view = Gtk.CheckButton(label="View")
    B.group(t, "Sheet Options", [
        [Gtk.Label(label="Gridlines", xalign=0), B.small("View", "r-gridlines-view-sm", "view-gridlines", "VG"),
         B.small("Print", "r-gridlines-print-sm", "print-gridlines", "PG")],
        [Gtk.Label(label="Headings", xalign=0), B.small("View", "r-headings-view-sm", "view-headings", "VH"),
         B.small("Print", "r-headings-print-sm", "print-headings", "PH")],
    ], launcher="page-setup-dialog", launcher_keytip="SO")
    B.group(t, "Arrange", [
        B.big("Bring\nForward", "r-bring-forward", "bring-forward", "AF"),
        B.big("Send\nBackward", "r-send-backward", "send-backward", "AE"),
        B.big("Selection\nPane", "r-selection-pane", "selection-pane", "AP"),
        [B.small("Align", "r-align-sm", None, "AA", menu=_objects_ui().ALIGN_MENU, key="align-objects"),
         B.small("Group", "r-group-sm", None, "AG", menu=_objects_ui().GROUP_MENU, key="group-objects"),
         B.small("Rotate", "r-rotate-sm", None, "AY", menu=_objects_ui().ROTATE_MENU, key="rotate-objects")],
    ])
    r.add_tab(t)

    # ================================================================ Formulas
    t = r.building(Tab("Formulas", "M"))
    def lib(cat):
        return function_menu(win, cat)
    B.group(t, "Function Library", [
        B.big("Insert\nFunction", "fx-insert", "insert-function", "F",
              tip="Insert Function (Shift+F3)\nWork with the formula in the current cell. You can easily pick "
                  "functions to use and get help on how to fill out the input values."),
        B.big("AutoSum", "fx-autosum", "autosum", "U",
              menu=[("Sum", "autosum"), ("Average", "autosum-average"), ("Count Numbers", "autosum-count"),
                    ("Max", "autosum-max"), ("Min", "autosum-min"), None, ("More Functions...", "insert-function")],
              tip="AutoSum (Alt+=)\nAutomatically add it up. Your total will appear after the selected cells."),
        B.big("Recently\nUsed", "fx-recent", None, "E", menu=lib("recent"), key="fn-recent"),
        B.big("Financial", "fx-financial", None, "I", menu=lib("Financial"), key="fn-financial"),
        B.big("Logical", "fx-logical", None, "L", menu=lib("Logical"), key="fn-logical"),
        B.big("Text", "fx-text", None, "T", menu=lib("Text"), key="fn-text"),
        B.big("Date &\nTime", "fx-datetime", None, "E2", menu=lib("Date & Time"), key="fn-datetime"),
        B.big("Lookup &\nReference", "fx-lookup", None, "O", menu=lib("Lookup & Reference"), key="fn-lookup"),
        B.big("Math &\nTrig", "fx-math", None, "G", menu=lib("Math & Trig"), key="fn-math"),
        B.big("More\nFunctions", "fx-more", None, "Q", menu=more_functions_menu(win), key="fn-more"),
    ])
    B.group(t, "Defined Names", [
        B.big("Name\nManager", "fx-name-manager", "name-manager", "N",
              tip="Name Manager (Ctrl+F3)\nCreate, edit, delete and find all the names used in the workbook."),
        [B.small("Define Name", "fx-define-name-sm", "define-name", "MD",
                 menu=[("Define Name...", "define-name"), ("Apply Names...", "apply-names")]),
         B.small("Use in Formula", "fx-use-in-formula-sm", None, "S", menu=[("Paste Names...", "use-in-formula")]),
         B.small("Create from Selection", "fx-from-selection-sm", "names-from-selection", "C",
                 tip="Create from Selection (Ctrl+Shift+F3)\nAutomatically generate names from the selected cells.")],
    ])
    B.group(t, "Formula Auditing", [
        [B.small("Trace Precedents", "fx-trace-precedents-sm", "trace-precedents", "P"),
         B.small("Trace Dependents", "fx-trace-dependents-sm", "trace-dependents", "D"),
         B.small("Remove Arrows", "fx-remove-arrows-sm", "remove-arrows", "A",
                 menu=[("Remove Arrows", "remove-arrows"), ("Remove Precedent Arrows", "remove-arrows"),
                       ("Remove Dependent Arrows", "remove-arrows")])],
        [B.toggle_small("Show Formulas", "fx-show-formulas-sm", "show-formulas", "H")
         if hasattr(B, "toggle_small") else B.small("Show Formulas", "fx-show-formulas-sm", "show-formulas", "H"),
         B.small("Error Checking", "fx-error-checking-sm", "error-checking", "K",
                 menu=[("Error Checking...", "error-checking"), ("Trace Error", "trace-precedents")]),
         B.small("Evaluate Formula", "fx-evaluate-sm", "evaluate-formula", "V")],
        B.big("Watch\nWindow", "fx-watch", "watch-window", "W"),
    ])
    B.group(t, "Calculation", [
        B.big("Calculation\nOptions", "fx-calc-options", None, "X",
              menu=[("Automatic", "calc-options:auto"), ("Automatic Except for Data Tables", "calc-options:auto-no-tables"),
                    ("Manual", "calc-options:manual")]),
        [B.small("Calculate Now", "fx-calc-now-sm", "calc-now", "B", tip="Calculate Now (F9)"),
         B.small("Calculate Sheet", "fx-calc-sheet-sm", "calc-sheet", "J", tip="Calculate Sheet (Shift+F9)")],
    ])
    r.add_tab(t)

    # ================================================================ Data
    t = r.building(Tab("Data", "A"))
    B.group(t, "Get & Transform Data", [
        B.big("Get\nData", "r-get-data", None, "PN", key="get-data", menu=[
            ("From File", [("From Workbook", "from-workbook"), ("From Text/CSV", "from-text-csv"),
                           ("From XML", "from-source:xml"), ("From JSON", "from-source:json"),
                           ("From PDF", "from-source:pdf"), ("From Folder", "from-folder")]),
            ("From Database", [("From SQLite Database", "from-source:sqlite"),
                               ("From SQL Server Database", "get-data-unavailable:Getting data from a database server")]),
            ("From Online Services", "get-data-unavailable:Getting data from online services"),
            ("From Other Sources", [("From Table/Range", "from-table-range"),
                                    ("From Web", "get-data-unavailable:Getting data from the web")]),
            None,
            ("Legacy Wizards", [("From Text (Legacy)", "from-text-legacy")]),
            ("Query Options", "get-data-unavailable:Query Options")]),
        [B.small("From Text/CSV", "r-from-text-csv-sm", "from-text-csv", "FT"),
         B.small("From Web", "r-from-web-sm", "get-data-unavailable:Getting data from the web", "FW"),
         B.small("From Table/Range", "r-from-table-sm", "from-table-range", "FR")],
        [B.small("Recent Sources", "r-recent-sources-sm", "existing-connections", "FE"),
         B.small("Existing Connections", "r-existing-connections-sm", "existing-connections", "EX")],
    ])
    B.group(t, "Queries & Connections", [
        B.big("Refresh\nAll", "r-refresh-all", "refresh-all", "R", menu=[("Refresh All", "refresh-all"), ("Refresh", "refresh")]),
        [B.small("Queries & Connections", "r-queries-connections-sm", "queries-connections", "O"),
         B.small("Workbook Links", "r-workbook-links-sm", "workbook-links", "K")],
    ])
    B.group(t, "Sort & Filter", [
        [B.icon("r-sort-asc-sm", "sort-asc", "SA", "Sort A to Z"),
         B.icon("r-sort-desc-sm", "sort-desc", "SD", "Sort Z to A")],
        B.big("Sort", "r-sort", "sort-custom", "SS"),
        B.big("Filter", "r-filter", "filter", "T"),
        [B.small("Clear", "r-filter-clear-sm", "filter-clear", "C"),
         B.small("Reapply", "r-filter-reapply-sm", "filter-reapply", "Y"),
         B.small("Advanced", "r-filter-advanced-sm", "advanced-filter", "Q")],
    ])
    B.group(t, "Data Tools", [
        B.big("Text to\nColumns", "r-text-to-columns", "text-to-columns", "E"),
        [B.icon("r-flash-fill-sm", "flash-fill", "FF", "Flash Fill (Ctrl+E)"),
         B.icon("r-remove-duplicates-sm", "remove-duplicates", "M", "Remove Duplicates"),
         B.icon("r-data-validation-sm", "data-validation", "V", "Data Validation",
                menu=[("Data Validation...", "data-validation"), ("Circle Invalid Data", "data-validation:circle"),
                      ("Clear Validation Circles", "data-validation:clear-circles")])],
        [B.icon("r-consolidate-sm", "consolidate", "N", "Consolidate"),
         B.icon("r-relationships-sm", "data-relationships", "RL", "Relationships")],
    ])
    B.group(t, "Forecast", [
        B.big("What-If\nAnalysis", "r-what-if", None, "W", key="what-if",
              menu=[("Scenario Manager...", "scenario-manager"), ("Goal Seek...", "goal-seek"),
                    ("Data Table...", "data-table")]),
        B.big("Forecast\nSheet", "r-forecast-sheet", "forecast-sheet", "G"),
    ])
    B.group(t, "Outline", [
        B.big("Group", "r-group", "group", "G2", menu=[("Group...", "group"), ("Auto Outline", "group:auto")]),
        B.big("Ungroup", "r-ungroup", "ungroup", "U", menu=[("Ungroup...", "ungroup"), ("Clear Outline", "ungroup:clear")]),
        B.big("Subtotal", "r-subtotal", "subtotal", "B"),
        [B.icon("r-show-detail-sm", "show-detail", "J", "Show Detail"),
         B.icon("r-hide-detail-sm", "hide-detail", "H", "Hide Detail")],
    ], launcher="group:settings", launcher_keytip="OS")
    B.group(t, "Analyze", [B.big("Solver", "r-solver", "solver", "Y2"),
                           B.big("Data\nAnalysis", "r-data-analysis", "data-analysis", "Y1")])
    r.add_tab(t)

    # ================================================================ Review
    t = r.building(Tab("Review", "R"))
    B.group(t, "Proofing", [
        B.big("Spelling", "r-spelling", "spelling", "S"),
        [B.small("Thesaurus", "r-thesaurus-sm", "thesaurus", "E"),
         B.small("Workbook Statistics", "r-workbook-statistics-sm", "workbook-statistics", "W")],
    ])
    B.group(t, "Accessibility", [B.big("Check\nAccessibility", "r-check-accessibility", "check-accessibility", "A1")])
    B.group(t, "Comments", [
        B.big("New\nComment", "r-new-comment", "new-comment", "C"),
        [B.small("Delete", "r-delete-comment-sm", "delete-comment", "D"),
         B.small("Previous Comment", "r-prev-comment-sm", "prev-comment", "V"),
         B.small("Next Comment", "r-next-comment-sm", "next-comment", "N")],
        B.big("Show\nComments", "r-show-comments", "show-comments", "H"),
    ])
    B.group(t, "Notes", [
        B.big("Notes", "r-notes", None, "T", menu=[("New Note", "new-note"), ("Edit Note", "edit-note"), None,
                                                    ("Previous Note", "prev-note"), ("Next Note", "next-note"), None,
                                                    ("Show/Hide Note", "show-hide-note"), ("Show All Notes", "show-all-notes"), None,
                                                    ("Convert to Comments", "convert-notes")]),
    ])
    B.group(t, "Protect", [
        B.big("Protect\nSheet", "r-protect-sheet", "protect-sheet", "PS"),
        B.big("Protect\nWorkbook", "r-protect-workbook", "protect-workbook", "PW"),
        B.big("Allow Edit\nRanges", "r-allow-edit-ranges", "allow-edit-ranges", "U"),
    ])
    B.group(t, "Ink", [B.big("Hide\nInk", "r-hide-ink", "hide-ink", "I",
                             menu=[("Hide Ink", "hide-ink"), None, ("Delete All Ink on Sheet", "hide-ink:delete-sheet"),
                                   ("Delete All Ink in Workbook", "hide-ink:delete-all")])])
    r.add_tab(t)

    # ================================================================ View
    t = r.building(Tab("View", "W"))
    B.group(t, "Workbook Views", [
        B.big("Normal", "r-view-normal", "view-normal", "L"),
        B.big("Page Break\nPreview", "r-view-page-break", "view-page-break", "I"),
        B.big("Page\nLayout", "r-view-page-layout", "view-page-layout", "P"),
        B.big("Custom\nViews", "r-custom-views", "custom-views", "C"),
    ])
    win.chk_ruler = Gtk.CheckButton(label="Ruler")
    win.chk_formula_bar = Gtk.CheckButton(label="Formula Bar")
    win.chk_gridlines = Gtk.CheckButton(label="Gridlines")
    win.chk_headings = Gtk.CheckButton(label="Headings")
    for chk, cmd, kt in ((win.chk_ruler, "view-ruler", "R"), (win.chk_formula_bar, "view-formula-bar", "F"),
                         (win.chk_gridlines, "view-gridlines", "VG"), (win.chk_headings, "view-headings", "H")):
        chk.keytip = kt
        chk.action = cmd
        chk.press = (lambda c=chk: c.set_active(not c.get_active()))
        chk.connect("toggled", lambda c, cmd=cmd: None if win.syncing_checks else win.run(cmd))
        t.controls.append(chk)
    B.group(t, "Show", [[win.chk_ruler, win.chk_formula_bar], [win.chk_gridlines, win.chk_headings]])
    B.group(t, "Zoom", [
        B.big("Zoom", "r-zoom", "zoom-dialog", "Q"),
        B.big("100%", "r-zoom-100", "zoom-100", "J"),
        B.big("Zoom to\nSelection", "r-zoom-selection", "zoom-selection", "G"),
    ])
    B.group(t, "Window", [
        B.big("New\nWindow", "r-new-window", "new-window", "N"),
        B.big("Arrange\nAll", "r-arrange-all", "arrange-all", "A"),
        B.big("Freeze\nPanes", "r-freeze-panes", None, "F",
              menu=[("Freeze Panes", "freeze-panes"), ("Freeze Top Row", "freeze-top-row"),
                    ("Freeze First Column", "freeze-first-column"), ("Unfreeze Panes", "unfreeze")]),
        [B.small("Split", "r-split-sm", "split", "S", state="split"),
         B.small("Hide", "r-hide-window-sm", "hide-window", "H"),
         B.small("Unhide", "r-unhide-window-sm", "unhide-window", "U")],
        [B.small("View Side by Side", "r-side-by-side-sm", "side-by-side", "B", state="side-by-side"),
         B.small("Synchronous Scrolling", "r-sync-scroll-sm", "sync-scroll", "SS", state="sync-scroll"),
         B.small("Reset Window Position", "r-reset-window-sm", "reset-window", "T")],
        B.big("Switch\nWindows", "r-switch-windows", None, "W", menu=win.window_list_menu),
    ])
    B.group(t, "Macros", [B.big("Macros", "r-macros", None, "M", menu=[("View Macros", "macros"), ("Record Macro...", "record-macro"),
                                                                    ("Use Relative References", "relative-refs")])])
    r.add_tab(t)

    # ================================================================ Developer
    t = r.building(Tab("Developer", "L"))
    B.group(t, "Code", [
        B.big("Visual\nBasic", "r-visual-basic", "visual-basic", "V"),
        B.big("Macros", "r-macros", "macros", "PM"),
        [B.small("Record Macro", "r-record-macro-sm", "record-macro", "R"),
         B.small("Use Relative References", "r-relative-refs-sm", "relative-refs", "U")],
    ])
    from .controls_ui import KINDS as _FC
    B.group(t, "Controls", [
        B.big("Insert", "r-form-controls", None, "I", key="form-controls-menu",
              menu=[("#", "Form Controls")] + [(lab, "form-control:" + k, "fc-" + k) for k, lab, _s in _FC]
              + [("#", "ActiveX Controls"), ("ActiveX Controls...", "activex-controls")]),
        RibbonButton(B.r, "Design\nMode", "r-design-mode", "design-mode", size="large", keytip="DM",
                     tooltip="Design Mode\nClicks select the form controls instead of using them.", state="design-mode"),
        [B.small("Properties", "r-control-properties-sm", "control-properties", "PR")],
    ])
    r.add_tab(t)

    # ================================================================ Help
    t = r.building(Tab("Help", "Y"))
    B.group(t, "Help", [B.big("Help", "r-help", "help", "H")])
    r.add_tab(t)

    # ================================================================ Chart Design (contextual)
    t = r.building(Tab("Chart Design", "JC", contextual=True))
    B.group(t, "Chart Layouts", [
        B.big("Add Chart\nElement", "r-chart-add-element", None, "A", menu=[
            ("#", "Chart Title"), ("None", "chart-add-element:title:none"), ("Above Chart", "chart-add-element:title:auto"),
            ("#", "Axis Titles"), ("Primary Horizontal", "chart-add-element:axis-title-x:on"),
            ("Primary Vertical", "chart-add-element:axis-title-y:on"),
            ("No Axis Titles", "chart-add-element:axis-title-x:off"),
            ("#", "Data Labels"), ("Show", "chart-add-element:labels:on"), ("None", "chart-add-element:labels:off"),
            ("#", "Gridlines"), ("Primary Major Horizontal", "chart-add-element:gridlines:on"),
            ("No Gridlines", "chart-add-element:gridlines:off"),
            ("#", "Legend"), ("None", "chart-add-element:legend:none"), ("Right", "chart-add-element:legend:r"),
            ("Top", "chart-add-element:legend:t"), ("Left", "chart-add-element:legend:l"),
            ("Bottom", "chart-add-element:legend:b")]),
    ])
    B.group(t, "Chart Styles", [
        B.big("Change\nColors", "r-chart-colors", None, "H", menu=[("#", "Colorful"), ("Colorful Palette", "chart-colors:0"),
            ("#", "Monochromatic")] + [("Monochrome %d" % k, "chart-colors:%d" % k) for k in range(1, 7)]),
        B.big("Format\nChart", "r-chart-format", "chart-format", "F"),
    ])
    B.group(t, "Data", [
        B.big("Switch\nRow/Column", "r-chart-switch", "chart-switch-row-col", "W"),
        B.big("Select\nData", "r-chart-select-data", "chart-select-data", "E"),
    ])
    B.group(t, "Type", [B.big("Change\nChart Type", "r-chart-change-type", "chart-change-type", "C")])
    r.add_tab(t)
    r.show_contextual("Chart Design", False)
    from . import table_ui
    table_ui.build_tab(win, B)
    from . import pivot_ui
    pivot_ui.build_tabs(win, B)
    from . import slicer_ui
    slicer_ui.build_tabs(win, B)
    from . import shapes_ui
    shapes_ui.build_tab(win, B)
    from . import sparkline_ui
    sparkline_ui.build_tab(win, B)
    from . import equation_ui
    equation_ui.build_tab(win, B)
    r.building(None)
    r.select(r.tabs[0])
