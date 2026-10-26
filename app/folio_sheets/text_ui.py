"""Text to Columns and the Text Import Wizard: delimited or fixed-width text, a preview,
and a format for each column (General, Text, Date in a given order, or skipped)."""
import re

from gi.repository import Gdk, Gtk, Pango, PangoCairo

from .core import MAX_COLS, MAX_ROWS, a1, parse_ref

DATE_ORDERS = ["MDY", "DMY", "YMD", "MYD", "DYM", "YDM"]
PREVIEW_ROWS = 40
ENCODINGS = [("Unicode (UTF-8)", "utf-8-sig"), ("Western European (Windows-1252)", "cp1252"),
             ("Western European (ISO 8859-1)", "latin-1"), ("Unicode (UTF-16)", "utf-16")]


# ------------------------------------------------------------------ parsing
def split_delimited(line, delims, consecutive=False, qualifier='"'):
    """One line into fields. Text inside the qualifier keeps its delimiters; a doubled
    qualifier inside it is one qualifier."""
    out, cur, i, n = [], [], 0, len(line)
    quoted = False
    while i < n:
        ch = line[i]
        if qualifier and ch == qualifier:
            if quoted and i + 1 < n and line[i + 1] == qualifier:
                cur.append(qualifier)
                i += 2
                continue
            if quoted or not cur:
                quoted = not quoted
                i += 1
                continue
            cur.append(ch)
        elif not quoted and ch in delims:
            out.append("".join(cur))
            cur = []
            if consecutive:
                while i + 1 < n and line[i + 1] in delims:
                    i += 1
        else:
            cur.append(ch)
        i += 1
    out.append("".join(cur))
    return out


def split_fixed(line, breaks):
    """A line cut at the break positions (character offsets), each field trimmed on the right."""
    cuts = [0] + sorted(b for b in breaks if b > 0) + [None]
    return [line[cuts[k]:cuts[k + 1]].rstrip() if cuts[k] < len(line) else "" for k in range(len(cuts) - 1)]


def guess_breaks(lines):
    """Fixed width: break after each run of characters where every line has a space."""
    lines = [ln for ln in lines if ln.strip()][:PREVIEW_ROWS]
    if not lines:
        return []
    width = max(len(ln) for ln in lines)
    blank = [all(k >= len(ln) or ln[k] == " " for ln in lines) for k in range(width)]
    out = []
    for k in range(1, width):
        if blank[k - 1] and not blank[k]:
            out.append(k)
    return out


def looks_delimited(lines):
    sample = "\n".join(lines[:20])
    return any(d in sample for d in ("\t", ",", ";", "|"))


def guess_delims(lines):
    sample = "\n".join(lines[:20])
    counts = {d: sample.count(d) for d in ("\t", ",", ";", "|")}
    best = max(counts, key=counts.get)
    return best if counts[best] else "\t"


def parse_date(text, order):
    """A date typed in the given order (MDY...) as yyyy-mm-dd, or None."""
    parts = re.split(r"[/\-. ]+", text.strip())
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        return None
    vals = dict(zip(order, (int(p) for p in parts)))
    y, m, d = vals["Y"], vals["M"], vals["D"]
    if y < 100:
        y += 2000 if y < 30 else 1900
    if not (1 <= m <= 12 and 1 <= d <= 31):
        return None
    return "%04d-%02d-%02d" % (y, m, d)


def convert(value, fmt, decimal=".", thousands=",", trailing_minus=True):
    """A field as the text to put in its cell, or None to skip the column."""
    if fmt == "skip":
        return None
    if fmt == "text":
        return value
    if fmt.startswith("date:"):
        iso = parse_date(value, fmt[5:])
        return iso if iso is not None else value
    v = value.strip()
    if not v:
        return ""
    # numbers written with other separators, and a minus after the number
    num = v
    if trailing_minus and num.endswith("-") and num[:-1].replace(thousands, "").replace(decimal, "").isdigit():
        num = "-" + num[:-1]
    if thousands and thousands != decimal:
        cand = num.replace(thousands, "")
    else:
        cand = num
    if decimal != ".":
        cand = cand.replace(decimal, ".")
    body = cand.lstrip("+-")
    if body.replace(".", "", 1).isdigit():
        return cand
    if v.startswith("="):
        return "'" + v      # text that only looks like a formula
    return v


