"""Formula editing aids: Point mode (arrow keys and clicks put references
into the formula), coloured references in the text and on the grid, F4
to cycle absolute/relative references, function AutoComplete and the
argument tip."""
from gi.repository import Gdk, GLib, Gtk, Pango

from .core import MAX_COLS, MAX_ROWS, a1, col_name, function_names, ref_spans
from . import fncatalog

# reference colours, in order of appearance in the formula
REF_COLORS = [0x2F6DB5, 0xC4382F, 0x7B3FB5, 0x2F8F3A, 0x1F8FA3, 0xB5651D, 0xC2185B, 0x5D6D7E]

POINT_AFTER = set("=(,+-*/^&<>:;% ")


def quote_sheet(name):
    simple = name.replace("_", "a").replace(".", "a").isalnum() and not name[0].isdigit()
    return name if simple else "'%s'" % name.replace("'", "''")


def ref_text(r1, c1, r2, c2, sheet=None):
    if r1 == 0 and r2 == MAX_ROWS - 1 and not (c1 == 0 and c2 == MAX_COLS - 1):
        t = "%s:%s" % (col_name(c1), col_name(c2))
    elif c1 == 0 and c2 == MAX_COLS - 1 and not (r1 == 0 and r2 == MAX_ROWS - 1):
        t = "%d:%d" % (r1 + 1, r2 + 1)
    elif (r1, c1) == (r2, c2):
        t = a1(r1, c1)
    else:
        t = "%s:%s" % (a1(r1, c1), a1(r2, c2))
    return (quote_sheet(sheet) + "!" + t) if sheet else t


def cycle_dollars(ref):
    """A1 → $A$1 → A$1 → $A1 → A1, for each end of a range."""
    def one(part):
        if "!" in part:
            sh, p = part.rsplit("!", 1)
            return sh + "!" + one(p)
        letters = "".join(ch for ch in part if ch.isalpha())
        digits = "".join(ch for ch in part if ch.isdigit())
        ca = part.startswith("$")
        ra = "$" in part[1:] if letters else part.startswith("$")
        if not letters or not digits:
            # whole column/row ends: just toggle
            bare = part.replace("$", "")
            return bare if part.startswith("$") else "$" + bare
        state = (ca, ra)
        nxt = {(False, False): (True, True), (True, True): (False, True), (False, True): (True, False),
               (True, False): (False, False)}[state]
        return ("$" if nxt[0] else "") + letters + ("$" if nxt[1] else "") + digits
    if ":" in ref and "!" not in ref.split(":")[1]:
        a, b = ref.split(":", 1)
        return one(a) + ":" + one(b)
    return one(ref)


class Popup(Gtk.Window):
    """A small undecorated window under the editor."""
    def __init__(self, parent):
        Gtk.Window.__init__(self, type=Gtk.WindowType.POPUP)
        self.set_transient_for(parent)
        self.get_style_context().add_class("folio-popup")
        self.set_type_hint(Gdk.WindowTypeHint.TOOLTIP)


