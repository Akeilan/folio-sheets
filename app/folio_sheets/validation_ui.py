"""Data Validation: the dialog (Settings, Input Message, Error Alert), the check made when
an entry is stored, the in-cell drop-down list, the input message box, and the circles
round invalid data."""
from gi.repository import Gdk, Gtk, Pango

from . import APP_NAME
from .core import MAX_ROWS

ALLOW = [("Any value", "none"), ("Whole number", "whole"), ("Decimal", "decimal"), ("List", "list"),
         ("Date", "date"), ("Time", "time"), ("Text length", "textLength"), ("Custom", "custom")]
DATA = [("between", "between"), ("not between", "notBetween"), ("equal to", "equal"),
        ("not equal to", "notEqual"), ("greater than", "greaterThan"), ("less than", "lessThan"),
        ("greater than or equal to", "greaterThanOrEqual"), ("less than or equal to", "lessThanOrEqual")]
STYLES = [("Stop", "stop"), ("Warning", "warning"), ("Information", "information")]
DEFAULT_ERROR = "This value doesn't match the data validation restrictions defined for this cell."
MAX_CIRCLES = 255
DATE_CODE = "m/d/yyyy"
TIME_CODE = "h:mm:ss AM/PM"


def bound_labels(kind, op):
    """The labels of the one or two boxes under Data, as the kind and operator want them."""
    if kind == "list":
        return ("Source:", None)
    if kind == "custom":
        return ("Formula:", None)
    if kind == "none":
        return (None, None)
    lo, hi, one = "Minimum:", "Maximum:", "Value:"
    if kind == "date":
        lo, hi, one = "Start date:", "End date:", "Date:"
    elif kind == "time":
        lo, hi, one = "Start time:", "End time:", "Time:"
    elif kind == "textLength":
        one = "Length:"
    if op in ("between", "notBetween"):
        return (lo, hi)
    if op in ("equal", "notEqual"):
        return (one, None)
    if op in ("greaterThan", "greaterThanOrEqual"):
        return (lo, None)
    return (hi, None)


def _textview(text=""):
    tv = Gtk.TextView()
    tv.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
    tv.get_buffer().set_text(text)
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(260, 110)
    sw.add(tv)
    return sw, tv


def _tv_text(tv):
    buf = tv.get_buffer()
    return buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)


def _combo(items, key):
    cb = Gtk.ComboBoxText()
    for label, _ in items:
        cb.append_text(label)
    keys = [k for _, k in items]
    cb.set_active(keys.index(key) if key in keys else 0)
    return cb


