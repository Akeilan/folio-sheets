"""Themes (SH-22): Page Layout › Themes, Colors, Fonts and Effects. A theme is the workbook's
colours, fonts and effects; everything drawn in theme colours and theme fonts — cells, the
palette, charts, shapes, table and cell styles, sparklines — changes with it.

Custom colour and font sets (Customize Colors / Customize Fonts) and saved themes (Save Current
Theme) are kept in the user's config folder and listed under Custom, as the desktop spreadsheet
keeps them with the user rather than in the workbook."""
import json
import os

from gi.repository import Gdk, GLib, Gtk, Pango, PangoCairo

from . import APP_NAME
from .core import Book
from .ribbon_defs import set_active_theme, system_fonts

SLOT_LABELS = ["Text/Background - Dark 1", "Text/Background - Light 1", "Text/Background - Dark 2",
               "Text/Background - Light 2", "Accent 1", "Accent 2", "Accent 3", "Accent 4", "Accent 5", "Accent 6",
               "Hyperlink", "Followed Hyperlink"]


def theme_dir(win=None):
    d = getattr(win, "_theme_dir", None) if win is not None else None
    if not d:
        d = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"), "folio-sheets", "themes")
    return d


def _custom(win, kind):
    """Custom colour or font sets: [(name, data)]."""
    p = os.path.join(theme_dir(win), kind + ".json")
    try:
        with open(p) as f:
            v = json.load(f)
        return [(x["name"], x) for x in v if isinstance(x, dict) and "name" in x]
    except (OSError, ValueError):
        return []


def _save_custom(win, kind, entry):
    d = theme_dir(win)
    os.makedirs(d, exist_ok=True)
    items = [x for _, x in _custom(win, kind) if x["name"] != entry["name"]] + [entry]
    p = os.path.join(d, kind + ".json")
    with open(p + ".tmp", "w") as f:
        json.dump(items, f, indent=1)
    os.replace(p + ".tmp", p)


def custom_themes(win):
    d = os.path.join(theme_dir(win), "Document Themes")
    try:
        names = sorted(n for n in os.listdir(d) if n.lower().endswith(".thmx"))
    except OSError:
        return []
    return [(os.path.splitext(n)[0], os.path.join(d, n)) for n in names]


# ---------------------------------------------------------------- after a change
def changed(win):
    """The theme changed: the palette, the sheet, the tabs and the colour buttons follow."""
    if win.book is None:
        return
    set_active_theme(win.book.theme_colors())
    win.grid.refresh()
    win.tabs.rebuild()
    if hasattr(win, "show_last_colors"):
        win.show_last_colors()
    win.update_title()


def sync(win):
    if win.book is not None:
        set_active_theme(win.book.theme_colors())


def _apply(win, fn, *args):
    try:
        fn(*args)
    except ValueError as e:
        win.dialogs.message(str(e), "Themes")
        return False
    changed(win)
    return True


# ---------------------------------------------------------------- previews
def _rgb(cr, c):
    cr.set_source_rgb(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255)


def theme_card(name, colors, major, current=False, w=96, h=66):
    """A theme in the gallery: Aa in its headings font and text colour, its six accents."""
    da = Gtk.DrawingArea()
    da.set_size_request(w, h)

    def draw(widget, cr):
        _rgb(cr, colors[1])
        cr.rectangle(0, 0, w, h)
        cr.fill()
        lay = PangoCairo.create_layout(cr)
        fd = Pango.FontDescription(major or "Sans")
        fd.set_absolute_size(22 * Pango.SCALE)
        lay.set_font_description(fd)
        lay.set_text("Aa", -1)
        _rgb(cr, colors[0])
        cr.move_to(10, 8)
        PangoCairo.show_layout(cr, lay)
        bw = (w - 20) / 6
        for k in range(6):
            _rgb(cr, colors[4 + k])
            cr.rectangle(10 + k * bw, h - 18, bw - 2, 10)
            cr.fill()
        cr.set_line_width(2 if current else 1)
        cr.set_source_rgb(0.85, 0.55, 0.1) if current else cr.set_source_rgb(0.7, 0.7, 0.7)
        cr.rectangle(0.5, 0.5, w - 1, h - 1)
        cr.stroke()
    da.connect("draw", draw)
    da.set_tooltip_text(name)
    return da