class Options:
    def __init__(self):
        self.kind = "delimited"         # or "fixed"
        self.start = 1                  # first row to read (1-based)
        self.delims = ","
        self.other = ""
        self.consecutive = False
        self.qualifier = '"'
        self.breaks = []
        self.formats = {}               # column index -> "general" | "text" | "date:DMY" | "skip"
        self.decimal = "."
        self.thousands = ","
        self.trailing_minus = True
        self.headers = False
        self.encoding = "utf-8-sig"

    def all_delims(self):
        return self.delims + self.other

    def rows(self, lines):
        lines = lines[self.start - 1:]
        if self.kind == "fixed":
            return [split_fixed(ln, self.breaks) for ln in lines]
        d = self.all_delims() or "\t"
        return [split_delimited(ln, d, self.consecutive, self.qualifier) for ln in lines]

    def cells(self, lines):
        """The rows as cell inputs: skipped columns left out."""
        out = []
        for row in self.rows(lines):
            vals = []
            for j, v in enumerate(row):
                t = convert(v, self.formats.get(j, "general"), self.decimal, self.thousands, self.trailing_minus)
                if t is not None:
                    vals.append((t, self.formats.get(j, "general")))
            out.append(vals)
        return out


def put(win, sheet, top, left, cells, label):
    """Write converted rows at (top, left) as one undo step; Text columns get the @ format."""
    b = win.book
    with b.group(label):
        width = max((len(r) for r in cells), default=0)
        if top + len(cells) > MAX_ROWS or left + width > MAX_COLS:
            raise ValueError("The text does not fit on the sheet from there.")
        text_cols = set()
        for row in cells:
            for j, (_t, fmt) in enumerate(row):
                if fmt == "text":
                    text_cols.add(j)
        for j in text_cols:
            b.set_number_format(sheet, [(top, left + j, top + len(cells) - 1, left + j)], "@")
        for i, row in enumerate(cells):
            for j, (t, _fmt) in enumerate(row):
                b.set_input(sheet, top + i, left + j, t)
    return (top, left, top + max(len(cells), 1) - 1, left + max(width, 1) - 1)


