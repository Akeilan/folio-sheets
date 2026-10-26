"""Insert › Symbol (SH-21): the Symbol dialog — Symbols (font, subset, the characters, recently used
symbols, the Unicode name and character code) and Special Characters. Insert puts the character in
the cell being edited (starting to edit the active cell if need be) or in the shape being typed in."""
import unicodedata

from gi.repository import Gdk, Gtk, Pango, PangoCairo

SUBSETS = [
    ("Basic Latin", 0x0020, 0x007E), ("Latin-1 Supplement", 0x00A0, 0x00FF), ("Latin Extended-A", 0x0100, 0x017F),
    ("Latin Extended-B", 0x0180, 0x024F), ("IPA Extensions", 0x0250, 0x02AF),
    ("Spacing Modifier Letters", 0x02B0, 0x02FF), ("Combining Diacritical Marks", 0x0300, 0x036F),
    ("Greek and Coptic", 0x0370, 0x03FF), ("Cyrillic", 0x0400, 0x04FF), ("Armenian", 0x0530, 0x058F),
    ("Hebrew", 0x0590, 0x05FF), ("Arabic", 0x0600, 0x06FF), ("Devanagari", 0x0900, 0x097F), ("Thai", 0x0E00, 0x0E7F),
    ("Latin Extended Additional", 0x1E00, 0x1EFF), ("Greek Extended", 0x1F00, 0x1FFF),
    ("General Punctuation", 0x2000, 0x206F), ("Superscripts and Subscripts", 0x2070, 0x209F),
    ("Currency Symbols", 0x20A0, 0x20CF), ("Letterlike Symbols", 0x2100, 0x214F), ("Number Forms", 0x2150, 0x218F),
    ("Arrows", 0x2190, 0x21FF), ("Mathematical Operators", 0x2200, 0x22FF), ("Miscellaneous Technical", 0x2300, 0x23FF),
    ("Enclosed Alphanumerics", 0x2460, 0x24FF), ("Box Drawing", 0x2500, 0x257F), ("Block Elements", 0x2580, 0x259F),
    ("Geometric Shapes", 0x25A0, 0x25FF), ("Miscellaneous Symbols", 0x2600, 0x26FF), ("Dingbats", 0x2700, 0x27BF),
    ("Supplemental Arrows-A", 0x27F0, 0x27FF), ("Supplemental Mathematical Operators", 0x2A00, 0x2AFF),
    ("CJK Symbols and Punctuation", 0x3000, 0x303F), ("Hiragana", 0x3040, 0x309F), ("Katakana", 0x30A0, 0x30FF),
    ("Alphabetic Presentation Forms", 0xFB00, 0xFB4F), ("Halfwidth and Fullwidth Forms", 0xFF00, 0xFFEF),
]
SPECIAL = [
    ("Em Dash", "—"), ("En Dash", "–"), ("Nonbreaking Hyphen", "‑"), ("Optional Hyphen", "­"),
    ("Em Space", " "), ("En Space", " "), ("1/4 Em Space", " "), ("Nonbreaking Space", " "),
    ("Copyright", "©"), ("Registered", "®"), ("Trademark", "™"), ("Section", "§"),
    ("Paragraph", "¶"), ("Ellipsis", "…"), ("Single Opening Quote", "‘"),
    ("Single Closing Quote", "’"), ("Double Opening Quote", "“"), ("Double Closing Quote", "”"),
    ("No-Width Optional Break", "​"), ("No-Width Non Break", "⁠"),
]
DEFAULT_RECENT = ["€", "£", "¥", "©", "®", "™", "±", "≠", "≤", "≥",
                  "÷", "×", "∞", "µ", "α", "β"]
NORMAL = "(normal text)"
COLS = 16
CELL = 30


def usable(cp):
    ch = chr(cp)
    cat = unicodedata.category(ch)
    return not cat.startswith("C") or cat == "Cf" and cp in (0x00AD, 0x200B, 0x2060)


def uname(ch):
    try:
        return unicodedata.name(ch).title()
    except ValueError:
        return ""


def subset_of(cp):
    for name, a, z in SUBSETS:
        if a <= cp <= z:
            return name
    return None


