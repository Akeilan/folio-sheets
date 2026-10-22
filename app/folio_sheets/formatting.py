"""Home ▸ Font, Alignment, Borders, Fill, Merge, Clear Formats, Format Painter and
Format Cells: the formatting commands of the workbook window (a mixin)."""
from gi.repository import GLib, Gtk

from .core import MAX_COLS, MAX_ROWS, Book
from . import restyle as R

THIN, MEDIUM, DOUBLE = "thin", "medium", "double"
SIDES = ("left", "right", "top", "bottom")


def _contains(rg, r, c):
    return rg[0] <= r <= rg[2] and rg[1] <= c <= rg[3]


def border_fn(styles, preset, color=None, style=THIN):
    """Style change for one of the Borders menu presets; positional (edges follow the
    selection's outline). r or c is None for a whole-column or whole-row style."""
    pen = (style, color)

    def edges(r, c, rg):
        r1, c1, r2, c2 = rg
        return (c is not None and c == c1, c is not None and c == c2,
                r is not None and r == r1, r is not None and r == r2)

    def fn(xf, r, c, rg):
        L, Rt, T, B = edges(r, c, rg)
        d = {}
        if preset == "none":
            d = {s: None for s in SIDES}
            d.update({"diagonal": None, "diagonalUp": False, "diagonalDown": False})
        elif preset == "all":
            d = {s: pen for s in SIDES}
        elif preset in ("outside", "thick-outside"):
            p = (MEDIUM, color) if preset == "thick-outside" else pen
            for side, on in zip(SIDES, (L, Rt, T, B)):
                if on:
                    d[side] = p
        elif preset == "bottom":
            if B:
                d["bottom"] = pen
        elif preset == "top":
            if T:
                d["top"] = pen
        elif preset == "left":
            if L:
                d["left"] = pen
        elif preset == "right":
            if Rt:
                d["right"] = pen
        elif preset == "bottom-double":
            if B:
                d["bottom"] = (DOUBLE, color)
        elif preset == "bottom-thick":
            if B:
                d["bottom"] = (MEDIUM, color)
        elif preset == "top-bottom":
            if T:
                d["top"] = pen
            if B:
                d["bottom"] = pen
        elif preset == "top-thick-bottom":
            if T:
                d["top"] = pen
            if B:
                d["bottom"] = (MEDIUM, color)
        elif preset == "top-double-bottom":
            if T:
                d["top"] = pen
            if B:
                d["bottom"] = (DOUBLE, color)
        if not d:
            return xf
        return styles.derive(xf, {"border": d})
    return fn


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
            while changed and not (R.whole_cols(rg) or R.whole_rows(rg)):
                changed = False
                for m in merges:
                    m = tuple(m)
                    if R.intersects((r1, c1, r2, c2), m) and not (r1 <= m[0] and c1 <= m[1] and r2 >= m[2] and c2 >= m[3]):
                        r1, c1, r2, c2 = min(r1, m[0]), min(c1, m[1]), max(r2, m[2]), max(c2, m[3])
                        changed = True
            out.append((r1, c1, r2, c2))
        return out

    def restyle(self, label, edit, post=None):
        """Run edit(pkg, sheet) on the workbook file and make the result current, as one
        undoable step. post(book) may make engine edits on the new book that belong to
        the same step (contents a merge discards). Returns True when something changed."""
        g = self.grid
        if g.editing() and g.commit_edit() is False:
            return False
        if self.book.sheet_info(g.sheet)["kind"] != "work":
            self.dialogs.message("This command works on worksheets only.")
            return False
        self.status.extra.set_text("")
        try:
            self.store_view_state()
            pkg = R.Package(self.book.snapshot())
            sheet = pkg.sheet(g.sheet)
            if edit(pkg, sheet) is False:
                return False
            if not (pkg.styles.changed or sheet.changed):
                return False
            book = Book.from_bytes(pkg.to_bytes())
        except ValueError as e:
            self.dialogs.message(str(e))
            return False
        try:
            book.active_sheet = g.sheet
        except Exception:
            pass
        if post:
            post(book)
        self.book.push(book, label)
        g.refresh()
        self.update_title()
        return True

    def apply_style(self, label, change, ranges=None):
        """One style change (a derive() dict, or fn(styles) -> fn(xf, r, c, rg))
        over the selection."""
        ranges = ranges or self.style_ranges()

        def edit(pkg, sh):
            if callable(change):
                sh.apply(ranges, change(pkg.styles), positional=True)
            else:
                sh.apply(ranges, lambda xf, r, c, rg: pkg.styles.derive(xf, change))
        return self.restyle(label, edit)

    def active_style(self):
        g = self.grid
        r, c = g.sel.active
        return self.book.cell_style(g.sheet, r, c)

    def active_spec(self):
        """Everything Format Cells needs about the active cell (reads the file)."""
        g = self.grid
        r, c = g.sel.active
        pkg = R.Package(self.book.snapshot())
        sh = pkg.sheet(g.sheet)
        spec = pkg.styles.spec(sh.cell_xf(r, c))
        spec["merged"] = any(_contains(m, r, c) and m[0:2] != m[2:4] for m in sh.merges())
        return spec

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
        self.apply_style("Fill Color", {"fill": {"pattern": "solid", "fg": c} if c is not None else None})

    # ------------------------------------------------------------ borders
    def _border(self, preset, label):
        ranges = self.style_ranges()
        pen = getattr(self, "border_pen", (THIN, None))

        def edit(pkg, sh):
            fn = border_fn(pkg.styles, preset, pen[1], pen[0])
            sh.apply(ranges, fn, positional=True)
            if preset == "none":
                # the cells around lose the edges they share with the selection
                clear = {"bottom": None}
                for rg in ranges:
                    r1, c1, r2, c2 = rg
                    nb = []
                    if r1 > 0 and not R.whole_cols(rg):
                        nb.append(((r1 - 1, c1, r1 - 1, c2), {"bottom": None}))
                    if r2 < MAX_ROWS - 1 and not R.whole_cols(rg):
                        nb.append(((r2 + 1, c1, r2 + 1, c2), {"top": None}))
                    if c1 > 0 and not R.whole_rows(rg):
                        nb.append(((r1, c1 - 1, r2, c1 - 1), {"right": None}))
                    if c2 < MAX_COLS - 1 and not R.whole_rows(rg):
                        nb.append(((r1, c2 + 1, r2, c2 + 1), {"left": None}))
                    for nrg, clear in nb:
                        for r in range(nrg[0], min(nrg[2], nrg[0] + 100000) + 1):
                            for c in range(nrg[1], nrg[3] + 1):
                                xf = sh.cell_xf(r, c)
                                if xf and sh.cells(r).get(c) is not None:
                                    sh.set_cell_xf(r, c, pkg.styles.derive(xf, {"border": clear}))
        self.restyle(label, edit)

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
            self.status.extra.set_text("Borders now draw in #%06X." % c)

    def act_border_line_style(self, style=None):
        pen = getattr(self, "border_pen", (THIN, None))
        if style is None:
            from .format_cells import pick_line_style
            style = pick_line_style(self, pen[0])
            if style is None:
                return
        self.border_pen = (style, pen[1])
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
            if R.whole_cols(rg) or R.whole_rows(rg):
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

        def edit(pkg, sh):
            keep = [m for m in sh.merges() if not any(R.intersects(m, t) for t in targets)]
            for t in targets:
                top = sh.cell_xf(t[0], t[1])
                if center:
                    top = pkg.styles.derive(top, {"align": {"horizontal": "center"}})
                sh.apply([t], lambda xf, rr, cc, rg, top=top: top)
            sh.set_merges(keep + targets)

        post = (lambda book: book.clear_contents(g.sheet, discard)) if discard else None
        if self.restyle(label, edit, post):
            g.sel.ranges = [tuple(rg) for rg in ranges]
            g.refresh()

    def act_merge_center(self):
        self._merge("Merge & Center", center=True)

    def act_merge_across(self):
        self._merge("Merge Across", across=True)

    def act_merge_cells(self):
        self._merge("Merge Cells")

    def act_unmerge(self):
        ranges = self.style_ranges()

        def edit(pkg, sh):
            before = sh.merges()
            keep = [m for m in before if not any(R.intersects(m, rg) for rg in ranges)]
            if len(keep) == len(before):
                return False
            sh.set_merges(keep)
        self.restyle("Unmerge Cells", edit)

    # ------------------------------------------------------------ clear
    def act_clear_formats(self):
        ranges = self.style_ranges()

        def edit(pkg, sh):
            sh.apply(ranges, lambda xf, r, c, rg: 0)
            keep = [m for m in sh.merges() if not any(R.intersects(m, rg) for rg in ranges)]
            if len(keep) != len(sh.merges()):
                sh.set_merges(keep)
        self.restyle("Clear Formats", edit)

    def act_clear_all(self):
        g = self.grid
        ranges = self.style_ranges()

        def edit(pkg, sh):
            sh.apply(ranges, lambda xf, r, c, rg: 0)
            keep = [m for m in sh.merges() if not any(R.intersects(m, rg) for rg in ranges)]
            if len(keep) != len(sh.merges()):
                sh.set_merges(keep)
            sh.changed = True
        self.restyle("Clear All", edit, post=lambda book: book.clear_contents(g.sheet, ranges))

    # ------------------------------------------------------------ protection
    def act_lock_cell(self):
        spec = self.active_spec()
        self.apply_style("Lock Cell", {"protection": {"locked": not spec["protection"]["locked"]}})

    # ------------------------------------------------------------ Format Painter / paste formats
    def act_format_painter(self):
        g = self.grid
        if self.painter:
            self.painter = None
            self.status.extra.set_text("")
            return
        rg = g.sel.ranges[-1]
        if R.whole_cols(rg) or R.whole_rows(rg):
            rg = (rg[0], rg[1], min(rg[2], rg[0] + 999), min(rg[3], rg[1] + 255))
        pkg = R.Package(self.book.snapshot())
        sh = pkg.sheet(g.sheet)
        self.painter = {"specs": [[pkg.styles.spec(x) for x in row] for row in sh.xf_grid(rg)],
                        "xml": self._xf_xml(pkg, sh, rg), "merges": [m for m in sh.merges() if R.intersects(m, rg)],
                        "src": rg}
        self.status.extra.set_text("Select the cells to paste the formatting to (Esc cancels).")

    @staticmethod
    def _xf_xml(pkg, sh, rg):
        """Self-contained copies of a block's styles, so they can go into another workbook state."""
        import copy
        st = pkg.styles
        out = []
        for row in sh.xf_grid(rg):
            line = []
            for xi in row:
                xf = st._get(st.xfs, xi)
                line.append({"xf": copy.deepcopy(xf), "font": copy.deepcopy(st._get(st.fonts, xf.get("fontId"))),
                             "fill": copy.deepcopy(st._get(st.fills, xf.get("fillId"))),
                             "border": copy.deepcopy(st._get(st.borders, xf.get("borderId"))),
                             "numfmt": st.numfmt_code(xf.get("numFmtId"))})
            out.append(line)
        return out

    @staticmethod
    def _import_xf(st, item, cache):
        k = id(item)
        if k in cache:
            return cache[k]
        import copy
        xf = copy.deepcopy(item["xf"])
        xf.set("fontId", str(st._intern("font", st.fonts, copy.deepcopy(item["font"]))))
        xf.set("fillId", str(st._intern("fill", st.fills, copy.deepcopy(item["fill"]))))
        xf.set("borderId", str(st._intern("border", st.borders, copy.deepcopy(item["border"]))))
        xf.set("numFmtId", str(st._numfmt_id(item["numfmt"])))
        cache[k] = st._intern("xf", st.xfs, xf)
        return cache[k]

    def paste_formats(self, src, label):
        """Tile a copied block of styles (and its merges) over the selection."""
        g = self.grid
        grid = src["xml"]
        sr = src["src"]
        h, w = len(grid), len(grid[0])
        ranges = []
        for rg in g.sel.ranges:
            r1, c1, r2, c2 = rg
            # a single cell (or smaller area) takes the copied block's size
            if (r2 - r1 + 1) < h or (c2 - c1 + 1) < w:
                r2, c2 = r1 + h - 1, c1 + w - 1
            ranges.append((r1, c1, min(r2, MAX_ROWS - 1), min(c2, MAX_COLS - 1)))
        cache = {}

        def edit(pkg, sh):
            st = pkg.styles
            sh.apply(ranges, lambda xf, r, c, rg: self._import_xf(st, grid[(r - rg[0]) % h if r is not None else 0]
                                                                     [(c - rg[1]) % w if c is not None else 0], cache))
            new = []
            for rg in ranges:
                for m in src["merges"]:
                    for dr in range(0, rg[2] - rg[0] + 1, h):
                        for dc in range(0, rg[3] - rg[1] + 1, w):
                            nm = (m[0] - sr[0] + rg[0] + dr, m[1] - sr[1] + rg[1] + dc,
                                  m[2] - sr[0] + rg[0] + dr, m[3] - sr[1] + rg[1] + dc)
                            if nm[2] <= rg[2] and nm[3] <= rg[3]:
                                new.append(nm)
            keep = [m for m in sh.merges() if not any(R.intersects(m, rg) for rg in ranges)]
            if new or len(keep) != len(sh.merges()):
                sh.set_merges(keep + new)
        if self.restyle(label, edit):
            g.sel.ranges = ranges
            g.refresh()

    def painter_finish(self):
        if not self.painter:
            return
        src, self.painter = self.painter, None
        self.status.extra.set_text("")
        self.paste_formats(src, "Format Painter")

    def act_paste_formatting(self):
        cp = getattr(self, "_copied", None)
        if not cp:
            return
        sheet, rg, cut = cp
        if R.whole_cols(rg) or R.whole_rows(rg):
            rg = (rg[0], rg[1], min(rg[2], rg[0] + 999), min(rg[3], rg[1] + 255))
        pkg = R.Package(self.book.snapshot())
        sh = pkg.sheet(sheet)
        self.paste_formats({"xml": self._xf_xml(pkg, sh, rg), "merges": [m for m in sh.merges() if R.intersects(m, rg)],
                            "src": rg}, "Paste Formatting")

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

        def edit(pkg, sh):
            st = pkg.styles
            if changes:
                sh.apply(ranges, lambda xf, rr, cc, rg: st.derive(xf, changes))
            if border:
                sh.apply(ranges, lambda xf, rr, cc, rg: st.derive(xf, {"border": border_layout(border, rr, cc, rg)}),
                         positional=True)
            if merge is not None:
                if merge:
                    keep = [m for m in sh.merges() if not any(R.intersects(m, rg) for rg in ranges)]
                    sh.set_merges(keep + [rg for rg in ranges if rg[0:2] != rg[2:4]])
                else:
                    keep = [m for m in sh.merges() if not any(R.intersects(m, rg) for rg in ranges)]
                    if len(keep) != len(sh.merges()):
                        sh.set_merges(keep)
        self.restyle("Format Cells", edit)

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


def border_layout(lay, r, c, rg):
    """Format Cells ▸ Border: outline edges for cells on the selection's outline, the
    inside lines between cells. lay maps left/right/top/bottom/insideV/insideH/diagUp/diagDown
    to a (style, color) pen, None (remove) or absent (leave)."""
    r1, c1, r2, c2 = rg
    d = {}

    def put(side, key):
        if key in lay:
            d[side] = lay[key]
    put("left", "left" if (c is None or c == c1) else "insideV")
    put("right", "right" if (c is None or c == c2) else "insideV")
    put("top", "top" if (r is None or r == r1) else "insideH")
    put("bottom", "bottom" if (r is None or r == r2) else "insideH")
    if "diagUp" in lay or "diagDown" in lay:
        up, down = lay.get("diagUp"), lay.get("diagDown")
        pen = up or down
        d["diagonal"] = pen
        d["diagonalUp"] = bool(up)
        d["diagonalDown"] = bool(down)
    return d
