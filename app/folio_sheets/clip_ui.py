"""Copy, Cut, Paste and Paste Special.

Inside the program a copy keeps everything (formulas, formats, merges, widths): the
copied block is kept as a small workbook, so it can go to another window too. Other
programs get tab-separated text, and paste from them reads HTML tables or text."""
import html
import re
from html.parser import HTMLParser

from gi.repository import Gdk, Gtk

from .core import MAX_COLS, MAX_ROWS


class Clip:
    """What this program copied last (shared by all its windows)."""
    data = None      # the block as .xlsx bytes
    text = None      # what went on the system clipboard as text
    window = None    # the window it came from
    sheet = 0
    rg = None
    cut = False


def _clipboard():
    return Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)


def _text_of(win, s, rg):
    b = win.book
    lines = []
    for r in range(rg[0], rg[2] + 1):
        lines.append("\t".join(b.display(s, r, c) for c in range(rg[1], rg[3] + 1)))
    return "\r\n".join(lines) + "\r\n"


def copy(win, cut=False):
    g = win.grid
    if len(g.sel.ranges) > 1:
        win.dialogs.message("This action won't work on multiple selections.")
        return
    rg = tuple(g.sel.ranges[0])
    ext = win.book.used_extent(g.sheet) or (0, 0)
    # whole rows/columns: as far as the sheet is used
    rg = (rg[0], rg[1], min(rg[2], max(ext[0], rg[0])) if rg[2] - rg[0] > 65535 else rg[2],
          min(rg[3], max(ext[1], rg[1])) if rg[3] - rg[1] > 1023 else rg[3])
    if (rg[2] - rg[0] + 1) * (rg[3] - rg[1] + 1) > 2_000_000:
        win.dialogs.message("The selection is too large to copy.")
        return
    text = _text_of(win, g.sheet, rg)
    Clip.data = bytes(win.book.copy_to_bytes(g.sheet, rg))
    Clip.text = text
    Clip.window, Clip.sheet, Clip.rg, Clip.cut = win, g.sheet, rg, cut
    _clipboard().set_text(text, -1)
    win._copied = (g.sheet, rg, cut)
    g.copy_range = rg
    win._start_ants()
    g.refresh()


def cancel(win):
    if Clip.window is win:
        Clip.cut = False
    win.grid.copy_range = None
    win._copied = None
    win.grid.area.queue_draw()


def _ours():
    """Is the system clipboard still holding our last copy?"""
    if Clip.data is None:
        return False
    t = _clipboard().wait_for_text()
    return t is not None and t.replace("\r\n", "\n") == (Clip.text or "").replace("\r\n", "\n")


def paste(win, opts=None):
    g = win.grid
    if g.editing():
        return False
    if len(g.sel.ranges) > 1:
        win.dialogs.message("This action won't work on multiple selections.")
        return False
    dst = tuple(g.sel.ranges[0])
    b = win.book
    if _ours():
        try:
            if Clip.window is win:
                out = b.paste_cells(Clip.sheet, Clip.rg, g.sheet, dst, opts, Clip.cut and not opts)
            else:
                if opts and opts.get("link"):
                    win.dialogs.message("Links to other workbooks aren't available yet.")
                    return False
                out = b.paste_from_bytes(Clip.data, g.sheet, dst, opts)
                if Clip.cut and not opts and Clip.window is not None:
                    src = Clip.window
                    try:
                        src.book.restyle(Clip.sheet, [Clip.rg], [("set", 0)], "Cut")
                        src.book.clear_contents(Clip.sheet, [Clip.rg], "Cut")
                        src.grid.refresh()
                    except Exception:
                        pass
        except ValueError as e:
            win.dialogs.message(str(e))
            return False
        if Clip.cut and not opts:
            Clip.data = None
            if Clip.window is not None:
                cancel(Clip.window)
            cancel(win)
        g.select_range(tuple(out), tuple(out[0:2]))
        win.update_title()
        return True
    # from another program
    rows = _html_rows() or _text_rows()
    if not rows:
        return False
    tr, tc = dst[0], dst[1]
    if tr + len(rows) > MAX_ROWS or tc + max(len(r) for r in rows) > MAX_COLS:
        win.dialogs.message("The information cannot be pasted because the paste area is outside the sheet.")
        return False
    with b.group("Paste"):
        for i, row in enumerate(rows):
            for j, v in enumerate(row):
                if opts and opts.get("skip_blanks") and v == "":
                    continue
                try:
                    b.set_input(g.sheet, tr + i, tc + j, v)
                except ValueError:
                    pass
    g.select_range((tr, tc, tr + len(rows) - 1, tc + max(len(r) for r in rows) - 1), (tr, tc))
    win.update_title()
    return True