class FormulaAssist:
    def __init__(self, win):
        self.win = win
        self.point = None          # {"start", "end", "anchor", "cursor", "sheet"}
        self.refs = []             # [((r1, c1, r2, c2), colour)] on the current sheet
        self._tags_for = set()
        # AutoComplete list
        self.ac = Popup(win)
        self.ac_list = Gtk.ListBox()
        self.ac_list.get_style_context().add_class("folio-aclist")
        self.ac_list.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.ac_list.connect("row-activated", lambda *_: self.ac_accept())
        sc = Gtk.ScrolledWindow()
        sc.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sc.set_propagate_natural_height(True)
        sc.set_max_content_height(220)
        sc.set_min_content_width(200)
        sc.add(self.ac_list)
        frame = Gtk.Frame()
        frame.add(sc)
        self.ac.add(frame)
        self.ac_items = []
        self.ac_word = None
        # argument tip
        self.tip = Popup(win)
        self.tip_label = Gtk.Label()
        self.tip_label.set_margin_start(6)
        self.tip_label.set_margin_end(6)
        self.tip_label.set_margin_top(2)
        self.tip_label.set_margin_bottom(2)
        tf = Gtk.Frame()
        tf.add(self.tip_label)
        self.tip.add(tf)
        self.ac_desc = None
        self._names = None

    # ------------------------------------------------------------ buffers
    def buffer(self):
        fb = self.win.formula_bar
        if fb.view.has_focus():
            return fb.buf
        return self.win.grid.editor.get_buffer()

    def text(self):
        b = self.buffer()
        return b.get_text(b.get_start_iter(), b.get_end_iter(), True)

    def cursor(self):
        b = self.buffer()
        return b.get_iter_at_mark(b.get_insert()).get_offset()

    def active(self):
        g = self.win.grid
        if not g.editing():
            return False
        t = self.text()
        return t.startswith("=") or (len(t) > 1 and t[0] in "+-" and not t[1:2].isdigit())

    # ------------------------------------------------------------ point mode
    def point_ready(self):
        if not self.active():
            return False
        pos = self.cursor()
        if self.point and self.point["end"] == pos:
            return True
        t = self.text()[:pos]
        return bool(t) and t[-1] in POINT_AFTER

    def _replace(self, start, end, s):
        b = self.buffer()
        b.begin_user_action()
        if end > start:
            b.delete(b.get_iter_at_offset(start), b.get_iter_at_offset(end))
        b.insert(b.get_iter_at_offset(start), s)
        b.end_user_action()
        b.place_cursor(b.get_iter_at_offset(start + len(s)))

    def _sheet_prefix(self):
        """A reference to another sheet than the edited cell's."""
        g = self.win.grid
        if self.point and self.point.get("sheet") is not None and self.point["sheet"] != g.edit_sheet:
            return self.win.book.sheet_info(self.point["sheet"])["name"]
        return None

    def _put(self):
        p = self.point
        r1, c1 = p["anchor"]
        r2, c2 = p["cursor"]
        s = ref_text(min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2), self._sheet_prefix())
        self._replace(p["start"], p["end"], s)
        p["end"] = p["start"] + len(s)
        g = self.win.grid
        g.ensure_visible(r2, c2)
        self.win.status.set_mode("Point")
        self.update()

    def point_move(self, dr, dc, extend=False):
        g = self.win.grid
        pos = self.cursor()
        if not (self.point and self.point["end"] == pos):
            r, c = g.editor.cell
            self.point = {"start": pos, "end": pos, "anchor": (r, c), "cursor": (r, c), "sheet": g.sheet}
        p = self.point
        r, c = p["cursor"]
        r = min(max(r + dr, 0), MAX_ROWS - 1)
        c = min(max(c + dc, 0), MAX_COLS - 1)
        p["cursor"] = (r, c)
        if not extend:
            p["anchor"] = (r, c)
        self._put()

    def insert_reference(self, r, c, extend=False):
        """A click (or drag) on the grid while typing a formula."""
        g = self.win.grid
        pos = self.cursor()
        if not (self.point and self.point["end"] == pos) and not self.point_ready():
            # clicking where a reference can't go: append an operator-less
            # reference anyway, as the original does after text
            pass
        if not (self.point and self.point["end"] == pos):
            self.point = {"start": pos, "end": pos, "anchor": (r, c), "cursor": (r, c), "sheet": g.sheet}
        p = self.point
        p["cursor"] = (r, c)
        if not extend:
            p["anchor"] = (r, c)
        p["sheet"] = g.sheet
        self._put()

    def end_point(self):
        if self.point:
            self.point = None
            if self.win.grid.editing():
                self.win.status.set_mode("Edit" if self.win.grid.editor.mode == "edit" else "Enter")

    def text_changed(self):
        """Typing (not pointing) ends the current point."""
        if self.point and self.cursor() != self.point["end"]:
            self.point = None
        self.update()

    # ------------------------------------------------------------ F4
    def cycle_reference(self):
        if not self.active():
            return False
        t = self.text()
        pos = self.cursor()
        for (a, b, *_rest) in ref_spans(t):
            if a <= pos <= b:
                s = t[a:b]
                n = cycle_dollars(s)
                self._replace(a, b, n)
                if self.point and self.point["start"] == a:
                    self.point["end"] = a + len(n)
                self.update()
                return True
        return False

    # ------------------------------------------------------------ colours
    def update(self):
        g = self.win.grid
        self.refs = []
        for buf in (g.editor.get_buffer(), self.win.formula_bar.buf):
            self._ensure_tags(buf)
            for i in range(len(REF_COLORS)):
                buf.remove_tag_by_name("ref%d" % i, buf.get_start_iter(), buf.get_end_iter())
        if not self.active():
            self.hide_popups()
            g.area.queue_draw()
            return
        t = self.text()
        here = g.sheet
        edit_sheet_name = self.win.book.sheet_info(g.edit_sheet)["name"] if g.edit_sheet is not None else None
        seen = {}
        for (a, b, sheet, r1, c1, r2, c2) in ref_spans(t):
            key = t[a:b].replace("$", "").upper()
            if key not in seen:
                seen[key] = len(seen) % len(REF_COLORS)
            k = seen[key]
            for buf in (g.editor.get_buffer(), self.win.formula_bar.buf):
                if buf.get_char_count() >= b:
                    buf.apply_tag_by_name("ref%d" % k, buf.get_iter_at_offset(a), buf.get_iter_at_offset(b))
            target = sheet if sheet is not None else edit_sheet_name
            if target is not None and target.lower() == self.win.book.sheet_info(here)["name"].lower():
                self.refs.append(((r1, c1, r2, c2), REF_COLORS[k]))
        g.area.queue_draw()
        self.update_popups()

    def _ensure_tags(self, buf):
        if id(buf) in self._tags_for:
            return
        tt = buf.get_tag_table()
        for i, c in enumerate(REF_COLORS):
            tag = Gtk.TextTag(name="ref%d" % i)
            tag.set_property("foreground", "#%06x" % c)
            tt.add(tag)
        self._tags_for.add(id(buf))

    # ------------------------------------------------------------ AutoComplete
    def names(self):
        if self._names is None:
            self._names = sorted(function_names())
        return self._names

    def _word_before(self):
        t = self.text()
        pos = self.cursor()
        i = pos
        while i > 0 and (t[i - 1].isalnum() or t[i - 1] in "._"):
            i -= 1
        word = t[i:pos]
        if not word or not word[0].isalpha():
            return None
        # inside a string literal?
        if t[:i].count('"') % 2:
            return None
        before = t[:i].rstrip()
        if before and before[-1] not in POINT_AFTER and before[-1] != "{":
            return None
        if i < len(t) and False:
            return None
        return (i, word)

    def _call_context(self):
        """(function name, argument index) of the call the cursor is in."""
        t = self.text()[:self.cursor()]
        depth = 0
        arg = 0
        in_str = False
        for i in range(len(t) - 1, -1, -1):
            ch = t[i]
            if ch == '"':
                in_str = not in_str
                continue
            if in_str:
                continue
            if ch == ")":
                depth += 1
            elif ch == "(":
                if depth == 0:
                    j = i
                    while j > 0 and (t[j - 1].isalnum() or t[j - 1] in "._"):
                        j -= 1
                    name = t[j:i].upper().replace("_XLFN.", "")
                    return (name, arg) if name else None
                depth -= 1
            elif ch in ",;" and depth == 0:
                arg += 1
        return None

    def update_popups(self):
        if not self.active():
            self.hide_popups()
            return
        w = self._word_before()
        if w and len(w[1]) >= 1 and not (self.point and self.point["end"] == self.cursor()):
            up = w[1].upper()
            items = [n for n in self.names() if n.startswith(up)]
            items += sorted(n for n, _r, _l, h in self.win.book.names()
                            if not h and n.upper().startswith(up) and not n.startswith("_xlnm"))
            if items:
                self.show_ac(items[:200], w)
                self.tip.hide()
                return
        self.ac.hide()
        ctx = self._call_context()
        if ctx and ctx[0] in fncatalog.FUNCTIONS:
            self.show_tip(*ctx)
        else:
            self.tip.hide()

    def _anchor_xy(self):
        g = self.win.grid
        ed = g.editor
        a = ed.get_allocation()
        win = ed.get_window(Gtk.TextWindowType.WIDGET)
        if win is None:
            return None
        ox, oy = win.get_origin()[1:]
        return ox, oy + a.height + 2

    def show_ac(self, items, word):
        self.ac_word = word
        if items != self.ac_items:
            for ch in self.ac_list.get_children():
                self.ac_list.remove(ch)
            for n in items:
                row = Gtk.ListBoxRow()
                lab = Gtk.Label(label=n, xalign=0)
                lab.set_margin_start(6)
                lab.set_margin_end(6)
                row.add(lab)
                row.name = n
                desc = fncatalog.describe(n)
                if desc:
                    row.set_tooltip_text(desc)
                self.ac_list.add(row)
            self.ac_items = items
            self.ac_list.show_all()
        self.ac_list.select_row(self.ac_list.get_row_at_index(0))
        xy = self._anchor_xy()
        if xy:
            self.ac.move(*xy)
        self.ac.resize(1, 1)
        self.ac.show_all()

    def ac_visible(self):
        return self.ac.get_visible()

    def ac_step(self, d):
        row = self.ac_list.get_selected_row()
        i = row.get_index() if row else -1
        n = len(self.ac_items)
        i = min(max(i + d, 0), n - 1)
        r = self.ac_list.get_row_at_index(i)
        self.ac_list.select_row(r)
        adj = self.ac_list.get_parent().get_vadjustment() if hasattr(self.ac_list.get_parent(), "get_vadjustment") else None
        if adj and r:
            alloc = r.get_allocation()
            if alloc.y < adj.get_value():
                adj.set_value(alloc.y)
            elif alloc.y + alloc.height > adj.get_value() + adj.get_page_size():
                adj.set_value(alloc.y + alloc.height - adj.get_page_size())

    def ac_accept(self):
        row = self.ac_list.get_selected_row()
        if not row or not self.ac_word:
            return False
        start, word = self.ac_word
        name = row.name
        is_fn = name in fncatalog.FUNCTIONS
        self._replace(start, start + len(word), name + ("(" if is_fn else ""))
        self.ac.hide()
        self.update()
        return True

    def show_tip(self, name, arg):
        names = fncatalog.arg_names(name)
        parts = []
        for i, n in enumerate(names):
            esc = GLib.markup_escape_text(n)
            is_more = n.strip() == "..."
            cur = (i == arg) or (is_more and arg >= i)
            parts.append("<b>%s</b>" % esc if cur and not is_more else esc)
        self.tip_label.set_markup("%s(%s)" % (GLib.markup_escape_text(name), ", ".join(parts)))
        xy = self._anchor_xy()
        if xy:
            self.tip.move(*xy)
        self.tip.show_all()

    def hide_popups(self):
        self.ac.hide()
        self.tip.hide()

    # ------------------------------------------------------------ keys
    def key(self, ev):
        """Keys while editing, before the editor handles them."""
        kv = ev.keyval
        state = ev.get_state()
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK)
        if self.ac_visible():
            if kv in (Gdk.KEY_Down, Gdk.KEY_KP_Down):
                self.ac_step(1)
                return True
            if kv in (Gdk.KEY_Up, Gdk.KEY_KP_Up):
                self.ac_step(-1)
                return True
            if kv == Gdk.KEY_Tab:
                return self.ac_accept()
            if kv == Gdk.KEY_Escape:
                self.ac.hide()
                return True
        if kv == Gdk.KEY_F4 and not (state & Gdk.ModifierType.CONTROL_MASK):
            return self.cycle_reference()
        return False


__all__ = ["FormulaAssist", "cycle_dollars", "ref_text", "REF_COLORS"]