def color_strip(colors, n=8):
    """A colour set in the Colors menu: dark 2, light 2 and the six accents."""
    da = Gtk.DrawingArea()
    da.set_size_request(n * 14, 14)

    def draw(widget, cr):
        for k, c in enumerate([colors[2], colors[3]] + list(colors[4:10])):
            _rgb(cr, c)
            cr.rectangle(k * 14, 0, 13, 13)
            cr.fill()
            cr.set_source_rgb(0.6, 0.6, 0.6)
            cr.set_line_width(1)
            cr.rectangle(k * 14 + 0.5, 0.5, 12, 12)
            cr.stroke()
    da.connect("draw", draw)
    return da


def font_preview(major, minor):
    box = Gtk.Box(spacing=8)
    aa = Gtk.Label()
    aa.set_markup('<span font_desc="%s 16">Aa</span>' % GLib.markup_escape_text(major))
    box.pack_start(aa, False, False, 0)
    names = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    for f in (major, minor):
        lab = Gtk.Label(xalign=0)
        lab.set_markup('<span font_desc="%s 10">%s</span>' % (GLib.markup_escape_text(f), GLib.markup_escape_text(f)))
        names.pack_start(lab, False, False, 0)
    box.pack_start(names, False, False, 0)
    return box


def _menu_button(label, cb, pop, child=None, name=None):
    b = Gtk.Button()
    b.set_relief(Gtk.ReliefStyle.NONE)
    if name:
        b.set_name(name)
    if child is not None:
        row = Gtk.Box(spacing=8)
        row.pack_start(child, False, False, 0)
        row.pack_start(Gtk.Label(label=label, xalign=0), True, True, 0)
        b.add(row)
    else:
        b.add(Gtk.Label(label=label, xalign=0))
    b.label_text = label
    b.connect("clicked", lambda *_: (pop.popdown(), cb()))
    return b


def _heading(text):
    lab = Gtk.Label(label=text, xalign=0)
    lab.get_style_context().add_class("dim-label")
    return lab


# ---------------------------------------------------------------- Themes
def themes_menu(win):
    pop = Gtk.Popover()
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(8)
    cur = win.book.theme_info()
    pop.cards = []

    def section(title, items):
        if not items:
            return
        box.pack_start(_heading(title), False, False, 0)
        flow = Gtk.FlowBox()
        flow.set_max_children_per_line(4)
        flow.set_min_children_per_line(min(4, len(items)))
        flow.set_selection_mode(Gtk.SelectionMode.NONE)
        for name, colors, major, cb in items:
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            v = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
            v.pack_start(theme_card(name, colors, major, current=(name == cur["name"])), False, False, 0)
            lab = Gtk.Label(label=name)
            lab.set_max_width_chars(14)
            lab.set_ellipsize(Pango.EllipsizeMode.END)
            v.pack_start(lab, False, False, 0)
            b.add(v)
            b.theme_name = name
            b.connect("clicked", lambda _b, cb=cb: (pop.popdown(), cb()))
            flow.add(b)
            pop.cards.append(b)
        box.pack_start(flow, False, False, 0)

    section("This Workbook", [(cur["name"] or "Theme", cur["colors"], cur["major"], lambda: None)])
    customs = []
    for name, path in custom_themes(win):
        try:
            x = Book.read_theme_file(path)
            i = Book.theme_xml_info(x)
        except ValueError:
            continue
        customs.append((name, i["colors"], i["major"], lambda x=x: _apply(win, win.book.set_theme_xml, x)))
    section("Custom", customs)
    section("Office", [(n, c, maj, lambda n=n: _apply(win, win.book.apply_theme, n)) for n, c, maj, _mn in Book.builtin_themes()])
    box.pack_start(Gtk.Separator(), False, False, 2)
    box.pack_start(_menu_button("Browse for Themes...", lambda: browse(win), pop), False, False, 0)
    box.pack_start(_menu_button("Save Current Theme...", lambda: save_current(win), pop), False, False, 0)
    pop.add(box)
    box.show_all()
    return pop


