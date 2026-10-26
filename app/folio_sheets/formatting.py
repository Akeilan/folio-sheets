"""Home ▸ Font, Alignment, Borders, Fill, Merge, Clear Formats, Format Painter and
Format Cells: the formatting commands of the workbook window (a mixin)."""
from gi.repository import GLib, Gtk

from .core import MAX_COLS, MAX_ROWS


def whole_cols(rg):
    return rg[0] == 0 and rg[2] >= MAX_ROWS - 1


def whole_rows(rg):
    return rg[1] == 0 and rg[3] >= MAX_COLS - 1


def intersects(a, b):
    return not (a[2] < b[0] or b[2] < a[0] or a[3] < b[1] or b[3] < a[1])

THIN, MEDIUM, DOUBLE = "thin", "medium", "double"
SIDES = ("left", "right", "top", "bottom")


def _contains(rg, r, c):
    return rg[0] <= r <= rg[2] and rg[1] <= c <= rg[3]


BORDER_PRESETS = ["bottom", "top", "left", "right", "none", "all", "outside", "thick-outside", "bottom-double",
                  "bottom-thick", "top-bottom", "top-thick-bottom", "top-double-bottom"]


class FormattingMixin:
    last_fill = 0xFFFF00
    last_font_color = 0xFF0000
    painter = None

    # ------------------------------------------------------------ the mechanism
    def style_ranges(self):
        """The selection, with merged areas it touches taken in whole."""
        g = self.grid
        out = []
        merges = self.book.merges(g.sheet) or []
        for rg in g.sel.ranges:
            r1, c1, r2, c2 = rg
            changed = True
            while changed and not (whole_cols(rg) or whole_rows(rg)):
                changed = False
                for m in merges:
                    m = tuple(m)
                    if intersects((r1, c1, r2, c2), m) and not (r1 <= m[0] and c1 <= m[1] and r2 >= m[2] and c2 >= m[3]):
                        r1, c1, r2, c2 = min(r1, m[0]), min(c1, m[1]), max(r2, m[2]), max(c2, m[3])
                        changed = True
            out.append((r1, c1, r2, c2))
        return out

    def _ready(self):
        g = self.grid
        if g.editing() and g.commit_edit() is False:
            return False
        if self.book.sheet_info(g.sheet)["kind"] != "work":
            self.dialogs.message("This command works on worksheets only.")
            return False
        self.status.extra.set_text("")
        return True

    def restyle(self, label, ops, ranges=None):
        """Formatting operations (see Book.restyle) over the selection, as one undo
        step. Returns True when something changed."""
        if not self._ready():
            return False
        g = self.grid
        ranges = ranges or self.style_ranges()
        from .protect_ui import may_format, protected
        if protected(self):
            if not may_format(self, ranges):
                return False
            self._approved = True
        try:
            changed = self.book.restyle(g.sheet, [tuple(r) for r in ranges], ops, label)
        except ValueError as e:
            self.dialogs.message(str(e))
            return False
        g.refresh()
        self.update_title()
        return changed

    def apply_style(self, label, change, ranges=None):
        """One style change (a derive() dict, or an operation tuple) over the selection."""
        return self.restyle(label, [change], ranges)

    def active_style(self):
        g = self.grid
        r, c = g.sel.active
        return self.book.cell_style(g.sheet, r, c)

    def active_spec(self):
        """Everything Format Cells needs about the active cell."""
        g = self.grid
        r, c = g.sel.active
        return self.book.style_spec(g.sheet, r, c)

    # ------------------------------------------------------------ font
    def _toggle_font(self, label, key, on_value=True):
        cur = self.active_style().get(key)
        on = not cur if key != "underline" else (not cur)
        self.apply_style(label, {"font": {key: (on_value if on else (None if key == "underline" else False))}})

    def act_bold(self):
        self._toggle_font("Bold", "bold")

    def act_italic(self):
        self._toggle_font("Italic", "italic")

    def act_underline(self):
        self._toggle_font("Underline", "underline", "single")

    def act_double_underline(self):
        st = self.active_style()
        self.apply_style("Double Underline", {"font": {"underline": None if st.get("underline") == 2 else "double"}})

    def act_strikethrough(self):
        self._toggle_font("Strikethrough", "strike")

    def act_font_name(self, name):
        name = (name or "").strip()
        if name:
            self.apply_style("Font", {"font": {"name": name}})
        self.focus_document()

    def act_font_size(self, v):
        try:
            size = float(str(v).strip())
        except ValueError:
            self.dialogs.message("This isn't a valid font size. Type a number between 1 and 409.")
            return
        if not 1 <= size <= 409:
            self.dialogs.message("The font size must be between 1 and 409.")
            return
        size = round(size * 2) / 2
        self.apply_style("Font Size", {"font": {"size": size}})
        self.focus_document()

    def act_font_grow(self):
        self.apply_style("Increase Font Size", {"font": {"grow": 1}})

    def act_font_shrink(self):
        self.apply_style("Decrease Font Size", {"font": {"grow": -1}})

    def _pick_color(self, title, initial):
        d = Gtk.ColorChooserDialog(title=title, transient_for=self)
        from gi.repository import Gdk
        rgba = Gdk.RGBA()
        rgba.parse("#%06X" % (initial or 0))
        d.set_rgba(rgba)
        d.set_use_alpha(False)
        r = d.run()
        c = d.get_rgba()
        d.destroy()
        if r != Gtk.ResponseType.OK:
            return None
        return (round(c.red * 255) << 16) | (round(c.green * 255) << 8) | round(c.blue * 255)

    def act_font_color(self, arg="last"):
        if arg == "more":
            c = self._pick_color("Colors", self.last_font_color)
            if c is None:
                return
        elif arg == "auto":
            c = None
        elif arg == "last":
            c = self.last_font_color
        else:
            c = int(arg, 16)
        if c is not None:
            self.last_font_color = c
            self.show_last_colors()
        self.apply_style("Font Color", {"font": {"color": c}})

    def act_fill_color(self, arg="last"):
        if arg == "more":
            c = self._pick_color("Colors", self.last_fill)
            if c is None:
                return
        elif arg == "none":
            c = None
        elif arg == "last":
            c = self.last_fill
        else:
            c = int(arg, 16)
        if c is not None:
            self.last_fill = c
            self.show_last_colors()
        self.apply_style("Fill Color", {"fill": {"pattern": "solid", "fg": c} if c is not None else None})

    # ------------------------------------------------------------ borders
    def _border(self, preset, label):
        pen = getattr(self, "border_pen", (THIN, None))
        self.set_border_mode(None)
        self.last_border = (preset, label)
        self.ribbon_border_remembered()
        self.restyle(label, [("preset", preset, pen[0], pen[1])])

    def show_last_colors(self):
        """The Fill Color and Font Color buttons show the colour they will apply."""
        for btn in getattr(self, "color_buttons", []):
            btn.set_swatch(self.last_fill if btn.action.startswith("fill-color") else self.last_font_color)

    def act_border_last(self):
        """The Borders button: the border picked last (Bottom Border at first)."""
        preset, label = getattr(self, "last_border", ("bottom", "Bottom Border"))
        self._border(preset, label)

    def ribbon_border_remembered(self):
        preset, label = getattr(self, "last_border", ("bottom", "Bottom Border"))
        btn = getattr(self, "borders_button", None)
        if btn is not None:
            btn.set_tooltip_text("Borders: %s" % label)

    # ---- Draw Border, Draw Border Grid, Erase Border: a pen over the cells
    BORDER_MODES = {"draw": ("outside", "Draw Border"), "grid": ("all", "Draw Border Grid"),
                    "erase": ("none", "Erase Border")}

    def set_border_mode(self, mode):
        """None, "draw", "grid" or "erase"; the grid then draws borders where it is dragged."""
        if getattr(self, "border_mode", None) == mode:
            return
        self.border_mode = mode
        g = self.grid
        g.border_drag = None
        g.area.queue_draw()
        if mode is None:
            self.status.extra.set_text("")
        else:
            self.status.extra.set_text("%s: drag over the cells. Esc to stop." % self.BORDER_MODES[mode][1])

    def apply_border_pen(self, rg):
        mode = getattr(self, "border_mode", None)
        if mode is None:
            return
        preset, label = self.BORDER_MODES[mode]
        pen = getattr(self, "border_pen", (THIN, None))
        self.restyle(label, [("preset", preset, pen[0], pen[1])], [tuple(rg)])

    def act_border_draw(self):
        self.set_border_mode(None if getattr(self, "border_mode", None) == "draw" else "draw")

    def act_border_draw_grid(self):
        self.set_border_mode(None if getattr(self, "border_mode", None) == "grid" else "grid")

    def act_border_erase(self):
        self.set_border_mode(None if getattr(self, "border_mode", None) == "erase" else "erase")

    def act_border_bottom(self):
        self._border("bottom", "Bottom Border")

    def act_border_top(self):
        self._border("top", "Top Border")

    def act_border_left(self):
        self._border("left", "Left Border")

    def act_border_right(self):
        self._border("right", "Right Border")

    def act_border_none(self):
        self._border("none", "No Border")

    def act_border_all(self):
        self._border("all", "All Borders")

    def act_border_outside(self):
        self._border("outside", "Outside Borders")

    def act_border_thick_outside(self):
        self._border("thick-outside", "Thick Outside Borders")

    def act_border_bottom_double(self):
        self._border("bottom-double", "Bottom Double Border")

    def act_border_bottom_thick(self):
        self._border("bottom-thick", "Thick Bottom Border")

    def act_border_top_bottom(self):
        self._border("top-bottom", "Top and Bottom Border")

    def act_border_top_thick_bottom(self):
        self._border("top-thick-bottom", "Top and Thick Bottom Border")

    def act_border_top_double_bottom(self):
        self._border("top-double-bottom", "Top and Double Bottom Border")

    def act_border_more(self):
        self.open_format_cells("border")

    def act_border_line_color(self):
        pen = getattr(self, "border_pen", (THIN, None))
        c = self._pick_color("Line Color", pen[1] or 0)
        if c is not None:
            self.border_pen = (pen[0], c)
            if getattr(self, "border_mode", None) in (None, "erase"):
                self.set_border_mode("draw")    # picking a pen picks up the pen
            self.status.extra.set_text("Borders now draw in #%06X." % c)

    def act_border_line_style(self, style=None):
        pen = getattr(self, "border_pen", (THIN, None))
        if style is None:
            from .format_cells import pick_line_style
            style = pick_line_style(self, pen[0])
            if style is None:
                return
        self.border_pen = (style, pen[1])
        if getattr(self, "border_mode", None) in (None, "erase"):
            self.set_border_mode("draw")
        self.status.extra.set_text("Borders now draw with the %s line." % style)

    # ------------------------------------------------------------ alignment
    def _align(self, label, key, value, default):
        st = self.active_style()
        cur = st.get("halign" if key == "horizontal" else "valign") or default
        if key == "horizontal" and cur == value:
            value = "general"
        self.apply_style(label, {"align": {key: value}})

    def act_align_left(self):
        self._align("Align Left", "horizontal", "left", "general")

    def act_align_center(self):
        self._align("Center", "horizontal", "center", "general")

    def act_align_right(self):
        self._align("Align Right", "horizontal", "right", "general")

    def act_align_top(self):
        self._align("Top Align", "vertical", "top", "bottom")

    def act_align_middle(self):
        self._align("Middle Align", "vertical", "center", "bottom")

    def act_align_bottom(self):
        self._align("Bottom Align", "vertical", "bottom", "bottom")

    def act_wrap_text(self):
        st = self.active_style()
        self.apply_style("Wrap Text", {"align": {"wrap": not st.get("wrap")}})

    def act_orientation(self, arg):
        v = int(arg)
        cur = self.active_spec()["align"]["rotation"]
        self.apply_style("Orientation", {"align": {"rotation": 0 if cur == v else v}})

    def act_indent_increase(self):
        self.apply_style("Increase Indent", {"align": {"indent_step": 1}})

    def act_indent_decrease(self):
        self.apply_style("Decrease Indent", {"align": {"indent_step": -1}})

    # ------------------------------------------------------------ merge
    def _merge(self, label, across=False, center=False):
        g = self.grid
        ranges = self.style_ranges()
        r, c = g.sel.active
        merges = [tuple(m) for m in (self.book.merges(g.sheet) or [])]
        # Merge & Center on a merged cell undoes the merge, like the original's toggle
        if center and not across and any(_contains(m, r, c) for m in merges) and \
                all(any(tuple(rg) == m for m in merges) for rg in ranges):
            return self.act_unmerge()
        targets = []
        for rg in ranges:
            if whole_cols(rg) or whole_rows(rg):
                if (rg[2] - rg[0] + 1) * (rg[3] - rg[1] + 1) > 4_000_000:
                    self.dialogs.message("The selection is too large to merge.")
                    return
            if across:
                targets += [(rr, rg[1], rr, rg[3]) for rr in range(rg[0], rg[2] + 1) if rg[3] > rg[1]]
            elif rg[0:2] != rg[2:4]:
                targets.append(rg)
        if not targets:
            if center:
                self.apply_style(label, {"align": {"horizontal": "center"}})
            return
        # values other than each area's upper-left one are discarded (after a warning)
        discard = []
        ext = self.book.used_extent(g.sheet)
        for t in targets:
            if ext is None:
                break
            for rr in range(t[0], min(t[2], ext[0]) + 1):
                for cc in range(t[1], min(t[3], ext[1]) + 1):
                    if (rr, cc) != (t[0], t[1]) and self.book.has_content(g.sheet, rr, cc):
                        discard.append((rr, cc, rr, cc))
        if discard:
            d = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.WARNING,
                                  buttons=Gtk.ButtonsType.OK_CANCEL,
                                  text="Merging cells only keeps the upper-left value and discards other values.")
            d.set_title("Folio Sheets")
            ok = d.run() == Gtk.ResponseType.OK
            d.destroy()
            if not ok:
                return

        if not self._ready():
            return
        keep = [m for m in merges if not any(intersects(m, t) for t in targets)]
        with self.book.group(label):
            ops_sheet = g.sheet
            for t in targets:
                top = self.book.xf_grid(ops_sheet, (t[0], t[1], t[0], t[1]))[0][0]
                ops = [("set", top)]
                if center:
                    ops.append({"align": {"horizontal": "center"}})
                self.book.restyle(ops_sheet, [t], ops, label)
            self.book.set_merges(ops_sheet, keep + targets, label)
            if discard:
                self.book.clear_contents(ops_sheet, discard, label)
        g.sel.ranges = [tuple(rg) for rg in ranges]
        g.refresh()
        self.update_title()

    def act_merge_center(self):
        self._merge("Merge & Center", center=True)

    def act_merge_across(self):
        self._merge("Merge Across", across=True)

    def act_merge_cells(self):
        self._merge("Merge Cells")

    def act_unmerge(self):
        if not self._ready():
            return
        g = self.grid
        ranges = self.style_ranges()
        before = [tuple(m) for m in (self.book.merges(g.sheet) or [])]
        keep = [m for m in before if not any(intersects(m, rg) for rg in ranges)]
        if len(keep) == len(before):
            return
        self.book.set_merges(g.sheet, keep, "Unmerge Cells")
        g.refresh()
        self.update_title()

    # ------------------------------------------------------------ clear
    def _clear_formats(self, label, contents=False):
        if not self._ready():
            return
        g = self.grid
        ranges = self.style_ranges()
        before = [tuple(m) for m in (self.book.merges(g.sheet) or [])]
        keep = [m for m in before if not any(intersects(m, rg) for rg in ranges)]
        try:
            with self.book.group(label):
                self.book.restyle(g.sheet, ranges, [("set", 0)], label)
                if len(keep) != len(before):
                    self.book.set_merges(g.sheet, keep, label)
                if contents:
                    self.book.clear_contents(g.sheet, ranges, label)
        except ValueError as e:
            self.dialogs.message(str(e))
        g.refresh()
        self.update_title()

    def act_clear_formats(self):
        self._clear_formats("Clear Formats")

    def act_clear_all(self):
        # the cells' links go with everything else, in the same undo step
        self.book.clear_links(self.grid.sheet, [tuple(r) for r in self.grid.sel.ranges])
        self._clear_formats("Clear All", contents=True)

    def act_clear_hyperlinks(self):
        self.links.clear(remove_format=False)

    def act_remove_hyperlinks(self):
        self.links.clear(remove_format=True)

    def act_link(self):
        self.links.insert()

    def act_open_hyperlink(self):
        self.links.follow()

    # ------------------------------------------------------------ protection
    def act_protect_sheet(self):
        from . import protect_ui
        protect_ui.protect_sheet(self)

    def act_unprotect_sheet(self):
        from . import protect_ui
        if protect_ui.protected(self):
            protect_ui.unprotect_sheet(self)

    def act_protect_workbook(self):
        from . import protect_ui
        protect_ui.protect_workbook(self)

    def act_lock_cell(self):
        spec = self.active_spec()
        self.apply_style("Lock Cell", {"protection": {"locked": not spec["protection"]["locked"]}})

    # ------------------------------------------------------------ Format Painter / paste formats
    def _copy_formats(self, sheet, rg):
        if whole_cols(rg) or whole_rows(rg):
            rg = (rg[0], rg[1], min(rg[2], rg[0] + 999), min(rg[3], rg[1] + 255))
        return {"grid": self.book.xf_grid(sheet, rg),
                "merges": [tuple(m) for m in (self.book.merges(sheet) or []) if intersects(m, rg)], "src": rg}

    def act_format_painter(self):
        g = self.grid
        if self.painter:
            self.painter = None
            self.status.extra.set_text("")
            return
        self.painter = self._copy_formats(g.sheet, g.sel.ranges[-1])
        self.status.extra.set_text("Select the cells to paste the formatting to (Esc cancels).")

    def paste_formats(self, src, label):
        """Tile a copied block of styles (and its merges) over the selection."""
        g = self.grid
        grid = src["grid"]
        sr = src["src"]
        h, w = len(grid), len(grid[0])
        ranges = []
        for rg in g.sel.ranges:
            r1, c1, r2, c2 = rg
            # a single cell (or smaller area) takes the copied block's size
            if (r2 - r1 + 1) < h or (c2 - c1 + 1) < w:
                r2, c2 = r1 + h - 1, c1 + w - 1
            ranges.append((r1, c1, min(r2, MAX_ROWS - 1), min(c2, MAX_COLS - 1)))
        if not self._ready():
            return
        new = []
        for rg in ranges:
            for m in src["merges"]:
                for dr in range(0, rg[2] - rg[0] + 1, h):
                    for dc in range(0, rg[3] - rg[1] + 1, w):
                        nm = (m[0] - sr[0] + rg[0] + dr, m[1] - sr[1] + rg[1] + dc,
                              m[2] - sr[0] + rg[0] + dr, m[3] - sr[1] + rg[1] + dc)
                        if nm[2] <= rg[2] and nm[3] <= rg[3]:
                            new.append(nm)
        before = [tuple(m) for m in (self.book.merges(g.sheet) or [])]
        keep = [m for m in before if not any(intersects(m, rg) for rg in ranges)]
        try:
            with self.book.group(label):
                self.book.restyle(g.sheet, ranges, [("grid", grid)], label)
                if new or len(keep) != len(before):
                    self.book.set_merges(g.sheet, keep + new, label)
        except ValueError as e:
            self.dialogs.message(str(e))
            return
        g.sel.ranges = ranges
        g.refresh()
        self.update_title()

    def painter_finish(self):
        if not self.painter:
            return
        src, self.painter = self.painter, None
        self.status.extra.set_text("")
        self.paste_formats(src, "Format Painter")

    def act_paste_formatting(self):
        from . import clip_ui
        clip_ui.paste(self, {"what": "formats"})

    # ------------------------------------------------------------ dialogs
    def open_format_cells(self, page="number"):
        from .format_cells import FormatCells
        g = self.grid
        if g.editing() and g.commit_edit() is False:
            return
        if self.book.sheet_info(g.sheet)["kind"] != "work":
            return
        spec = self.active_spec()
        r, c = g.sel.active
        dlg = FormatCells(self, spec, page, self.book.value(g.sheet, r, c))
        changes = dlg.run_dialog()
        if not changes:
            return
        ranges = self.style_ranges()
        border = changes.pop("border_layout", None)
        merge = changes.pop("merge", None)

        ops = []
        if changes:
            ops.append(changes)
        if border:
            ops.append(("layout", border))
        if not self._ready():
            return
        before = [tuple(m) for m in (self.book.merges(g.sheet) or [])]
        keep = [m for m in before if not any(intersects(m, rg) for rg in ranges)]
        try:
            with self.book.group("Format Cells"):
                if ops:
                    self.book.restyle(g.sheet, ranges, ops, "Format Cells")
                if merge is not None:
                    if merge:
                        self.book.set_merges(g.sheet, keep + [rg for rg in ranges if rg[0:2] != rg[2:4]], "Format Cells")
                    elif len(keep) != len(before):
                        self.book.set_merges(g.sheet, keep, "Format Cells")
        except ValueError as e:
            self.dialogs.message(str(e))
        g.refresh()
        self.update_title()

    def act_format_cells(self):
        self.open_format_cells("number")

    def act_font_dialog(self):
        self.open_format_cells("font")

    def act_alignment_dialog(self):
        self.open_format_cells("alignment")

    def act_number_dialog(self):
        self.open_format_cells("number")

    def act_cell_styles(self):
        from .format_cells import pick_cell_style
        ch = pick_cell_style(self)
        if ch:
            name, change = ch
            self.apply_style("Cell Style: " + name, change)