class CharGrid(Gtk.DrawingArea):
    """All the characters of the font, a row of 16 at a time."""
    def __init__(self, dialog):
        super().__init__()
        self.d = dialog
        self.font = None
        self.chars = [cp for _, a, z in SUBSETS for cp in range(a, z + 1) if usable(cp)]
        self.sel = 0
        self.set_can_focus(True)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.KEY_PRESS_MASK)
        self.set_size_request(COLS * CELL + 1, ((len(self.chars) + COLS - 1) // COLS) * CELL + 1)
        self.connect("draw", self.on_draw)
        self.connect("button-press-event", self.on_press)
        self.connect("key-press-event", self.on_key)

    def index_of(self, cp):
        try:
            return self.chars.index(cp)
        except ValueError:
            return None

    def on_draw(self, w, cr):
        clip = cr.clip_extents()
        r0 = max(0, int(clip[1] // CELL))
        r1 = min((len(self.chars) + COLS - 1) // COLS, int(clip[3] // CELL) + 1)
        sc = self.get_style_context()
        fg = sc.get_color(Gtk.StateFlags.NORMAL)
        fd = Pango.FontDescription(self.font or "Sans")
        fd.set_absolute_size(15 * Pango.SCALE)
        lay = PangoCairo.create_layout(cr)
        lay.set_font_description(fd)
        for row in range(r0, r1):
            for col in range(COLS):
                i = row * COLS + col
                if i >= len(self.chars):
                    break
                x, y = col * CELL, row * CELL
                if i == self.sel:
                    cr.set_source_rgb(0.20, 0.40, 0.75)
                    cr.rectangle(x, y, CELL, CELL)
                    cr.fill()
                    cr.set_source_rgb(1, 1, 1)
                else:
                    cr.set_source_rgba(fg.red, fg.green, fg.blue, fg.alpha)
                lay.set_text(chr(self.chars[i]), -1)
                tw, th = lay.get_pixel_size()
                cr.move_to(x + (CELL - tw) / 2, y + (CELL - th) / 2)
                PangoCairo.show_layout(cr, lay)
        cr.set_source_rgba(0.6, 0.6, 0.6, 1)
        cr.set_line_width(1)
        for row in range(r0, r1 + 1):
            cr.move_to(0, row * CELL + 0.5)
            cr.line_to(COLS * CELL, row * CELL + 0.5)
        for col in range(COLS + 1):
            cr.move_to(col * CELL + 0.5, r0 * CELL)
            cr.line_to(col * CELL + 0.5, r1 * CELL)
        cr.stroke()

    def select(self, i, scroll=True, from_subset=False):
        if not self.chars:
            return
        self.sel = max(0, min(len(self.chars) - 1, i))
        self.queue_draw()
        if scroll:
            adj = self.d.scroll.get_vadjustment()
            y = (self.sel // COLS) * CELL
            if y < adj.get_value() or y + CELL > adj.get_value() + adj.get_page_size():
                adj.set_value(max(0, y - CELL))
        self.d.selected_changed(from_subset)

    def on_press(self, w, ev):
        self.grab_focus()
        i = int(ev.y // CELL) * COLS + int(ev.x // CELL)
        if ev.x < COLS * CELL and i < len(self.chars):
            self.select(i, scroll=False)
            if ev.type == Gdk.EventType._2BUTTON_PRESS:
                self.d.insert()
        return True

    def on_key(self, w, ev):
        k = ev.keyval
        step = {Gdk.KEY_Left: -1, Gdk.KEY_Right: 1, Gdk.KEY_Up: -COLS, Gdk.KEY_Down: COLS,
                Gdk.KEY_Page_Up: -COLS * 8, Gdk.KEY_Page_Down: COLS * 8}.get(k)
        if step:
            self.select(self.sel + step)
            return True
        if k in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            self.d.insert()
            return True
        return False


class SymbolDialog(Gtk.Dialog):
    def __init__(self, win):
        super().__init__(title="Symbol", transient_for=win, modal=True)
        self.win = win
        self.set_default_size(620, 470)
        self.close_btn = self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.insert_btn = self.add_button("Insert", Gtk.ResponseType.APPLY)
        self.insert_btn.get_style_context().add_class("suggested-action")
        self.inserted = []
        nb = Gtk.Notebook()
        self.nb = nb
        self.get_content_area().pack_start(nb, True, True, 0)

        # ---- Symbols
        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        page.set_border_width(10)
        top = Gtk.Box(spacing=8)
        top.pack_start(Gtk.Label(label="Font:"), False, False, 0)
        self.font = Gtk.ComboBoxText()
        self.font.set_name("font")
        self.font.append(NORMAL, NORMAL)
        fams = sorted(f.get_name() for f in self.get_pango_context().list_families())
        for f in fams:
            self.font.append(f, f)
        self.font.set_active_id(NORMAL)
        self.font.connect("changed", self.font_changed)
        top.pack_start(self.font, True, True, 0)
        top.pack_start(Gtk.Label(label="Subset:"), False, False, 0)
        self.subset = Gtk.ComboBoxText()
        self.subset.set_name("subset")
        for name, _, _ in SUBSETS:
            self.subset.append(name, name)
        self.subset.connect("changed", self.subset_changed)
        top.pack_start(self.subset, True, True, 0)
        page.pack_start(top, False, False, 0)
        self.grid = CharGrid(self)
        self.scroll = Gtk.ScrolledWindow()
        self.scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.scroll.set_min_content_height(CELL * 7)
        self.scroll.add(self.grid)
        self.scroll.get_vadjustment().connect("value-changed", self.scrolled)
        page.pack_start(self.scroll, True, True, 0)
        page.pack_start(Gtk.Label(label="Recently used symbols:", xalign=0), False, False, 0)
        self.recent_box = Gtk.Box(spacing=0)
        page.pack_start(self.recent_box, False, False, 0)
        bottom = Gtk.Box(spacing=8)
        self.name_lbl = Gtk.Label(xalign=0)
        self.name_lbl.set_name("unicode-name")
        self.name_lbl.set_ellipsize(Pango.EllipsizeMode.END)
        bottom.pack_start(self.name_lbl, True, True, 0)
        bottom.pack_start(Gtk.Label(label="Character code:"), False, False, 0)
        self.code = Gtk.Entry()
        self.code.set_name("code")
        self.code.set_width_chars(6)
        self.code.connect("activate", self.code_entered)
        self.code.connect("changed", self.code_typed)
        bottom.pack_start(self.code, False, False, 0)
        bottom.pack_start(Gtk.Label(label="from:"), False, False, 0)
        self.base = Gtk.ComboBoxText()
        self.base.append("hex", "Unicode (hex)")
        self.base.append("dec", "ASCII (decimal)")
        self.base.set_active_id("hex")
        self.base.connect("changed", lambda *_: self.selected_changed())
        bottom.pack_start(self.base, False, False, 0)
        page.pack_start(bottom, False, False, 0)
        nb.append_page(page, Gtk.Label(label="Symbols"))

        # ---- Special Characters
        sp = Gtk.ListStore(str, str, str)
        for name, ch in SPECIAL:
            shown = ch if unicodedata.category(ch)[0] not in "CZ" else ""
            sp.append([shown, name, ch])
        self.special = Gtk.TreeView(model=sp)
        self.special.set_name("special")
        for i, title in enumerate(("Character", "")):
            col = Gtk.TreeViewColumn(title, Gtk.CellRendererText(), text=i)
            col.set_expand(i == 1)
            self.special.append_column(col)
        self.special.connect("row-activated", lambda *_: self.insert())
        sw = Gtk.ScrolledWindow()
        sw.set_border_width(10)
        sw.add(self.special)
        nb.append_page(sw, Gtk.Label(label="Special Characters"))

        self._syncing = False
        self.refresh_recent()
        self.grid.select(self.grid.index_of(ord(self.recent()[0])) or 0)
        self.connect("response", self.on_response)
        self.show_all()

    # ---- recently used
    def recent(self):
        st = getattr(self.win.app, "settings", {}) if hasattr(self.win, "app") else {}
        r = st.get("recent_symbols") or DEFAULT_RECENT
        return list(r)[:16]

    def remember(self, ch):
        if not hasattr(self.win, "app"):
            return
        r = [c for c in self.recent() if c != ch]
        self.win.app.settings["recent_symbols"] = ([ch] + r)[:16]
        try:
            self.win.app.save_settings()
        except OSError:
            pass
        self.refresh_recent()

    def refresh_recent(self):
        for c in self.recent_box.get_children():
            c.destroy()
        for ch in self.recent():
            b = Gtk.Button(label=ch)
            b.set_tooltip_text(uname(ch))
            b.set_size_request(CELL, CELL)
            b.connect("clicked", lambda _b, ch=ch: self.pick(ch))
            self.recent_box.pack_start(b, False, False, 0)
        self.recent_box.show_all()

    def pick(self, ch):
        i = self.grid.index_of(ord(ch))
        if i is not None:
            self.grid.select(i)

    # ---- the selection
    def current(self):
        if self.nb.get_current_page() == 1:
            m, it = self.special.get_selection().get_selected()
            return m[it][2] if it is not None else None
        return chr(self.grid.chars[self.grid.sel]) if self.grid.chars else None

    def selected_changed(self, from_subset=False):
        cp = self.grid.chars[self.grid.sel]
        self.name_lbl.set_text(uname(chr(cp)))
        self._syncing = True
        try:
            if self.base.get_active_id() == "dec":
                self.code.set_text(str(cp))
            else:
                self.code.set_text("%04X" % cp)
            if not from_subset:
                s = subset_of(cp)
                if s and self.subset.get_active_id() != s:
                    self.subset.set_active_id(s)
        finally:
            self._syncing = False

    def font_changed(self, combo):
        f = combo.get_active_id()
        self.grid.font = None if f in (None, NORMAL) else f
        self.grid.queue_draw()

    def subset_changed(self, combo):
        if self._syncing:
            return
        name = combo.get_active_id()
        for n, a, z in SUBSETS:
            if n == name:
                for i, cp in enumerate(self.grid.chars):
                    if cp >= a:
                        self.grid.select(i, from_subset=True)
                        adj = self.scroll.get_vadjustment()
                        adj.set_value((i // COLS) * CELL)
                        break
                break

    def scrolled(self, adj):
        # the subset follows the top row, as you scroll
        i = int(adj.get_value() // CELL) * COLS
        if i < len(self.grid.chars):
            s = subset_of(self.grid.chars[i])
            if s and s != self.subset.get_active_id():
                self._syncing = True
                self.subset.set_active_id(s)
                self._syncing = False

    def code_typed(self, e):
        if self._syncing:
            return
        t = e.get_text().strip()
        try:
            cp = int(t, 10 if self.base.get_active_id() == "dec" else 16)
        except ValueError:
            return
        i = self.grid.index_of(cp)
        if i is not None:
            self._syncing = True
            try:
                self.grid.select(i)
            finally:
                self._syncing = False
            self.name_lbl.set_text(uname(chr(cp)))

    def code_entered(self, e):
        self.code_typed(e)
        self.insert()

    # ---- insert
    def insert(self):
        ch = self.current()
        if not ch:
            return False
        ok = insert_text(self.win, ch)
        if ok:
            self.inserted.append(ch)
            self.remember(ch)
            self.close_btn.set_label("Close")
        return ok

    def on_response(self, d, resp):
        if resp == Gtk.ResponseType.APPLY:
            self.insert()
            return
        self.destroy()


def insert_text(win, text):
    """Type text at the cursor: in the shape being edited, the cell being edited, or the active cell."""
    sh = getattr(win, "shapes", None)
    if sh is not None and getattr(sh, "editor", None):
        tv = sh.editor[1]
        tv.get_buffer().insert_at_cursor(text)
        return True
    g = win.grid
    if not g.editing():
        g.start_edit(mode="edit")
        if not g.editing():
            return False
    g.editor.get_buffer().insert_at_cursor(text)
    return True


def show(win):
    d = SymbolDialog(win)
    win._symbol_dialog = d
    return d