def browse(win, path=None):
    """Browse for Themes: a theme file (.thmx), or the theme of another workbook."""
    if path is None:
        d = Gtk.FileChooserDialog(title="Choose Theme or Themed Document", transient_for=win,
                                  action=Gtk.FileChooserAction.OPEN)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.add_button("Open", Gtk.ResponseType.OK)
        f = Gtk.FileFilter()
        f.set_name("Office Themes and Themed Documents")
        for pat in ("*.thmx", "*.xlsx", "*.xlsm", "*.xltx", "*.xltm", "*.docx", "*.pptx", "*.potx", "*.dotx"):
            f.add_pattern(pat)
        d.add_filter(f)
        if os.path.isdir(os.path.join(theme_dir(win), "Document Themes")):
            d.set_current_folder(os.path.join(theme_dir(win), "Document Themes"))
        r = d.run()
        path = d.get_filename()
        d.destroy()
        if r != Gtk.ResponseType.OK or not path:
            return False
    try:
        x = Book.read_theme_file(path)
    except ValueError as e:
        win.dialogs.message("%s couldn't use the theme in '%s'.\n\n%s" % (APP_NAME, os.path.basename(path), e), "Themes")
        return False
    return _apply(win, win.book.set_theme_xml, x)


def save_current(win, path=None):
    """Save Current Theme: a .thmx file the other suite apps (and the desktop office suite) can use."""
    d_themes = os.path.join(theme_dir(win), "Document Themes")
    if path is None:
        os.makedirs(d_themes, exist_ok=True)
        d = Gtk.FileChooserDialog(title="Save Current Theme", transient_for=win, action=Gtk.FileChooserAction.SAVE)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.add_button("Save", Gtk.ResponseType.OK)
        d.set_do_overwrite_confirmation(True)
        d.set_current_folder(d_themes)
        d.set_current_name("Theme1.thmx")
        f = Gtk.FileFilter()
        f.set_name("Office Theme (*.thmx)")
        f.add_pattern("*.thmx")
        d.add_filter(f)
        r = d.run()
        path = d.get_filename()
        d.destroy()
        if r != Gtk.ResponseType.OK or not path:
            return False
    if not path.lower().endswith(".thmx"):
        path += ".thmx"
    try:
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        win.book.save_theme(path, os.path.splitext(os.path.basename(path))[0])
    except ValueError as e:
        win.dialogs.message(str(e), "Save Current Theme")
        return False
    return path


# ---------------------------------------------------------------- Colors
def colors_menu(win):
    pop = Gtk.Popover()
    outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    outer.set_border_width(6)
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(420)
    sw.set_propagate_natural_width(True)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
    sw.add(box)
    pop.rows = []
    cur = win.book.theme_info()["colors_name"]
    custom = _custom(win, "colors")
    if custom:
        box.pack_start(_heading("Custom"), False, False, 2)
        for name, e in custom:
            b = _menu_button(name, lambda e=e: _apply(win, win.book.set_theme_colors, e["name"], e["colors"]), pop,
                             color_strip(e["colors"]))
            box.pack_start(b, False, False, 0)
            pop.rows.append(b)
    box.pack_start(_heading("Office"), False, False, 2)
    for name, colors in Book.color_sets():
        b = _menu_button(name, lambda n=name, c=colors: _apply(win, win.book.set_theme_colors, n, c), pop,
                         color_strip(colors))
        if name == cur:
            b.get_style_context().add_class("folio-current")
            b.set_tooltip_text("%s (in use)" % name)
        box.pack_start(b, False, False, 0)
        pop.rows.append(b)
    outer.pack_start(sw, True, True, 0)
    outer.pack_start(Gtk.Separator(), False, False, 2)
    outer.pack_start(_menu_button("Customize Colors...", lambda: customize_colors(win), pop), False, False, 0)
    pop.add(outer)
    outer.show_all()
    return pop