class ValidationDialog(Gtk.Dialog):
    """Data > Data Validation. `rule` is the active cell's rule (a dict) or None."""

    def __init__(self, win, rule):
        Gtk.Dialog.__init__(self, title="Data Validation", transient_for=win, modal=True)
        self.win = win
        self.get_style_context().add_class("folio-dialog")
        self.set_resizable(False)
        self.clear_btn = self.add_button("Clear All", 1)
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        bb = self.clear_btn.get_parent()
        if isinstance(bb, Gtk.ButtonBox):
            bb.set_child_secondary(self.clear_btn, True)
        rule = dict(rule or {})
        self.original = rule
        nb = Gtk.Notebook()
        self.notebook = nb
        self.get_content_area().set_border_width(8)
        self.get_content_area().pack_start(nb, True, True, 0)

        # ---- Settings
        g = Gtk.Grid(row_spacing=6, column_spacing=12, border_width=12)
        g.attach(Gtk.Label(label="Validation criteria", xalign=0), 0, 0, 2, 1)
        g.attach(Gtk.Label(label="Allow:", xalign=0), 0, 1, 1, 1)
        self.allow = _combo(ALLOW, rule.get("kind", "none"))
        g.attach(self.allow, 0, 2, 1, 1)
        self.ignore_blank = Gtk.CheckButton(label="Ignore blank")
        self.ignore_blank.set_active(rule.get("allow_blank", True))
        g.attach(self.ignore_blank, 1, 2, 1, 1)
        self.dropdown = Gtk.CheckButton(label="In-cell dropdown")
        self.dropdown.set_active(rule.get("dropdown", True))
        g.attach(self.dropdown, 1, 3, 1, 1)
        self.op_label = Gtk.Label(label="Data:", xalign=0)
        g.attach(self.op_label, 0, 3, 1, 1)
        self.op = _combo(DATA, rule.get("op", "between"))
        g.attach(self.op, 0, 4, 1, 1)
        self.f1_label = Gtk.Label(xalign=0)
        g.attach(self.f1_label, 0, 5, 2, 1)
        self.f1 = Gtk.Entry(hexpand=True)
        self.f1.set_text(self._show_bound(rule.get("kind"), rule.get("f1", "")))
        g.attach(self.f1, 0, 6, 2, 1)
        self.f2_label = Gtk.Label(xalign=0)
        g.attach(self.f2_label, 0, 7, 2, 1)
        self.f2 = Gtk.Entry(hexpand=True)
        self.f2.set_text(self._show_bound(rule.get("kind"), rule.get("f2", "")))
        g.attach(self.f2, 0, 8, 2, 1)
        self.apply_same = Gtk.CheckButton(label="Apply these changes to all other cells with the same settings")
        g.attach(self.apply_same, 0, 9, 2, 1)
        nb.append_page(g, Gtk.Label(label="Settings"))

        # ---- Input Message
        g2 = Gtk.Grid(row_spacing=6, column_spacing=12, border_width=12)
        self.show_input = Gtk.CheckButton(label="Show input message when cell is selected")
        self.show_input.set_active(rule.get("show_input", True))
        g2.attach(self.show_input, 0, 0, 1, 1)
        g2.attach(Gtk.Label(label="When cell is selected, show this input message:", xalign=0), 0, 1, 1, 1)
        g2.attach(Gtk.Label(label="Title:", xalign=0), 0, 2, 1, 1)
        self.in_title = Gtk.Entry(hexpand=True, max_length=32)
        self.in_title.set_text(rule.get("prompt_title", ""))
        g2.attach(self.in_title, 0, 3, 1, 1)
        g2.attach(Gtk.Label(label="Input message:", xalign=0), 0, 4, 1, 1)
        sw, self.in_msg = _textview(rule.get("prompt", ""))
        g2.attach(sw, 0, 5, 1, 1)
        nb.append_page(g2, Gtk.Label(label="Input Message"))

        # ---- Error Alert
        g3 = Gtk.Grid(row_spacing=6, column_spacing=12, border_width=12)
        self.show_error = Gtk.CheckButton(label="Show error alert after invalid data is entered")
        self.show_error.set_active(rule.get("show_error", True))
        g3.attach(self.show_error, 0, 0, 2, 1)
        g3.attach(Gtk.Label(label="When user enters invalid data, show this error alert:", xalign=0), 0, 1, 2, 1)
        g3.attach(Gtk.Label(label="Style:", xalign=0), 0, 2, 1, 1)
        self.style = _combo(STYLES, rule.get("style") or "stop")
        g3.attach(self.style, 0, 3, 1, 1)
        self.style_icon = Gtk.Image()
        g3.attach(self.style_icon, 0, 4, 1, 2)
        g3.attach(Gtk.Label(label="Title:", xalign=0), 1, 2, 1, 1)
        self.err_title = Gtk.Entry(hexpand=True, max_length=32)
        self.err_title.set_text(rule.get("error_title", ""))
        g3.attach(self.err_title, 1, 3, 1, 1)
        g3.attach(Gtk.Label(label="Error message:", xalign=0), 1, 4, 1, 1)
        sw, self.err_msg = _textview(rule.get("error", ""))
        g3.attach(sw, 1, 5, 1, 1)
        nb.append_page(g3, Gtk.Label(label="Error Alert"))

        for w in (self.allow, self.op, self.style):
            w.connect("changed", lambda *_: self.sync())
        for w in (self.show_input, self.show_error):
            w.connect("toggled", lambda *_: self.sync())
        self._touched = False
        for w in (self.allow, self.op):
            w.connect("changed", self._touch)
        for w in (self.f1, self.f2):
            w.connect("changed", self._touch)
        for w in (self.ignore_blank, self.dropdown):
            w.connect("toggled", self._touch)
        self.sync()

    def _touch(self, *_):
        self._touched = True

    def _show_bound(self, kind, f):
        """A stored number shown as the date or time it stands for."""
        if not f or kind not in ("date", "time"):
            return f or ""
        try:
            n = float(f)
        except ValueError:
            return "=" + f if not f.startswith("=") else f
        return self.win.book.format_number(n, DATE_CODE if kind == "date" else TIME_CODE)

    def kind(self):
        return ALLOW[max(self.allow.get_active(), 0)][1]

    def op_key(self):
        return DATA[max(self.op.get_active(), 0)][1]

    def sync(self):
        kind = self.kind()
        ranged = kind in ("whole", "decimal", "date", "time", "textLength")
        self.op_label.set_visible(ranged)
        self.op.set_visible(ranged)
        self.dropdown.set_visible(kind == "list")
        self.ignore_blank.set_sensitive(kind != "none")
        l1, l2 = bound_labels(kind, self.op_key())
        for lab, entry, text in ((self.f1_label, self.f1, l1), (self.f2_label, self.f2, l2)):
            lab.set_visible(text is not None)
            entry.set_visible(text is not None)
            if text:
                lab.set_text(text)
        on = self.show_input.get_active()
        for w in (self.in_title, self.in_msg):
            w.set_sensitive(on)
        on = self.show_error.get_active()
        for w in (self.style, self.err_title, self.err_msg):
            w.set_sensitive(on)
        icon = {"stop": "dialog-error", "warning": "dialog-warning", "information": "dialog-information"}
        self.style_icon.set_from_icon_name(icon[STYLES[max(self.style.get_active(), 0)][1]], Gtk.IconSize.DIALOG)

    def clear_all(self):
        self.allow.set_active(0)
        self.op.set_active(0)
        for e in (self.f1, self.f2, self.in_title, self.err_title):
            e.set_text("")
        for tv in (self.in_msg, self.err_msg):
            tv.get_buffer().set_text("")
        self.ignore_blank.set_active(True)
        self.dropdown.set_active(True)
        self.show_input.set_active(True)
        self.show_error.set_active(True)
        self.style.set_active(0)
        self.apply_same.set_active(False)
        self.sync()

    def spec(self):
        kind = self.kind()
        l1, l2 = bound_labels(kind, self.op_key())
        return {
            "kind": kind, "op": self.op_key(),
            "f1": self.f1.get_text().strip() if l1 else "", "f2": self.f2.get_text().strip() if l2 else "",
            "allow_blank": self.ignore_blank.get_active(), "dropdown": self.dropdown.get_active(),
            "show_input": self.show_input.get_active(), "prompt_title": self.in_title.get_text(),
            "prompt": _tv_text(self.in_msg), "show_error": self.show_error.get_active(),
            "style": STYLES[max(self.style.get_active(), 0)][1], "error_title": self.err_title.get_text(),
            "error": _tv_text(self.err_msg),
        }