# ------------------------------------------------------------------ the wizard
class TextWizard(Gtk.Dialog):
    """Three steps: the kind of text, its delimiters or column breaks, and the column
    formats (with Destination for Text to Columns)."""

    def __init__(self, win, lines, title, destination=None, opts=None, reload=None):
        Gtk.Dialog.__init__(self, title=title, transient_for=win, modal=True)
        self.win = win
        self.lines = lines
        self.reload = reload            # encoding -> lines (files only)
        self.opts = opts or Options()
        self.get_style_context().add_class("folio-dialog")
        self.set_default_size(640, 520)
        self.back_btn = self.add_button("< Back", 1)
        self.next_btn = self.add_button("Next >", 2)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.finish_btn = self.add_button("Finish", Gtk.ResponseType.OK)
        self.set_default_response(2)
        self.step = 1
        self.destination = destination
        box = self.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        self.heading = Gtk.Label(xalign=0)
        box.pack_start(self.heading, False, False, 0)
        self.stack = Gtk.Stack()
        box.pack_start(self.stack, False, False, 0)
        self._step1()
        self._step2()
        self._step3()
        box.pack_start(Gtk.Label(label="Data preview", xalign=0), False, False, 0)
        self.preview_store = None
        self.preview = Gtk.TreeView()
        self.preview.set_enable_search(False)
        self.preview.get_selection().set_mode(Gtk.SelectionMode.NONE)
        self.preview.connect("button-press-event", self._preview_click)
        sw = Gtk.ScrolledWindow(hexpand=True, vexpand=True)
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(600, 190)
        sw.add(self.preview)
        box.pack_start(sw, True, True, 0)
        self.selected_col = 0
        self.show_step(1)

    # ---- step 1
    def _step1(self):
        g = Gtk.Grid(row_spacing=6, column_spacing=10)
        g.attach(Gtk.Label(label="Choose the file type that best describes your data:", xalign=0), 0, 0, 3, 1)
        self.k_delim = Gtk.RadioButton.new_with_label(None, "Delimited - Characters such as commas or tabs separate each field.")
        self.k_fixed = Gtk.RadioButton.new_with_label_from_widget(self.k_delim, "Fixed width - Fields are aligned in columns with spaces between each field.")
        (self.k_fixed if self.opts.kind == "fixed" else self.k_delim).set_active(True)
        for w in (self.k_delim, self.k_fixed):
            w.connect("toggled", lambda *_: self._changed())
        g.attach(self.k_delim, 0, 1, 3, 1)
        g.attach(self.k_fixed, 0, 2, 3, 1)
        g.attach(Gtk.Label(label="Start import at row:", xalign=0), 0, 3, 1, 1)
        self.start = Gtk.SpinButton.new_with_range(1, max(1, len(self.lines)), 1)
        self.start.set_value(self.opts.start)
        self.start.connect("value-changed", lambda *_: self._changed())
        g.attach(self.start, 1, 3, 1, 1)
        self.encoding = Gtk.ComboBoxText()
        for label, _code in ENCODINGS:
            self.encoding.append_text(label)
        codes = [c for _l, c in ENCODINGS]
        self.encoding.set_active(codes.index(self.opts.encoding) if self.opts.encoding in codes else 0)
        self.encoding.connect("changed", lambda *_: self._encoding_changed())
        if self.reload is not None:
            g.attach(Gtk.Label(label="File origin:", xalign=0), 0, 4, 1, 1)
            g.attach(self.encoding, 1, 4, 2, 1)
        self.headers = Gtk.CheckButton(label="My data has headers.")
        self.headers.set_active(self.opts.headers)
        g.attach(self.headers, 0, 5, 3, 1)
        self.stack.add_named(g, "1")

    # ---- step 2
    def _step2(self):
        delim = Gtk.Grid(row_spacing=4, column_spacing=12)
        delim.attach(Gtk.Label(label="Delimiters", xalign=0), 0, 0, 2, 1)
        self.d_checks = {}
        for i, (label, ch) in enumerate([("Tab", "\t"), ("Semicolon", ";"), ("Comma", ","), ("Space", " ")]):
            cb = Gtk.CheckButton(label=label)
            cb.set_active(ch in self.opts.delims)
            cb.connect("toggled", lambda *_: self._changed())
            self.d_checks[ch] = cb
            delim.attach(cb, 0, 1 + i, 1, 1)
        self.d_other_on = Gtk.CheckButton(label="Other:")
        self.d_other = Gtk.Entry(max_length=1, width_chars=3)
        self.d_other.set_text(self.opts.other)
        self.d_other_on.set_active(bool(self.opts.other))
        for w, sig in ((self.d_other_on, "toggled"), (self.d_other, "changed")):
            w.connect(sig, lambda *_: self._changed())
        delim.attach(self.d_other_on, 0, 5, 1, 1)
        delim.attach(self.d_other, 1, 5, 1, 1)
        self.consecutive = Gtk.CheckButton(label="Treat consecutive delimiters as one")
        self.consecutive.set_active(self.opts.consecutive)
        self.consecutive.connect("toggled", lambda *_: self._changed())
        delim.attach(self.consecutive, 2, 1, 2, 1)
        delim.attach(Gtk.Label(label="Text qualifier:", xalign=0), 2, 2, 1, 1)
        self.qualifier = Gtk.ComboBoxText()
        for q in ('"', "'", "{none}"):
            self.qualifier.append_text(q)
        self.qualifier.set_active({'"': 0, "'": 1}.get(self.opts.qualifier, 2))
        self.qualifier.connect("changed", lambda *_: self._changed())
        delim.attach(self.qualifier, 3, 2, 1, 1)
        self.stack.add_named(delim, "2d")
        fixed = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        fixed.pack_start(Gtk.Label(label="Lines with arrows signify a column break. Type the positions where the "
                                         "columns start, separated by commas.", xalign=0, wrap=True), False, False, 0)
        self.breaks = Gtk.Entry()
        self.breaks.set_text(", ".join(str(b) for b in self.opts.breaks))
        self.breaks.connect("changed", lambda *_: self._changed())
        fixed.pack_start(self.breaks, False, False, 0)
        self.ruler = Gtk.DrawingArea()
        self.ruler.set_size_request(600, 60)
        self.ruler.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.ruler.connect("draw", self._draw_ruler)
        self.ruler.connect("button-press-event", self._ruler_click)
        fixed.pack_start(Gtk.Label(label="To add a break, click the ruler; click a break to remove it.", xalign=0),
                         False, False, 0)
        fixed.pack_start(self.ruler, False, False, 0)
        self.stack.add_named(fixed, "2f")

    def _char_width(self):
        lay = self.ruler.create_pango_layout("0000000000")
        lay.set_font_description(Pango.FontDescription("Monospace 9"))
        return lay.get_pixel_size()[0] / 10.0

    def _draw_ruler(self, w, cr):
        cw = self._char_width()
        cr.set_source_rgb(1, 1, 1)
        cr.paint()
        cr.set_source_rgb(0.3, 0.3, 0.3)
        cr.set_line_width(1)
        for k in range(0, 101):
            x = 4 + k * cw
            cr.move_to(x + 0.5, 0)
            cr.line_to(x + 0.5, 6 if k % 10 else 12)
        cr.stroke()
        lay = w.create_pango_layout("")
        lay.set_font_description(Pango.FontDescription("Monospace 9"))
        for i, ln in enumerate(self.lines[self.opts.start - 1:self.opts.start + 2]):
            lay.set_text(ln[:100], -1)
            cr.move_to(4, 14 + i * 14)
            PangoCairo.show_layout(cr, lay)
        cr.set_source_rgb(0.0, 0.0, 0.0)
        for bk in self._breaks():
            x = 4 + bk * cw
            cr.move_to(x + 0.5, 0)
            cr.line_to(x + 0.5, 60)
        cr.stroke()

    def _ruler_click(self, w, ev):
        cw = self._char_width()
        k = int(round((ev.x - 4) / cw))
        br = self._breaks()
        near = [bk for bk in br if abs(bk - k) <= 1]
        if near:
            br = [bk for bk in br if bk not in near]
        elif k > 0:
            br = sorted(set(br + [k]))
        self.breaks.set_text(", ".join(str(bk) for bk in br))
        return True

    def _breaks(self):
        out = []
        for t in self.breaks.get_text().replace(";", ",").split(","):
            t = t.strip()
            if t.isdigit() and int(t) > 0:
                out.append(int(t))
        return sorted(set(out))

    # ---- step 3
    def _step3(self):
        g = Gtk.Grid(row_spacing=4, column_spacing=12)
        g.attach(Gtk.Label(label="Select each column (click it in the preview) and set the data format.", xalign=0),
                 0, 0, 3, 1)
        g.attach(Gtk.Label(label="Column data format", xalign=0), 0, 1, 1, 1)
        self.f_general = Gtk.RadioButton.new_with_label(None, "General")
        self.f_text = Gtk.RadioButton.new_with_label_from_widget(self.f_general, "Text")
        self.f_date = Gtk.RadioButton.new_with_label_from_widget(self.f_general, "Date:")
        self.f_skip = Gtk.RadioButton.new_with_label_from_widget(self.f_general, "Do not import column (skip)")
        self.date_order = Gtk.ComboBoxText()
        for o in DATE_ORDERS:
            self.date_order.append_text(o)
        self.date_order.set_active(0)
        g.attach(self.f_general, 0, 2, 1, 1)
        g.attach(self.f_text, 0, 3, 1, 1)
        g.attach(self.f_date, 0, 4, 1, 1)
        g.attach(self.date_order, 1, 4, 1, 1)
        g.attach(self.f_skip, 0, 5, 2, 1)
        for w in (self.f_general, self.f_text, self.f_date, self.f_skip):
            w.connect("toggled", lambda b: b.get_active() and self._format_picked())
        self.date_order.connect("changed", lambda *_: self._format_picked())
        g.attach(Gtk.Label(label="General converts numeric values to numbers, date values to dates, and all "
                                 "remaining values to text.", xalign=0, wrap=True, max_width_chars=40), 2, 2, 1, 3)
        self.dest = Gtk.Entry()
        if self.destination is not None:
            g.attach(Gtk.Label(label="Destination:", xalign=0), 0, 6, 1, 1)
            self.dest.set_text(a1(*self.destination))
            g.attach(self.dest, 1, 6, 1, 1)
        adv = Gtk.Expander(label="Advanced")
        ag = Gtk.Grid(row_spacing=4, column_spacing=8, margin_top=4)
        ag.attach(Gtk.Label(label="Decimal separator:", xalign=0), 0, 0, 1, 1)
        self.dec = Gtk.Entry(max_length=1, width_chars=3)
        self.dec.set_text(self.opts.decimal)
        ag.attach(self.dec, 1, 0, 1, 1)
        ag.attach(Gtk.Label(label="Thousands separator:", xalign=0), 0, 1, 1, 1)
        self.thou = Gtk.Entry(max_length=1, width_chars=3)
        self.thou.set_text(self.opts.thousands)
        ag.attach(self.thou, 1, 1, 1, 1)
        self.tminus = Gtk.CheckButton(label="Trailing minus for negative numbers")
        self.tminus.set_active(self.opts.trailing_minus)
        ag.attach(self.tminus, 0, 2, 2, 1)
        for w, sig in ((self.dec, "changed"), (self.thou, "changed"), (self.tminus, "toggled")):
            w.connect(sig, lambda *_: self._changed())
        adv.add(ag)
        g.attach(adv, 0, 7, 3, 1)
        self.stack.add_named(g, "3")

    # ---- moving through it
    def show_step(self, n):
        self.step = n
        self.read_widgets()
        names = {1: "1", 2: "2f" if self.opts.kind == "fixed" else "2d", 3: "3"}
        self.stack.set_visible_child_name(names[n])
        what = ["", "The Text Wizard has determined that your data is %s." % (
            "Fixed Width" if self.opts.kind == "fixed" else "Delimited"),
            "This screen lets you set the %s your data contains." % (
                "field widths (column breaks)" if self.opts.kind == "fixed" else "delimiters"),
            "This screen lets you select each column and set the Data Format."][n]
        self.heading.set_text("Step %d of 3 - %s" % (n, what))
        self.back_btn.set_sensitive(n > 1)
        self.next_btn.set_sensitive(n < 3)
        if n == 2 and self.opts.kind == "fixed" and not self.opts.breaks:
            self.breaks.set_text(", ".join(str(b) for b in guess_breaks(self.lines[self.opts.start - 1:])))
        self.refresh_preview()

    def read_widgets(self):
        o = self.opts
        o.kind = "fixed" if self.k_fixed.get_active() else "delimited"
        o.start = int(self.start.get_value())
        o.delims = "".join(ch for ch, cb in self.d_checks.items() if cb.get_active())
        o.other = self.d_other.get_text() if self.d_other_on.get_active() else ""
        o.consecutive = self.consecutive.get_active()
        o.qualifier = {0: '"', 1: "'"}.get(self.qualifier.get_active(), "")
        o.breaks = self._breaks()
        o.decimal = self.dec.get_text() or "."
        o.thousands = self.thou.get_text()
        o.trailing_minus = self.tminus.get_active()
        o.headers = self.headers.get_active()
        o.encoding = ENCODINGS[max(self.encoding.get_active(), 0)][1]

    def _changed(self):
        self.read_widgets()
        self.refresh_preview()
        self.ruler.queue_draw()

    def _encoding_changed(self):
        self.read_widgets()
        if self.reload is not None:
            self.lines = self.reload(self.opts.encoding)
        self.refresh_preview()

    def _format_picked(self):
        if getattr(self, "_quiet", False):
            return
        f = "general"
        if self.f_text.get_active():
            f = "text"
        elif self.f_date.get_active():
            f = "date:" + DATE_ORDERS[max(self.date_order.get_active(), 0)]
        elif self.f_skip.get_active():
            f = "skip"
        self.set_format(self.selected_col, f)

    def set_format(self, col, fmt):
        if fmt == "general":
            self.opts.formats.pop(col, None)
        else:
            self.opts.formats[col] = fmt
        self.refresh_preview()

    def select_column(self, col):
        self.selected_col = col
        f = self.opts.formats.get(col, "general")
        self._quiet = True
        {"general": self.f_general, "text": self.f_text, "skip": self.f_skip}.get(f, self.f_date).set_active(True)
        if f.startswith("date:"):
            self.date_order.set_active(DATE_ORDERS.index(f[5:]))
        self._quiet = False
        self.refresh_preview()

    def _preview_click(self, tv, ev):
        if self.step != 3:
            return False
        res = tv.get_path_at_pos(int(ev.x), int(ev.y))
        if res:
            col = tv.get_columns().index(res[1])
            self.select_column(col)
        return True

    def refresh_preview(self):
        rows = self.opts.rows(self.lines[:self.opts.start - 1 + PREVIEW_ROWS])[:PREVIEW_ROWS]
        n = max((len(r) for r in rows), default=1)
        n = max(n, 1)
        store = Gtk.ListStore(*([str] * n))
        for r in rows:
            store.append([(r[j] if j < len(r) else "") for j in range(n)])
        for c in self.preview.get_columns():
            self.preview.remove_column(c)
        for j in range(n):
            f = self.opts.formats.get(j, "general")
            title = {"general": "General", "text": "Text", "skip": "Skip Column"}.get(f, f[5:] if f.startswith("date:") else f)
            col = Gtk.TreeViewColumn(title, Gtk.CellRendererText(), text=j)
            col.set_resizable(True)
            if self.step == 3 and j == self.selected_col:
                col.set_title("[" + title + "]")
            self.preview.append_column(col)
        self.preview.set_model(store)
        self.preview_store = store
        self.columns = n

    def destination_cell(self):
        rg = parse_ref(self.dest.get_text().strip().replace("$", ""))
        return (rg[0], rg[1]) if rg else None


