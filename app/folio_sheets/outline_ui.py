"""Group and Outline in the window: the outline bars left of the row headings and above the
column headings (level buttons, brackets, + and - buttons), Group / Ungroup, Show / Hide
Detail, Clear Outline, and Subtotal."""
from gi.repository import Gdk, Gtk

from .core import MAX_COLS, MAX_ROWS

STEP = 16          # px per outline level
BTN = 12           # a button's size
FUNCS = [("Sum", 9), ("Count", 3), ("Average", 1), ("Max", 4), ("Min", 5), ("Product", 6),
         ("Count Numbers", 2), ("StdDev", 7), ("StdDevp", 8), ("Var", 10), ("Varp", 11)]
INK = (0.35, 0.35, 0.35)


class Gutter(Gtk.DrawingArea):
    """The outline bar for rows (left) or columns (top)."""

    def __init__(self, grid, rows):
        Gtk.DrawingArea.__init__(self)
        self.grid = grid
        self.rows = rows
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.connect("draw", self.on_draw)
        self.connect("button-press-event", self.on_press)
        self.set_no_show_all(True)
        self.targets = []           # (x, y, w, h, action)

    def levels(self):
        g = self.grid
        if g.book is None:
            return 0
        try:
            return g.book.max_outline_level(g.sheet, self.rows)
        except Exception:
            return 0

    def size(self):
        n = self.levels()
        return 0 if n == 0 else (n + 1) * STEP + 4

    def sync(self):
        """Shown, and as wide (tall) as the outline is deep."""
        n = self.size()
        if n:
            if self.rows:
                self.set_size_request(n, -1)
            else:
                self.set_size_request(-1, n)
            self.show()
        else:
            self.hide()
        self.queue_draw()

    def _offset(self):
        """Where the grid area starts inside this bar (the other bar may push it)."""
        g = self.grid
        if self.rows:
            top = g.col_gutter.size() if g.col_gutter.get_visible() else 0
            return top
        return 0

    def on_draw(self, w, cr):
        g = self.grid
        a = self.get_allocation()
        cr.set_source_rgb(0.953, 0.953, 0.953)
        cr.paint()
        self.targets = []
        n = self.levels()
        if not n:
            return False
        geo = g.geometry()
        off = self._offset()
        cr.set_line_width(1)
        # the level buttons 1..n+1: across the heading row (rows), down the heading column (columns)
        for k in range(1, n + 2):
            if self.rows:
                x, y = 2 + (k - 1) * STEP, off + max(1, (geo["hh"] - BTN) / 2)
            else:
                x, y = max(1, (geo["hw"] - BTN) / 2), 2 + (k - 1) * STEP
            self._button(cr, x, y, str(k))
            self.targets.append((x, y, BTN, BTN, ("level", k)))
        # brackets and +/- for the groups on screen
        lines = {ln[0]: (ln[1], ln[2]) for ln in (geo["rows"] if self.rows else geo["cols"])}
        if not lines:
            return False
        first, last = min(lines), max(lines)
        groups = g.book.outline_groups(g.sheet, self.rows)
        closed = [(a2, b2, lv2) for a2, b2, lv2, c2, _s in groups if c2]
        for a1, b1, lv, collapsed, summary in groups:
            # inside a closed outer group: nothing of it to show
            if any(lv2 < lv and a2 <= a1 and b1 <= b2 for a2, b2, lv2 in closed):
                continue
            if b1 < first - 1 and (summary is None or summary < first):
                continue
            if a1 > last + 1 and (summary is None or summary > last):
                continue
            pos = STEP * (lv - 1) + STEP // 2 + 2
            shown = [lines[k] for k in range(max(a1, first), min(b1, last) + 1) if k in lines]
            if shown and not collapsed:
                s0 = min(p for p, _ in shown)
                s1 = max(p + h for p, h in shown)
                cr.set_source_rgb(*INK)
                if self.rows:
                    cr.move_to(pos + 0.5, off + s0 + 2)
                    cr.line_to(pos + 0.5, off + s1)
                    cr.line_to(pos + 5.5, off + s1)
                else:
                    cr.move_to(s0 + 2, pos + 0.5)
                    cr.line_to(s1, pos + 0.5)
                    cr.line_to(s1, pos + 5.5)
                cr.stroke()
            if summary is not None and summary in lines:
                p, h = lines[summary]
                if self.rows:
                    bx, by = pos - BTN / 2, off + p + (h - BTN) / 2
                else:
                    bx, by = p + (h - BTN) / 2, pos - BTN / 2
                self._button(cr, bx, by, "+" if collapsed else "−")
                self.targets.append((bx, by, BTN, BTN, ("detail", summary, collapsed, a1, b1, lv)))
        return False

    def _button(self, cr, x, y, text):
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(int(x) + 0.5, int(y) + 0.5, BTN, BTN)
        cr.fill_preserve()
        cr.set_source_rgb(0.55, 0.55, 0.55)
        cr.stroke()
        cr.set_source_rgb(0.15, 0.15, 0.15)
        cr.select_font_face("Sans")
        cr.set_font_size(9)
        ext = cr.text_extents(text)
        cr.move_to(int(x) + 0.5 + (BTN - ext.width) / 2 - ext.x_bearing, int(y) + 0.5 + (BTN - ext.height) / 2 - ext.y_bearing)
        cr.show_text(text)

    def on_press(self, w, ev):
        for x, y, bw, bh, act in self.targets:
            if x - 1 <= ev.x <= x + bw + 1 and y - 1 <= ev.y <= y + bh + 1:
                self.act(act)
                return True
        return False

    def act(self, act):
        g = self.grid
        win = g.win
        from .protect_ui import protected, refuse
        if protected(win):
            return refuse(win)
        if act[0] == "level":
            run(win, lambda: g.book.show_levels(g.sheet, self.rows, act[1]))
        else:
            _, _summary, collapsed, a, b, lv = act
            run(win, lambda: g.book.set_group_detail(g.sheet, self.rows, a, b, lv, collapsed))