def _text_rows():
    text = _clipboard().wait_for_text()
    if not text:
        return None
    return [ln.split("\t") for ln in text.replace("\r\n", "\n").rstrip("\n").split("\n")]


class _Table(HTMLParser):
    def __init__(self):
        super().__init__()
        self.rows, self.cell, self.in_cell, self.depth = [], None, False, 0

    def handle_starttag(self, tag, attrs):
        if tag == "tr":
            self.rows.append([])
        elif tag in ("td", "th"):
            self.cell, self.in_cell = [], True
            span = dict(attrs).get("colspan")
            self._span = int(span) if span and span.isdigit() else 1
        elif tag == "br" and self.in_cell:
            self.cell.append("\n")

    def handle_endtag(self, tag):
        if tag in ("td", "th") and self.in_cell:
            if not self.rows:
                self.rows.append([])
            self.rows[-1].append(re.sub(r"[ \t\r\n]+", " ", "".join(self.cell)).strip())
            for _ in range(self._span - 1):
                self.rows[-1].append("")
            self.in_cell = False

    def handle_data(self, data):
        if self.in_cell:
            self.cell.append(data)


def _html_rows():
    try:
        sel = _clipboard().wait_for_contents(Gdk.Atom.intern("text/html", False))
    except Exception:
        return None
    if sel is None:
        return None
    raw = sel.get_data()
    if not raw:
        return None
    try:
        s = raw.decode("utf-16") if raw[:2] in (b"\xff\xfe", b"\xfe\xff") else raw.decode("utf-8", "replace")
    except Exception:
        return None
    if "<table" not in s.lower():
        return None
    p = _Table()
    p.feed(s)
    rows = [r for r in p.rows if r]
    return [[html.unescape(c) for c in r] for r in rows] or None


# ------------------------------------------------------------------ Paste Special
PASTE = [("All", "all"), ("Formulas", "formulas"), ("Values", "values"), ("Formats", "formats"),
         ("Validation", "validation"), ("All except borders", "noborders"), ("Column widths", "widths"),
         ("Formulas and number formats", "formulas_numfmt"), ("Values and number formats", "values_numfmt")]
OPERATIONS = [("None", "none"), ("Add", "add"), ("Subtract", "sub"), ("Multiply", "mul"), ("Divide", "div")]


def paste_special(win):
    if not _ours():
        # text from another program: nothing to choose
        return paste(win, {"what": "values"})
    d = Gtk.Dialog(title="Paste Special", transient_for=win, modal=True)
    d.add_button("Paste Link", 7)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    grid = Gtk.Grid(row_spacing=2, column_spacing=24)
    box.pack_start(Gtk.Label(label="Paste", xalign=0), False, False, 0)
    group = None
    whats = []
    for i, (label, key) in enumerate(PASTE):
        rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
        group = group or rb
        whats.append((rb, key))
        grid.attach(rb, i % 2, i // 2, 1, 1)
    box.pack_start(grid, False, False, 0)
    box.pack_start(Gtk.Label(label="Operation", xalign=0), False, False, 6)
    og = Gtk.Grid(row_spacing=2, column_spacing=24)
    group = None
    ops = []
    for i, (label, key) in enumerate(OPERATIONS):
        rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
        group = group or rb
        ops.append((rb, key))
        og.attach(rb, i % 2, i // 2, 1, 1)
    box.pack_start(og, False, False, 0)
    skip = Gtk.CheckButton(label="Skip blanks")
    trans = Gtk.CheckButton(label="Transpose")
    hb = Gtk.Box(spacing=24)
    hb.pack_start(skip, False, False, 0)
    hb.pack_start(trans, False, False, 0)
    box.pack_start(hb, False, False, 6)
    d.show_all()
    r = d.run()
    opts = {"what": next(k for rb, k in whats if rb.get_active()), "op": next(k for rb, k in ops if rb.get_active()),
            "skip_blanks": skip.get_active(), "transpose": trans.get_active()}
    d.destroy()
    if r == 7:
        return paste(win, {"what": "formulas", "link": True})
    if r != Gtk.ResponseType.OK:
        return False
    return paste(win, opts)