# ------------------------------------------------------------------ the commands
def text_to_columns(win):
    g = win.grid
    b, s = win.book, g.sheet
    if len(g.sel.ranges) > 1:
        win.dialogs.message("This operation doesn't work on multiple selections.")
        return False
    rg = tuple(g.sel.ranges[0])
    if rg[1] != rg[3]:
        win.dialogs.message("Text to Columns can only convert one column at a time. You can select many rows, "
                            "but only one column. Try again by selecting cells in one column only.")
        return False
    r2 = rg[2]
    if r2 - rg[0] > 100000:
        ext = b.used_extent(s) or (0, 0)
        r2 = min(r2, ext[0])
    lines = [b.input(s, r, rg[1]) for r in range(rg[0], r2 + 1)]
    if not any(lines):
        win.dialogs.message("No data was selected to parse.")
        return False
    opts = Options()
    opts.kind = "delimited" if looks_delimited(lines) else "fixed"
    opts.delims = ""
    if opts.kind == "delimited":
        dl = guess_delims(lines)
        opts.delims, opts.other = (dl, "") if dl in "\t;," else ("", dl)
    d = TextWizard(win, lines, "Convert Text to Columns Wizard", destination=(rg[0], rg[1]), opts=opts)
    d.show_all()
    d.show_step(1)
    while True:
        r = d.run()
        if r == 1:
            d.show_step(max(1, d.step - 1))
            continue
        if r == 2:
            d.show_step(min(3, d.step + 1))
            continue
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        d.read_widgets()
        dest = d.destination_cell()
        if dest is None:
            win.dialogs.message("The destination reference is not valid.")
            continue
        break
    opts = d.opts
    d.destroy()
    cells = opts.cells(lines)
    # replacing cells beside the column asks first
    width = max((len(row) for row in cells), default=0)
    busy = any(b.input(s, dest[0] + i, dest[1] + j) != "" for i in range(len(cells)) for j in range(width)
               if not (dest[1] + j == rg[1] and rg[0] <= dest[0] + i <= r2))
    if busy and not _confirm(win, "Do you want to replace the contents of the destination cells?"):
        return False
    try:
        # the source column is emptied first where the result doesn't reach it
        with b.group("Text to Columns"):
            if dest == (rg[0], rg[1]):
                # converted in place: every row of the column is written again
                b.clear_contents(s, [(rg[0], rg[1], r2, rg[1])])
            out = put(win, s, dest[0], dest[1], cells, "Text to Columns")
    except ValueError as e:
        win.dialogs.message(str(e))
        return False
    g.select_range(out, (out[0], out[1]))
    win.update_title()
    return True