def run(win, fn):
    try:
        with win.book.group("Outline"):
            fn()
    except ValueError as e:
        win.dialogs.message(str(e))
        return False
    win.grid.refresh()
    sync(win)
    win.update_title()
    return True


def sync(win):
    g = win.grid
    for gut in (g.row_gutter, g.col_gutter):
        gut.sync()


# ------------------------------------------------------------------ commands
def _which(win, title):
    """Rows or columns: from a whole-row / whole-column selection, else asked."""
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    if rg[1] == 0 and rg[3] >= MAX_COLS - 1:
        return True, rg
    if rg[0] == 0 and rg[2] >= MAX_ROWS - 1:
        return False, rg
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.pack_start(Gtk.Label(label=title, xalign=0), False, False, 0)
    rows = Gtk.RadioButton.new_with_label(None, "Rows")
    cols = Gtk.RadioButton.new_with_label_from_widget(rows, "Columns")
    box.pack_start(rows, False, False, 0)
    box.pack_start(cols, False, False, 0)
    d.rows, d.cols = rows, cols
    d.show_all()
    r = d.run()
    on_rows = rows.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return None, rg
    return on_rows, rg


def group(win, delta, arg=None):
    g = win.grid
    if arg == "clear":
        run(win, lambda: (g.book.clear_outline(g.sheet, True), g.book.clear_outline(g.sheet, False)))
        return
    if arg == "auto":
        return auto_outline(win)
    if arg == "settings":
        return settings(win)
    rows, rg = _which(win, "Group" if delta > 0 else "Ungroup")
    if rows is None:
        return
    a, b = (rg[0], rg[2]) if rows else (rg[1], rg[3])
    run(win, lambda: g.book.outline_lines(g.sheet, rows, a, b, delta))