class DvUi:
    """What data validation shows in the window, kept per window."""

    def __init__(self, win):
        self.win = win
        self.circles = {}           # sheet -> [(r, c)]
        self.popover = None
        self.box = Gtk.EventBox()
        self.box.get_style_context().add_class("folio-dvmsg")
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        self.msg_title = Gtk.Label(xalign=0)
        self.msg_title.set_attributes(_bold())
        self.msg_text = Gtk.Label(xalign=0, wrap=True, max_width_chars=30)
        self.msg_text.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
        inner.pack_start(self.msg_title, False, False, 0)
        inner.pack_start(self.msg_text, False, False, 0)
        self.box.add(inner)
        inner.show_all()
        self.box.set_no_show_all(True)
        self.msg_at = None

    def attach(self, grid):
        grid.fixed.put(self.box, -1000, -1000)

    # ------------------------------------------------------------ the dialog
    def dialog(self):
        win = self.win
        g = win.grid
        b, s = win.book, g.sheet
        ranges = [tuple(x) for x in g.sel.ranges]
        rule, mixed, same = b.dv_for_selection(s, tuple(g.sel.active), ranges)
        if mixed == "erase":
            if not self._ask("The selection contains more than one type of validation.\n\n"
                             "Erase current settings and continue?", Gtk.ButtonsType.OK_CANCEL):
                return
            rule = None
        elif mixed == "extend":
            if not self._ask("The selection contains some cells without Data Validation settings.\n\n"
                             "Do you want to extend Data Validation to these cells?", Gtk.ButtonsType.YES_NO):
                return
        d = ValidationDialog(win, rule)
        d.show_all()
        d.sync()
        while True:
            r = d.run()
            if r == 1:
                d.clear_all()
                continue
            if r != Gtk.ResponseType.OK:
                d.destroy()
                return
            sp = d.spec()
            # "Apply to all other cells with the same settings" grows the target to them
            target = ranges + [tuple(x) for x in same] if d.apply_same.get_active() and rule else ranges
            try:
                if sp["kind"] == "none" and not (sp["show_input"] and (sp["prompt"] or sp["prompt_title"])):
                    b.dv_clear(s, target)
                else:
                    b.dv_set(s, target, sp)
            except ValueError as e:
                win.dialogs.message(str(e))
                continue
            break
        d.destroy()
        self.recheck_circles()
        g.refresh()
        win.update_title()

    def _ask(self, text, buttons):
        d = Gtk.MessageDialog(transient_for=self.win, modal=True, message_type=Gtk.MessageType.WARNING,
                              buttons=buttons, text=text)
        d.set_title(APP_NAME)
        r = d.run()
        d.destroy()
        return r in (Gtk.ResponseType.OK, Gtk.ResponseType.YES)

    # ------------------------------------------------------------ checking an entry
    def check_entry(self, text, cell=None, sheet=None):
        """The typed text if it may be stored, None to keep editing, False to cancel the edit."""
        win = self.win
        g = win.grid
        s = g.edit_sheet if sheet is None and g.edit_sheet is not None else (g.sheet if sheet is None else sheet)
        r, c = cell if cell is not None else (g.editor.cell or g.sel.active)
        try:
            bad = win.book.dv_check(s, r, c, text)
        except Exception:
            return text
        if not bad:
            return text
        style = bad.get("style") or "stop"
        msg = bad.get("error") or DEFAULT_ERROR
        title = bad.get("error_title") or APP_NAME
        if style == "warning":
            d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.WARNING,
                                  text=msg, secondary_text="Continue?")
            d.add_button("Yes", Gtk.ResponseType.YES)
            d.add_button("No", Gtk.ResponseType.NO)
            d.add_button("Cancel", Gtk.ResponseType.CANCEL)
            d.set_default_response(Gtk.ResponseType.NO)
        elif style == "information":
            d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.INFO, text=msg)
            d.add_button("OK", Gtk.ResponseType.OK)
            d.add_button("Cancel", Gtk.ResponseType.CANCEL)
            d.set_default_response(Gtk.ResponseType.OK)
        else:
            d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.ERROR, text=msg)
            d.add_button("Retry", Gtk.ResponseType.OK)
            d.add_button("Cancel", Gtk.ResponseType.CANCEL)
            d.set_default_response(Gtk.ResponseType.OK)
        d.set_title(title)
        resp = d.run()
        d.destroy()
        if style == "stop":
            if resp == Gtk.ResponseType.OK:
                self._select_editor_text()
                return None
            return False
        if style == "warning":
            if resp == Gtk.ResponseType.YES:
                return text
            if resp == Gtk.ResponseType.NO:
                self._select_editor_text()
                return None
            return False
        return text if resp == Gtk.ResponseType.OK else False

    def _select_editor_text(self):
        ed = self.win.grid.editor
        buf = ed.get_buffer()
        buf.select_range(buf.get_start_iter(), buf.get_end_iter())

    # ------------------------------------------------------------ the drop-down list
    def dropdown(self, r, c, rect=None):
        """The list a list rule offers, under the cell; picking stores the item."""
        win = self.win
        g = win.grid
        items = win.book.dv_list(g.sheet, r, c)
        return self._popup(r, c, items, rect, "validation")

    def pick_from_list(self):
        """Alt+Down / Pick From Drop-down List: the validation list, or the column's own entries."""
        win = self.win
        g = win.grid
        r, c = g.sel.active
        if win.book.dv_dropdown_at(g.sheet, r, c):
            return self.dropdown(r, c)
        return self._popup(r, c, self.column_entries(r, c), None, "column")

    def column_entries(self, r, c):
        """The distinct text entries in the run of filled cells above and below (like AutoComplete)."""
        b, s = self.win.book, self.win.grid.sheet
        rows = []
        for step in (-1, 1):
            rr = r + step
            while 0 <= rr < MAX_ROWS and abs(rr - r) <= 5000:
                t = b.input(s, rr, c)
                if t == "":
                    break
                rows.append((rr, t))
                rr += step
        seen = {}
        for _, t in sorted(rows):
            # the first spelling down the column stands for the others
            if not t.startswith("=") and not _is_number(t):
                seen.setdefault(t.lower(), t)
        return sorted(seen.values(), key=str.lower)

    def _popup(self, r, c, items, rect, why):
        win = self.win
        g = win.grid
        if not items:
            return None
        if rect is None:
            rect = win.book.cell_rect(g.sheet, r, c, g.params())
        self.close_popup()
        pop = Gtk.Popover()
        pop.get_style_context().add_class("folio-dvlist")
        pop.set_relative_to(g.area)
        if rect:
            x, y, w, h = rect
            pr = Gdk.Rectangle()
            pr.x, pr.y, pr.width, pr.height = int(x), int(y), int(max(w, 1)), int(h)
            pop.set_pointing_to(pr)
        pop.set_position(Gtk.PositionType.BOTTOM)
        lb = Gtk.ListBox()
        lb.set_selection_mode(Gtk.SelectionMode.BROWSE)
        for it in items:
            row = Gtk.ListBoxRow()
            lab = Gtk.Label(label=it, xalign=0)
            lab.set_margin_start(4)
            lab.set_margin_end(12)
            row.add(lab)
            row.item = it
            lb.add(row)
        cur = win.book.display(g.sheet, r, c)
        for row in lb.get_children():
            if row.item == cur:
                lb.select_row(row)
                break
        else:
            first = lb.get_row_at_index(0)
            if first:
                lb.select_row(first)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_propagate_natural_height(True)
        sw.set_max_content_height(8 * 22)
        sw.set_min_content_width(int(max(rect[2] if rect else 80, 80)))
        sw.add(lb)
        pop.add(sw)

        def pick(_lb, row):
            if row is None:
                return
            pop.popdown()
            self.store_pick(r, c, row.item)
        lb.connect("row-activated", pick)
        pop.connect("closed", lambda *_: (g.area.grab_focus(), setattr(self, "popover", None)))
        pop.list = lb
        pop.items = list(items)
        pop.pick = lambda i: pick(lb, lb.get_row_at_index(i))
        pop.why = why
        sw.show_all()
        pop.popup()
        sel = lb.get_selected_row()
        if sel:
            sel.grab_focus()
        self.popover = pop
        return pop

    def close_popup(self):
        if self.popover is not None:
            self.popover.popdown()
            self.popover = None

    def store_pick(self, r, c, item):
        win = self.win
        g = win.grid
        with win.book.group("Typing"):
            # a list item is text as it stands, even one that looks like a formula
            v = "'" + item if item[:1] in ("=", "+", "-") and not _is_number(item) else item
            win.book.set_input(g.sheet, r, c, v)
        self.recheck_circles()
        g.refresh()
        win.update_title()

    # ------------------------------------------------------------ the input message
    def update_message(self):
        win = self.win
        g = win.grid
        if win.book is None or g.edit_sheet not in (None, g.sheet):
            self._hide()
            return
        r, c = g.sel.active
        try:
            rule = win.book.dv_at(g.sheet, r, c)
        except Exception:
            rule = None
        if not rule or not rule.get("show_input") or not (rule.get("prompt") or rule.get("prompt_title")):
            self._hide()
            return
        self.msg_title.set_text(rule.get("prompt_title", ""))
        self.msg_title.set_visible(bool(rule.get("prompt_title")))
        self.msg_text.set_text(rule.get("prompt", ""))
        # wrapped labels ask for their narrowest width; give the text room for a line of ~30
        longest = max([len(x) for x in rule.get("prompt", "").split("\n")] + [len(rule.get("prompt_title", ""))])
        self.msg_text.set_width_chars(min(30, max(longest, 1)))
        self.msg_text.set_visible(bool(rule.get("prompt")))
        self.msg_at = (g.sheet, r, c)
        self.box.show()
        self.place()

    def place(self):
        if self.msg_at is None or not self.box.get_visible():
            return
        g = self.win.grid
        s, r, c = self.msg_at
        rect = self.win.book.cell_rect(s, r, c, g.params()) if s == g.sheet else None
        if rect is None:
            g.fixed.move(self.box, -1000, -1000)
            return
        x, y, w, h = rect
        a = g.area.get_allocation()
        _, nat = self.box.get_preferred_width()
        _, nath = self.box.get_preferred_height()
        bx = int(x + min(w, 40) / 2 + 4)
        by = int(y + h + 6)
        if bx + nat > a.width:
            bx = max(0, a.width - nat - 4)
        if by + nath > a.height:
            by = max(0, int(y) - nath - 6)
        g.fixed.move(self.box, bx, by)

    def _hide(self):
        self.msg_at = None
        if self.box.get_visible():
            self.box.hide()

    def message_shown(self):
        return self.box.get_visible() and self.msg_at is not None

    # ------------------------------------------------------------ circles
    def circle(self):
        win = self.win
        g = win.grid
        cells = win.book.dv_invalid(g.sheet, MAX_CIRCLES + 1)
        if len(cells) > MAX_CIRCLES:
            win.dialogs.message("This worksheet contains more than 255 invalid cells. Only 255 cells will be "
                                "circled.")
            cells = cells[:MAX_CIRCLES]
        self.circles[g.sheet] = list(cells)
        g.area.queue_draw()

    def clear_circles(self):
        self.circles.pop(self.win.grid.sheet, None)
        self.win.grid.area.queue_draw()

    def recheck_circles(self):
        """A circled cell that has been put right loses its circle."""
        g = self.win.grid
        cur = self.circles.get(g.sheet)
        if not cur:
            return
        still = set(map(tuple, self.win.book.dv_invalid(g.sheet, 100000)))
        self.circles[g.sheet] = [rc for rc in cur if tuple(rc) in still]

    def circles_for(self, sheet):
        return self.circles.get(sheet, [])


def _bold():
    attrs = Pango.AttrList()
    attrs.insert(Pango.attr_weight_new(Pango.Weight.BOLD))
    return attrs


def _is_number(t):
    try:
        float(t.replace(",", ""))
        return True
    except ValueError:
        return False