class ColorsDialog(Gtk.Dialog):
    """Create New Theme Colors: the twelve theme colours, a sample, a name."""
    def __init__(self, win):
        super().__init__(title="Create New Theme Colors", transient_for=win, modal=True)
        self.win = win
        info = win.book.theme_info()
        self.start = list(info["colors"])
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("Save", Gtk.ResponseType.OK)
        self.set_default_response(Gtk.ResponseType.OK)
        body = Gtk.Box(spacing=16)
        body.set_border_width(12)
        self.get_content_area().pack_start(body, True, True, 0)
        grid = Gtk.Grid(column_spacing=10, row_spacing=3)
        grid.attach(_heading("Theme colors"), 0, 0, 2, 1)
        self.buttons = []
        for k, lab in enumerate(SLOT_LABELS):
            grid.attach(Gtk.Label(label=lab, xalign=0), 0, k + 1, 1, 1)
            cb = Gtk.ColorButton()
            cb.set_name("slot%d" % k)
            cb.set_rgba(self._rgba(self.start[k]))
            cb.connect("color-set", lambda *_: self.sample.queue_draw())
            grid.attach(cb, 1, k + 1, 1, 1)
            self.buttons.append(cb)
        body.pack_start(grid, False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        right.pack_start(_heading("Sample"), False, False, 0)
        self.sample = Gtk.DrawingArea()
        self.sample.set_size_request(220, 150)
        self.sample.connect("draw", self._draw_sample)
        right.pack_start(self.sample, False, False, 0)
        body.pack_start(right, False, False, 0)
        row = Gtk.Box(spacing=8)
        row.set_border_width(12)
        row.pack_start(Gtk.Label(label="Name:"), False, False, 0)
        self.name = Gtk.Entry(text=self._next_name())
        self.name.set_name("name")
        self.name.set_activates_default(True)
        row.pack_start(self.name, True, True, 0)
        reset = Gtk.Button(label="Reset")
        reset.connect("clicked", lambda *_: self.reset())
        row.pack_start(reset, False, False, 0)
        self.get_content_area().pack_start(row, False, False, 0)
        self.show_all()

    def _next_name(self):
        taken = {n for n, _ in _custom(self.win, "colors")}
        k = 1
        while "Custom %d" % k in taken:
            k += 1
        return "Custom %d" % k

    @staticmethod
    def _rgba(c):
        r = Gdk.RGBA()
        r.parse("#%06x" % c)
        return r

    def colors(self):
        out = []
        for b in self.buttons:
            c = b.get_rgba()
            out.append((round(c.red * 255) << 16) | (round(c.green * 255) << 8) | round(c.blue * 255))
        return out

    def set_color(self, k, c):
        self.buttons[k].set_rgba(self._rgba(c))
        self.sample.queue_draw()

    def reset(self):
        for k, c in enumerate(self.start):
            self.buttons[k].set_rgba(self._rgba(c))
        self.sample.queue_draw()

    def _draw_sample(self, w, cr):
        c = self.colors()
        for half, (bg, fg) in enumerate(((c[1], c[0]), (c[0], c[1]))):
            x0 = half * 110
            _rgb(cr, bg)
            cr.rectangle(x0, 0, 108, 150)
            cr.fill()
            _rgb(cr, fg)
            cr.select_font_face("Sans")
            cr.set_font_size(12)
            cr.move_to(x0 + 8, 18)
            cr.show_text("Text")
            for k in range(6):
                _rgb(cr, c[4 + k])
                hgt = 20 + k * 12
                cr.rectangle(x0 + 8 + k * 15, 130 - hgt, 12, hgt)
                cr.fill()
            _rgb(cr, c[10])
            cr.move_to(x0 + 8, 146)
            cr.set_font_size(10)
            cr.show_text("Hyperlink")
            _rgb(cr, c[11])
            cr.move_to(x0 + 60, 146)
            cr.show_text("Visited")

    def apply(self):
        name = self.name.get_text().strip() or self._next_name()
        colors = self.colors()
        try:
            _save_custom(self.win, "colors", {"name": name, "colors": colors})
        except OSError as e:
            self.win.status.extra.set_text(str(e))
        return _apply(self.win, self.win.book.set_theme_colors, name, colors)


def customize_colors(win):
    d = ColorsDialog(win)
    ok = d.run() == Gtk.ResponseType.OK and d.apply()
    d.destroy()
    return ok


# ---------------------------------------------------------------- Fonts
def fonts_menu(win):
    pop = Gtk.Popover()
    outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    outer.set_border_width(6)
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(420)
    sw.set_propagate_natural_width(True)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
    sw.add(box)
    pop.rows = []
    cur = win.book.theme_info()["fonts_name"]
    custom = _custom(win, "fonts")
    if custom:
        box.pack_start(_heading("Custom"), False, False, 2)
        for name, e in custom:
            b = _menu_button(name, lambda e=e: _apply(win, win.book.set_theme_fonts, e["name"], e["major"], e["minor"]),
                             pop, font_preview(e["major"], e["minor"]))
            box.pack_start(b, False, False, 0)
            pop.rows.append(b)
    box.pack_start(_heading("Office"), False, False, 2)
    for name, major, minor in Book.font_sets():
        b = _menu_button(name, lambda n=name, a=major, z=minor: _apply(win, win.book.set_theme_fonts, n, a, z), pop,
                         font_preview(major, minor))
        if name == cur:
            b.set_tooltip_text("%s (in use)" % name)
        box.pack_start(b, False, False, 0)
        pop.rows.append(b)
    outer.pack_start(sw, True, True, 0)
    outer.pack_start(Gtk.Separator(), False, False, 2)
    outer.pack_start(_menu_button("Customize Fonts...", lambda: customize_fonts(win), pop), False, False, 0)
    pop.add(outer)
    outer.show_all()
    return pop


class FontsDialog(Gtk.Dialog):
    """Create New Theme Fonts: the headings font and the body font, a sample, a name."""
    def __init__(self, win):
        super().__init__(title="Create New Theme Fonts", transient_for=win, modal=True)
        self.win = win
        info = win.book.theme_info()
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("Save", Gtk.ResponseType.OK)
        self.set_default_response(Gtk.ResponseType.OK)
        grid = Gtk.Grid(column_spacing=10, row_spacing=6)
        grid.set_border_width(12)
        self.get_content_area().pack_start(grid, True, True, 0)
        fonts = system_fonts()
        for n in (info["major"], info["minor"]):
            if n and n not in fonts:
                fonts.append(n)
        self.major = self._combo(fonts, info["major"], "major")
        self.minor = self._combo(fonts, info["minor"], "minor")
        grid.attach(Gtk.Label(label="Heading font:", xalign=0), 0, 0, 1, 1)
        grid.attach(self.major, 1, 0, 1, 1)
        grid.attach(Gtk.Label(label="Body font:", xalign=0), 0, 1, 1, 1)
        grid.attach(self.minor, 1, 1, 1, 1)
        grid.attach(_heading("Sample"), 2, 0, 1, 1)
        self.sample = Gtk.DrawingArea()
        self.sample.set_size_request(240, 80)
        self.sample.connect("draw", self._draw_sample)
        grid.attach(self.sample, 2, 1, 1, 2)
        grid.attach(Gtk.Label(label="Name:", xalign=0), 0, 3, 1, 1)
        taken = {n for n, _ in _custom(win, "fonts")}
        k = 1
        while "Custom %d" % k in taken:
            k += 1
        self.name = Gtk.Entry(text="Custom %d" % k)
        self.name.set_name("name")
        self.name.set_activates_default(True)
        grid.attach(self.name, 1, 3, 2, 1)
        self.show_all()

    def _combo(self, fonts, cur, name):
        c = Gtk.ComboBoxText.new_with_entry()
        c.set_name(name)
        for f in fonts:
            c.append_text(f)
        c.get_child().set_text(cur)
        c.connect("changed", lambda *_: self.sample.queue_draw() if hasattr(self, "sample") else None)
        return c

    def _draw_sample(self, w, cr):
        cr.set_source_rgb(1, 1, 1)
        a = w.get_allocation()
        cr.rectangle(0, 0, a.width, a.height)
        cr.fill()
        cr.set_source_rgb(0, 0, 0)
        for k, (combo, size, text) in enumerate(((self.major, 18, "Heading"), (self.minor, 11, "Body text body text body text."))):
            lay = PangoCairo.create_layout(cr)
            fd = Pango.FontDescription(combo.get_child().get_text() or "Sans")
            fd.set_absolute_size(size * 96 / 72 * Pango.SCALE)
            lay.set_font_description(fd)
            lay.set_text(text, -1)
            cr.move_to(8, 6 + k * 34)
            PangoCairo.show_layout(cr, lay)

    def apply(self):
        name = self.name.get_text().strip() or "Custom 1"
        major = self.major.get_child().get_text().strip()
        minor = self.minor.get_child().get_text().strip()
        if not major or not minor:
            self.win.dialogs.message("Choose a heading font and a body font.", "Create New Theme Fonts")
            return False
        try:
            _save_custom(self.win, "fonts", {"name": name, "major": major, "minor": minor})
        except OSError as e:
            self.win.status.extra.set_text(str(e))
        return _apply(self.win, self.win.book.set_theme_fonts, name, major, minor)


def customize_fonts(win):
    d = FontsDialog(win)
    ok = d.run() == Gtk.ResponseType.OK and d.apply()
    d.destroy()
    return ok


# ---------------------------------------------------------------- Effects
def effects_menu(win):
    """The workbook's effects, the default theme's, and those of saved themes."""
    pop = Gtk.Popover()
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    box.set_border_width(8)
    pop.rows = []
    cur = win.book.theme_info()["effects_name"] or "Office"
    box.pack_start(_heading("This Workbook"), False, False, 0)
    lab = Gtk.Label(label=cur + "  ✓", xalign=0)
    box.pack_start(lab, False, False, 0)
    options = [("Office", Book.default_theme_xml())]
    for name, path in custom_themes(win):
        try:
            x = Book.read_theme_file(path)
        except ValueError:
            continue
        i = Book.theme_xml_info(x)
        options.append(("%s (%s)" % (i["effects_name"] or name, name), x))
    box.pack_start(_heading("Effects"), False, False, 4)
    for name, x in options:
        b = _menu_button(name, lambda x=x: _apply(win, win.book.set_theme_effects, x), pop)
        box.pack_start(b, False, False, 0)
        pop.rows.append(b)
    note = Gtk.Label(label="The other effect schemes (Subtle Solids, Glossy ...) aren't available in %s.\n"
                           "Effects of a theme you browse for or save are kept and offered here." % APP_NAME, xalign=0)
    note.get_style_context().add_class("dim-label")
    note.set_line_wrap(True)
    note.set_max_width_chars(44)
    box.pack_start(note, False, False, 6)
    pop.add(box)
    box.show_all()
    return pop