def detail(win, show):
    g = win.grid
    r, c = g.sel.active
    rg = tuple(g.sel.ranges[-1])
    rows = not (rg[0] == 0 and rg[2] >= MAX_ROWS - 1)
    i = r if rows else c
    if g.book.max_outline_level(g.sheet, rows) == 0 and g.book.max_outline_level(g.sheet, not rows):
        rows, i = not rows, (c if rows else r)
    run(win, lambda: g.book.set_detail(g.sheet, rows, i, show))


def auto_outline(win):
    """Auto Outline: rows whose formulas SUM the rows just above become their summaries."""
    import re
    g = win.grid
    b, s = win.book, g.sheet
    ext = b.used_extent(s)
    if not ext:
        win.dialogs.message("Cannot create an outline.")
        return
    groups = []
    for r in range(0, ext[0] + 1):
        for c in range(0, min(ext[1], 50) + 1):
            t = b.input(s, r, c)
            m = re.match(r"^=\s*(?:SUM|SUBTOTAL\(\s*\d+\s*,)\s*\(?\s*\$?[A-Z]{1,3}\$?(\d+)\s*:\s*\$?[A-Z]{1,3}\$?(\d+)\s*\)",
                         t, re.I)
            if m:
                r1, r2 = int(m.group(1)) - 1, int(m.group(2)) - 1
                if r2 == r - 1 and r1 <= r2:
                    groups.append((r1, r2))
                    break
    if not groups:
        win.dialogs.message("Cannot create an outline.")
        return

    def make():
        for r1, r2 in sorted(set(groups), key=lambda x: x[1] - x[0], reverse=True):
            b.outline_lines(s, True, r1, r2, 1)
    run(win, make)


def settings(win):
    win.dialogs.info("Summary rows below detail and summary columns to the right of detail are used.", "Settings")