def _confirm(win, text):
    d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.QUESTION,
                          buttons=Gtk.ButtonsType.OK_CANCEL, text=text)
    d.set_title(win.get_title())
    r = d.run()
    d.destroy()
    return r == Gtk.ResponseType.OK


def read_lines(path, encoding):
    with open(path, "rb") as f:
        raw = f.read()
    try:
        text = raw.decode(encoding)
    except UnicodeDecodeError:
        text = raw.decode(encoding, errors="replace")
    return text.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n").split("\n")


def import_wizard(win, path, into_book=None):
    """The Text Import Wizard for a file. Returns the Options, or None when cancelled."""
    lines = read_lines(path, "utf-8-sig")
    opts = Options()
    if path.lower().endswith(".csv"):
        opts.delims = ","
    elif looks_delimited(lines):
        dl = guess_delims(lines)
        opts.delims, opts.other = (dl, "") if dl in "\t;," else ("", dl)
    else:
        opts.kind = "fixed"
        opts.delims = ""
    d = TextWizard(win, lines, "Text Import Wizard", opts=opts, reload=lambda enc: read_lines(path, enc))
    d.show_all()
    d.show_step(1)
    while True:
        r = d.run()
        if r == 1:
            d.show_step(max(1, d.step - 1))
            continue
        if r == 2:
            d.show_step(min(3, d.step + 1))
            continue
        break
    d.read_widgets()
    opts, lines = d.opts, d.lines
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return None
    return opts, lines


def import_into(win, path):
    """Data > From Text: the wizard, then the text at the active cell."""
    res = import_wizard(win, path)
    if res is None:
        return False
    opts, lines = res
    g = win.grid
    r, c = g.sel.active
    try:
        out = put(win, g.sheet, r, c, opts.cells(lines), "Import Text")
    except ValueError as e:
        win.dialogs.message(str(e))
        return False
    g.select_range(out, (out[0], out[1]))
    win.update_title()
    return True