# ------------------------------------------------------------------ Subtotal
def subtotal(win):
    g = win.grid
    b, s = win.book, g.sheet
    rg = tuple(g.sel.ranges[-1])
    if rg[0] == rg[2] and rg[1] == rg[3]:
        rg = tuple(b.current_region(s, rg[0], rg[1]))
    if rg[2] <= rg[0]:
        win.dialogs.message("The command could not be completed by using the range specified. Select a single "
                            "cell within the range and try the command again.")
        return
    heads = [b.display(s, rg[0], c) or "Column " + _col(c) for c in range(rg[1], rg[3] + 1)]
    d = Gtk.Dialog(title="Subtotal", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.remove_btn = d.add_button("Remove All", 3)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(4)
    box.pack_start(Gtk.Label(label="At each change in:", xalign=0), False, False, 0)
    at = Gtk.ComboBoxText()
    for h in heads:
        at.append_text(h)
    at.set_active(0)
    box.pack_start(at, False, False, 0)
    box.pack_start(Gtk.Label(label="Use function:", xalign=0), False, False, 0)
    fn = Gtk.ComboBoxText()
    for name, _code in FUNCS:
        fn.append_text(name)
    fn.set_active(0)
    box.pack_start(fn, False, False, 0)
    box.pack_start(Gtk.Label(label="Add subtotal to:", xalign=0), False, False, 0)
    lb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    adds = []
    for i, h in enumerate(heads):
        cb = Gtk.CheckButton(label=h)
        cb.set_active(i == len(heads) - 1)
        adds.append(cb)
        lb.pack_start(cb, False, False, 0)
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(240, 110)
    sw.add(lb)
    box.pack_start(sw, True, True, 0)
    replace = Gtk.CheckButton(label="Replace current subtotals")
    replace.set_active(True)
    box.pack_start(replace, False, False, 0)
    # summaries go below their groups (the outline reads them there)
    d.at, d.fn, d.adds, d.replace = at, fn, adds, replace
    d.show_all()
    r = d.run()
    opts = {"at": at.get_active(), "fn": FUNCS[max(fn.get_active(), 0)], "cols": [i for i, cb in enumerate(adds) if cb.get_active()],
            "replace": replace.get_active()}
    d.destroy()
    if r == 3:
        return run(win, lambda: remove_subtotals(win, rg))
    if r != Gtk.ResponseType.OK:
        return False
    if not opts["cols"]:
        win.dialogs.message("Select at least one column to add a subtotal to.")
        return False
    return run(win, lambda: add_subtotals(win, rg, opts))


def _col(c):
    s = ""
    c += 1
    while c:
        c, m = divmod(c - 1, 26)
        s = chr(65 + m) + s
    return s


def _is_subtotal_row(b, s, r, rg):
    return any("SUBTOTAL(" in b.input(s, r, c).upper() for c in range(rg[1], rg[3] + 1))


def remove_subtotals(win, rg):
    """Remove All: the subtotal rows go, and the outline with them."""
    b, s = win.book, win.grid.sheet
    end = rg[2]
    while end + 1 < MAX_ROWS and _is_subtotal_row(b, s, end + 1, rg):
        end += 1
    for r in range(end, rg[0], -1):
        if _is_subtotal_row(b, s, r, rg):
            b.delete_rows(s, r, 1)
    b.clear_outline(s, True)
    win.grid.select_range((rg[0], rg[1], rg[0], rg[1]), (rg[0], rg[1]))


def add_subtotals(win, rg, o):
    """A subtotal row under each run of equal keys, a grand total under all, and the outline:
    each run a level-2 group inside one level-1 group over everything."""
    b, s = win.book, win.grid.sheet
    if o["replace"] and any(_is_subtotal_row(b, s, r, rg) for r in range(rg[0] + 1, min(rg[2] + 2, MAX_ROWS))):
        remove_subtotals(win, rg)
        bottom = rg[0]
        while bottom + 1 < MAX_ROWS and any(b.input(s, bottom + 1, c) != "" for c in range(rg[1], rg[3] + 1)):
            bottom += 1
        rg = (rg[0], rg[1], bottom, rg[3])
    name, code = o["fn"]
    key_c = rg[1] + o["at"]
    label = "Total" if name == "Sum" else name
    runs = []
    start = rg[0] + 1
    for r in range(rg[0] + 2, rg[2] + 2):
        if r > rg[2] or b.display(s, r, key_c) != b.display(s, start, key_c):
            runs.append((start, r - 1, b.display(s, start, key_c)))
            start = r
    n = len(runs)
    # make the rows, bottom first so the rows still to do stay where they are
    for a, z, _key in reversed(runs):
        b.insert_rows(s, z + 1, 1)
    last = rg[2] + n
    grand = last + 1
    if any(b.input(s, grand, c) != "" for c in range(rg[1], rg[3] + 1)):
        b.insert_rows(s, grand, 1)
    # then fill them in, where everything has ended up
    for k, (a, z, key) in enumerate(runs):
        a2, z2, at = a + k, z + k, z + k + 1
        b.set_input(s, at, key_c, "%s %s" % (key, label))
        for j in o["cols"]:
            c = rg[1] + j
            if c != key_c:
                b.set_input(s, at, c, "=SUBTOTAL(%d,%s%d:%s%d)" % (code, _col(c), a2 + 1, _col(c), z2 + 1))
        b.outline_lines(s, True, a2, z2, 1)
    b.set_input(s, grand, key_c, "Grand %s" % label)
    for j in o["cols"]:
        c = rg[1] + j
        if c != key_c:
            # SUBTOTAL leaves the subtotals inside its range out
            b.set_input(s, grand, c, "=SUBTOTAL(%d,%s%d:%s%d)" % (code, _col(c), rg[0] + 2, _col(c), last + 1))
    b.outline_lines(s, True, rg[0] + 1, last, 1)
    win.grid.select_range((rg[0], rg[1], grand, rg[3]), (rg[0], rg[1]))
