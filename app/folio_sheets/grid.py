"""The worksheet grid: drawing (by the native engine), selection with the
mouse and keyboard, scrolling, and the in-cell editor."""
import time

from gi.repository import Gdk, GLib, Gtk, Pango

from .core import MAX_COLS, MAX_ROWS
from .links_ui import HOLD_TO_SELECT
from .selection import Selection, contains

WHEEL_ROWS = 3


class SheetState:
    """What the window remembers per sheet while the workbook is open."""
    def __init__(self, info):
        self.top_row = info.get("top_row", 0)
        self.left_col = info.get("left_col", 0)
        self.zoom = (info.get("zoom") or 100) / 100.0
        self.freeze_rows = info.get("freeze_rows", 0)
        self.freeze_cols = info.get("freeze_cols", 0)
        # a split window: the top/left panes show rows pane_top..freeze_rows and
        # columns pane_left..freeze_cols, scrolling on their own
        self.split = False
        self.pane_top = 0
        self.pane_left = 0
        self.show_grid = info.get("show_grid", True)
        self.show_headers = info.get("show_headers", True)
        self.view_mode = info.get("view") or "normal"
        sel = info.get("sel") or []
        act = tuple(info.get("active", (0, 0)))
        self.sel = Selection([tuple(s) for s in sel], act)


class CellEditor(Gtk.TextView):
    """The in-cell editor. It grows to the right over neighbouring cells as
    the text gets longer, like the original."""
    def __init__(self, grid):
        Gtk.TextView.__init__(self)
        self.grid = grid
        self.get_style_context().add_class("folio-celleditor")
        self.set_wrap_mode(Gtk.WrapMode.NONE)
        self.set_left_margin(2)
        self.set_right_margin(2)
        self.set_top_margin(0)
        self.set_bottom_margin(0)
        self.set_accepts_tab(False)
        self.cell = None
        self.mode = "enter"          # 'enter' (arrows commit) or 'edit'
        self.base = (0, 0, 64, 20)
        self._css = Gtk.CssProvider()
        self.get_style_context().add_provider(self._css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 2)
        self.get_buffer().connect("changed", self._changed)
        self._quiet = False

    def text(self):
        b = self.get_buffer()
        return b.get_text(b.get_start_iter(), b.get_end_iter(), True)

    def set_text(self, t, quiet=True):
        self._quiet = quiet
        self.get_buffer().set_text(t)
        self._quiet = False
        self.get_buffer().place_cursor(self.get_buffer().get_end_iter())

    def style_for(self, st, zoom):
        size = st.get("size", 11) * zoom
        fam = st.get("font", "Calibri")
        weight = "bold" if st.get("bold") else "normal"
        style = "italic" if st.get("italic") else "normal"
        color = st.get("color")
        fill = st.get("fill")
        css = ("textview, textview text { font-family: \"%s\"; font-size: %.1fpt; font-weight: %s; "
               "font-style: %s; color: %s; background: %s; }" % (
                   fam, size, weight, style,
                   "#%06x" % color if color is not None else "#000000",
                   "#%06x" % fill if fill is not None else "#ffffff"))
        self._css.load_from_data(css.encode())

    def _changed(self, buf):
        if self._quiet:
            return
        self.grid.editor_changed()


class GridView(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.HORIZONTAL)
        self.win = win
        self.book = None
        self.sheet = 0
        self.states = {}
        self.copy_range = None
        self.ants = 0.0
        self._ants_id = 0
        self.end_mode = False
        self.tab_origin = None        # column a run of Tabs started in
        self.extend_mode = False      # F8
        self.add_mode = False         # Shift+F8
        self._drag = None
        self.border_drag = None
        self.sel_objects = []         # selected charts (indexes on this sheet)
        self.hover_note = None
        self.edit_note = None
        self._hover_card = 0
        self.obj_preview = None
        self.edit_sheet = None        # the sheet of the cell being edited
        self._autoscroll_id = 0
        self._last_motion = None

        self.overlay = Gtk.Overlay()
        self.area = Gtk.DrawingArea()
        self.area.set_can_focus(True)
        self.area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK |
                             Gdk.EventMask.POINTER_MOTION_MASK | Gdk.EventMask.SCROLL_MASK |
                             Gdk.EventMask.SMOOTH_SCROLL_MASK | Gdk.EventMask.KEY_PRESS_MASK)
        self.area.connect("draw", self.on_draw)
        self.area.connect("button-press-event", self.on_press)
        self.area.connect("button-release-event", self.on_release)
        self.area.connect("motion-notify-event", self.on_motion)
        self.area.connect("scroll-event", self.on_scroll)
        self.area.connect("size-allocate", lambda *_: GLib.idle_add(self.update_scrollbars))
        self.overlay.add(self.area)
        self.fixed = Gtk.Fixed()
        self.overlay.add_overlay(self.fixed)
        self.overlay.set_overlay_pass_through(self.fixed, True)
        self.editor = CellEditor(self)
        self.editor.connect("key-press-event", self.on_editor_key)
        self.editor.set_no_show_all(True)
        self.fixed.put(self.editor, 0, 0)
        self.pack_start(self.overlay, True, True, 0)

        self.vadj = Gtk.Adjustment(value=0, lower=0, upper=100, step_increment=1, page_increment=20,
                                   page_size=20)
        self.vbar = Gtk.Scrollbar(orientation=Gtk.Orientation.VERTICAL, adjustment=self.vadj)
        self.vadj.connect("value-changed", self.on_vscroll)
        self.pack_start(self.vbar, False, False, 0)
        self.hadj = Gtk.Adjustment(value=0, lower=0, upper=100, step_increment=1, page_increment=10,
                                   page_size=10)
        self.hadj.connect("value-changed", self.on_hscroll)
        self._sync = False

    # ------------------------------------------------------------ model
    def set_book(self, book):
        self.book = book
        self.states = {}
        self.sheet = book.active_sheet
        self.copy_range = None
        self.sel_objects = []
        self.cancel_edit()
        self.refresh()

    def state(self, sheet=None):
        s = self.sheet if sheet is None else sheet
        if s not in self.states:
            self.states[s] = SheetState(self.book.sheet_info(s))
        return self.states[s]

    @property
    def sel(self):
        return self.state().sel

    def set_sheet(self, i):
        if self.editing():
            if self.win.formula_mode():
                # pointing into another sheet: the edit continues in the
                # formula bar
                self.sheet = i
                self.book.active_sheet = i
                self.place_editor()
                self.win.formula_bar.view.grab_focus()
                self.refresh()
                return
            if self.commit_edit() is False:
                return
        self.sheet = i
        self.book.active_sheet = i
        self.copy_range = None
        self.sel_objects = []
        self.refresh()

    def params(self, extra=None):
        st = self.state()
        a = self.area.get_allocation()
        p = {
            "width": float(max(a.width, 1)), "height": float(max(a.height, 1)), "zoom": st.zoom,
            "top_row": st.top_row, "left_col": st.left_col,
            "freeze_rows": st.freeze_rows, "freeze_cols": st.freeze_cols,
            "pane_top": st.pane_top if st.split else 0, "pane_left": st.pane_left if st.split else 0,
            "split": st.split,
            "show_grid": st.show_grid, "show_headers": st.show_headers,
            "sel": st.sel.ranges, "active": st.sel.active,
            "editing": self.editor.cell if self.editing() and self.edit_sheet == self.sheet else None,
            "copy": self.copy_range, "ants_phase": self.ants,
            "refs": self.win.assist.refs if self.editing() else [],
            "show_formulas": self.sheet in self.win.show_formulas,
            "arrows": self.win.arrows.get(self.sheet, []),
            "fill_target": getattr(self, "fill_target", None),
            "sel_objects": [i for i in self.sel_objects if i < self._object_count()],
            "obj_preview": self.obj_preview,
            "hover_note": self.hover_note,
            "page_breaks": st.view_mode == "pageBreakPreview",
            "edit_note": self.edit_note,
            "dv_circles": self.win.dv.circles_for(self.sheet),
        }
        if extra:
            p.update(extra)
        return p

    def refresh(self):
        self.area.queue_draw()
        self.update_scrollbars()
        self.win.on_selection_changed()

    # ------------------------------------------------------------ drawing
    def on_draw(self, area, cr):
        if self.book is None:
            return False
        info = self.book.sheet_info(self.sheet)
        if info["kind"] != "work" and info["kind"] != "macro":
            cr.set_source_rgb(1, 1, 1)
            cr.paint()
            cr.set_source_rgb(0.4, 0.4, 0.4)
            cr.move_to(24, 40)
            cr.show_text("This %s sheet is kept as it is in the file; it is not displayed yet." % info["kind"])
            return False
        self.book.render(self.sheet, cr, self.params())
        from .windows_ui import scrolled
        scrolled(self.win)
        sp = getattr(self, "_split_preview", None)
        if sp:
            a = self.area.get_allocation()
            cr.set_source_rgba(0.3, 0.3, 0.3, 0.6)
            if sp[0] is not None:
                cr.rectangle(0, sp[0] - 2, a.width, 4)
            if sp[1] is not None:
                cr.rectangle(sp[1] - 2, 0, 4, a.height)
            cr.fill()
        bd = getattr(self, "border_drag", None)
        if bd:
            # where the pen will draw
            p = self.params()
            a = self.book.cell_rect(self.sheet, bd[0], bd[1], p)
            z = self.book.cell_rect(self.sheet, bd[2], bd[3], p)
            if a and z:
                x0, y0 = a[0], a[1]
                x1, y1 = z[0] + z[2], z[1] + z[3]
                cr.set_line_width(1)
                cr.set_dash([3, 2])
                cr.set_source_rgb(0.25, 0.25, 0.25)
                cr.rectangle(int(x0) + 0.5, int(y0) + 0.5, int(x1 - x0) - 1, int(y1 - y0) - 1)
                if self.win.border_mode == "grid":
                    for r in range(bd[0] + 1, bd[2] + 1):
                        rr = self.book.cell_rect(self.sheet, r, bd[1], p)
                        if rr:
                            cr.move_to(x0, int(rr[1]) + 0.5)
                            cr.line_to(x1, int(rr[1]) + 0.5)
                    for c in range(bd[1] + 1, bd[3] + 1):
                        cc = self.book.cell_rect(self.sheet, bd[0], c, p)
                        if cc:
                            cr.move_to(int(cc[0]) + 0.5, y0)
                            cr.line_to(int(cc[0]) + 0.5, y1)
                cr.stroke()
                cr.set_dash([])
        return False

    # ------------------------------------------------------------ geometry
    def geometry(self):
        return self.book.geometry(self.sheet, self.params())

    def visible_rows(self):
        """(first scrolling row, last fully visible row)."""
        g = self.geometry()
        a = self.area.get_allocation()
        st = self.state()
        rows = [r for r in g["rows"] if r[0] >= max(st.top_row, st.freeze_rows)]
        full = [r for r in rows if r[1] + r[2] <= a.height]
        last = full[-1][0] if full else st.top_row
        return st.top_row, last

    def visible_cols(self):
        g = self.geometry()
        a = self.area.get_allocation()
        st = self.state()
        cols = [c for c in g["cols"] if c[0] >= max(st.left_col, st.freeze_cols)]
        full = [c for c in cols if c[1] + c[2] <= a.width]
        last = full[-1][0] if full else st.left_col
        return st.left_col, last

    def ensure_visible(self, r, c):
        st = self.state()
        if st.split:
            # above (left of) the top (left) pane: that pane moves to it
            if r < st.pane_top:
                self.scroll_pane(drows=r - st.pane_top)
            if c < st.pane_left:
                self.scroll_pane(dcols=c - st.pane_left)
        if r >= st.freeze_rows:
            first, last = self.visible_rows()
            if r < st.top_row:
                st.top_row = r
            elif r > last:
                # make r the last fully visible row
                a = self.area.get_allocation()
                g = self.geometry()
                avail = a.height - g["hh"]
                if st.freeze_rows:
                    avail -= sum(self.book.row_px(self.sheet, k) * st.zoom
                                 for k in range(st.pane_top if st.split else 0, st.freeze_rows))
                top = r
                used = self.book.row_px(self.sheet, r) * st.zoom
                while top > st.freeze_rows:
                    h = self.book.row_px(self.sheet, top - 1) * st.zoom
                    if used + h > avail:
                        break
                    used += h
                    top -= 1
                st.top_row = top
        if c >= st.freeze_cols:
            first, last = self.visible_cols()
            if c < st.left_col:
                st.left_col = c
            elif c > last:
                a = self.area.get_allocation()
                g = self.geometry()
                avail = a.width - g["hw"]
                if st.freeze_cols:
                    avail -= sum(self.book.col_px(self.sheet, k) * st.zoom
                                 for k in range(st.pane_left if st.split else 0, st.freeze_cols))
                left = c
                used = self.book.col_px(self.sheet, c) * st.zoom
                while left > st.freeze_cols:
                    w = self.book.col_px(self.sheet, left - 1) * st.zoom
                    if used + w > avail:
                        break
                    used += w
                    left -= 1
                st.left_col = left

    def update_scrollbars(self):
        if self.book is None:
            return False
        st = self.state()
        self._sync = True
        ext = self.book.used_extent(self.sheet) or (0, 0)
        first, last = self.visible_rows()
        page = max(1, last - first + 1)
        upper = min(MAX_ROWS, max(ext[0] + 1, st.top_row + page) + page)
        self.vadj.configure(st.top_row, 0, upper, 1, max(1, page - 1), page)
        f2, l2 = self.visible_cols()
        cpage = max(1, l2 - f2 + 1)
        cupper = min(MAX_COLS, max(ext[1] + 1, st.left_col + cpage) + cpage)
        self.hadj.configure(st.left_col, 0, cupper, 1, max(1, cpage - 1), cpage)
        self._sync = False
        return False

    def on_vscroll(self, adj):
        if self._sync or self.book is None:
            return
        st = self.state()
        st.top_row = max(st.freeze_rows, int(adj.get_value()))
        self.area.queue_draw()
        self.place_editor()
        GLib.idle_add(self.update_scrollbars)

    def on_hscroll(self, adj):
        if self._sync or self.book is None:
            return
        st = self.state()
        st.left_col = max(st.freeze_cols, int(adj.get_value()))
        self.area.queue_draw()
        self.place_editor()
        GLib.idle_add(self.update_scrollbars)

    def scroll_pane(self, drows=0, dcols=0):
        """A split's top (left) pane scrolls on its own; the bottom pane keeps below it."""
        st = self.state()
        if not st.split:
            return
        if drows and st.freeze_rows > st.pane_top:
            n = st.freeze_rows - st.pane_top
            st.pane_top = min(MAX_ROWS - n, max(0, st.pane_top + drows))
            st.freeze_rows = st.pane_top + n
            st.top_row = max(st.top_row, st.freeze_rows)
        if dcols and st.freeze_cols > st.pane_left:
            n = st.freeze_cols - st.pane_left
            st.pane_left = min(MAX_COLS - n, max(0, st.pane_left + dcols))
            st.freeze_cols = st.pane_left + n
            st.left_col = max(st.left_col, st.freeze_cols)
        self.area.queue_draw()
        self.place_editor()
        self.update_scrollbars()

    def move_split(self, y=None, x=None):
        """A split bar let go at y (x): the top (left) pane ends at the row (column) there;
        dragged to an edge, that bar goes."""
        st = self.state()
        g = self.geometry()
        a = self.area.get_allocation()
        if y is not None:
            if y <= g["hh"] + 4 or y >= a.height - 4:
                st.freeze_rows = st.pane_top = 0
            else:
                # as many rows as fit above the bar, counted from the top of the window
                n = max(1, len([r for r in g["rows"] if r[1] + r[2] / 2 <= y]))
                if not st.split:
                    st.pane_top = st.top_row
                st.freeze_rows = st.pane_top + n
                st.top_row = max(st.top_row, st.freeze_rows)
        if x is not None:
            if x <= g["hw"] + 4 or x >= a.width - 4:
                st.freeze_cols = st.pane_left = 0
            else:
                n = max(1, len([c for c in g["cols"] if c[1] + c[2] / 2 <= x]))
                if not st.split:
                    st.pane_left = st.left_col
                st.freeze_cols = st.pane_left + n
                st.left_col = max(st.left_col, st.freeze_cols)
        if st.freeze_rows <= st.pane_top and st.freeze_cols <= st.pane_left:
            st.split = False
            st.freeze_rows = st.freeze_cols = st.pane_top = st.pane_left = 0
        self.refresh()
        self.win.ribbon.update("split", "true" if st.split else "false")

    def split_at(self):
        """Where a split's bars are on screen: (y of the row bar or None, x of the column bar or None)."""
        st = self.state()
        if not st.split:
            return None, None
        g = self.geometry()
        y = g["row_spans"][0][1] if st.freeze_rows > st.pane_top and len(g["row_spans"]) > 1 else None
        x = g["col_spans"][0][1] if st.freeze_cols > st.pane_left and len(g["col_spans"]) > 1 else None
        return y, x

    def scroll_by(self, drows=0, dcols=0):
        st = self.state()
        st.top_row = min(MAX_ROWS - 1, max(st.freeze_rows, st.top_row + drows))
        st.left_col = min(MAX_COLS - 1, max(st.freeze_cols, st.left_col + dcols))
        self.area.queue_draw()
        self.place_editor()
        self.update_scrollbars()

    def on_scroll(self, w, ev):
        st = self.state()
        state = ev.get_state()
        ok, dx, dy = ev.get_scroll_deltas()
        if ev.direction == Gdk.ScrollDirection.UP:
            dx, dy = 0, -1
        elif ev.direction == Gdk.ScrollDirection.DOWN:
            dx, dy = 0, 1
        elif ev.direction == Gdk.ScrollDirection.LEFT:
            dx, dy = -1, 0
        elif ev.direction == Gdk.ScrollDirection.RIGHT:
            dx, dy = 1, 0
        if state & Gdk.ModifierType.CONTROL_MASK:
            # Ctrl+wheel zooms in 15 % steps
            z = st.zoom * 100 + (-15 if dy > 0 else 15)
            self.win.set_zoom(z)
            return True
        if state & Gdk.ModifierType.SHIFT_MASK and dx == 0:
            dx, dy = dy, 0
        sy, sx = self.split_at()
        if (dy and sy is not None and ev.y < sy) or (dx and sx is not None and ev.x < sx):
            # over a split's top (left) pane: that pane scrolls
            n = int(round(dy * WHEEL_ROWS)) or (WHEEL_ROWS if dy > 0 else -WHEEL_ROWS) if dy and sy is not None and ev.y < sy else 0
            m = (int(round(dx * 2)) or (1 if dx > 0 else -1)) if dx and sx is not None and ev.x < sx else 0
            self.scroll_pane(drows=n, dcols=m)
            return True
        if dy:
            n = int(round(dy * WHEEL_ROWS)) or (WHEEL_ROWS if dy > 0 else -WHEEL_ROWS)
            self.scroll_by(drows=n)
        if dx:
            n = int(round(dx * 2)) or (1 if dx > 0 else -1)
            self.scroll_by(dcols=n)
        return True

    # ------------------------------------------------------------ objects
    def _object_count(self):
        try:
            return len(self.book.objects(self.sheet))
        except Exception:
            return 0

    def select_object(self, i, add=False):
        if add:
            if i in self.sel_objects:
                self.sel_objects.remove(i)
            else:
                self.sel_objects.append(i)
        else:
            self.sel_objects = [i]
        self.refresh()

    def deselect_objects(self):
        if self.sel_objects:
            self.sel_objects = []
            self.refresh()

    def selected_chart(self):
        """The selected chart's index, if exactly one chart is selected."""
        objs = self.book.objects(self.sheet) if self.book is not None else []
        live = [i for i in self.sel_objects if i < len(objs) and objs[i]["kind"] == "chart"]
        return live[0] if len(live) == 1 and len(self.sel_objects) == 1 else None

    CURSORS = {"body": "move", "nw": "nw-resize", "se": "se-resize", "ne": "ne-resize", "sw": "sw-resize",
               "n": "n-resize", "s": "s-resize", "e": "e-resize", "w": "w-resize"}

    def _object_rect_for(self, d, x, y):
        _, i, part, x0, y0, (rx, ry, rw, rh) = d
        dx, dy = x - x0, y - y0
        if part == "body":
            return (rx + dx, ry + dy, rw, rh)
        l, t, r, b = rx, ry, rx + rw, ry + rh
        if "w" in part:
            l = min(l + dx, r - 12)
        if "e" in part:
            r = max(r + dx, l + 12)
        if "n" in part:
            t = min(t + dy, b - 12)
        if "s" in part:
            b = max(b + dy, t + 12)
        return (l, t, r - l, b - t)

    def _hover(self, h):
        """Notes show while the pointer is over their cell; comments show their card."""
        cell = (h[1], h[2]) if h[0] == "cell" else None
        note = cell if cell is not None and self.book.note(self.sheet, *cell) else None
        if note != self.hover_note:
            self.hover_note = note
            self.area.queue_draw()
        if self._hover_card:
            GLib.source_remove(self._hover_card)
            self._hover_card = 0
        if cell is not None and self.book.thread(self.sheet, *cell):
            card = getattr(self.win, "comment_card", None)
            if card is None or card.cell != cell:
                def show(cell=cell):
                    self._hover_card = 0
                    from .notes_ui import open_card
                    if not self.editing():
                        open_card(self.win, *cell, focus=False)
                    return False
                self._hover_card = GLib.timeout_add(450, show)

    # ------------------------------------------------------------ mouse
    def hit(self, x, y):
        return self.book.hit(self.sheet, float(x), float(y), self.params())

    def on_press(self, w, ev):
        if self.book is None:
            return False
        if self.win.backstage_open():
            return True
        h = self.hit(ev.x, ev.y)
        pointing = self.editing() and h[0] == "cell" and ev.button == 1 and self.win.formula_mode()
        if not pointing:
            self.area.grab_focus()
        state = ev.get_state()
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK) or self.extend_mode
        ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK) or self.add_mode
        oh = None if pointing else self.book.object_hit(self.sheet, float(ev.x), float(ev.y), self.params())
        if oh is not None:
            i, part = oh
            if self.editing() and self.commit_edit() is False:
                return True
            if ev.button == 3:
                if i not in self.sel_objects:
                    self.select_object(i)
                self.win.object_menu(ev, i)
                return True
            if ev.button != 1:
                return False
            if ev.type == Gdk.EventType._2BUTTON_PRESS:
                self.select_object(i)
                self.win.run("chart-format")
                return True
            if ctrl or shift:
                self.select_object(i, add=True)
                return True
            if i not in self.sel_objects:
                self.select_object(i)
            rect = self.book.object_screen_rect(self.sheet, i, self.params())
            self._drag = ("obj", i, part, ev.x, ev.y, rect)
            self.obj_preview = None
            self.win.on_object_selection()
            return True
        if self.sel_objects:
            self.sel_objects = []
            self.win.on_object_selection()
        sy, sx = self.split_at() if ev.button == 1 else (None, None)
        on_y = sy is not None and abs(ev.y - sy) <= 3
        on_x = sx is not None and abs(ev.x - sx) <= 3
        if on_y or on_x:
            if ev.type == Gdk.EventType._2BUTTON_PRESS:
                self.win.run("split")          # a double-click on a bar takes the split away
                return True
            self._drag = ("splitbar", on_y, on_x)
            return True
        if getattr(self.win, "border_mode", None) and ev.button == 1:
            # the pen draws on cells: the fill handle and buttons don't count
            h = self.book.hit(self.sheet, float(ev.x), float(ev.y), self.params({"sel": [], "active": (MAX_ROWS - 1, MAX_COLS - 1)}))
        if getattr(self.win, "border_mode", None) and ev.button == 1 and h[0] == "cell":
            # the border pen: drag out the cells it draws on
            if self.editing() and self.commit_edit() is False:
                return True
            self.border_drag = (h[1], h[2], h[1], h[2])
            self._drag = ("border", h[1], h[2])
            self.area.queue_draw()
            return True
        if ev.button == 3:
            if h[0] == "cell" and not any(contains(rg, h[1], h[2]) for rg in self.sel.ranges):
                self.select_cell(h[1], h[2])
            elif h[0] == "colhdr" and not any(rg[0] == 0 and rg[2] == MAX_ROWS - 1 and rg[1] <= h[1] <= rg[3]
                                             for rg in self.sel.ranges):
                self.sel.select_columns(h[1], h[1])
                self.refresh()
            elif h[0] == "rowhdr" and not any(rg[1] == 0 and rg[3] == MAX_COLS - 1 and rg[0] <= h[1] <= rg[2]
                                             for rg in self.sel.ranges):
                self.sel.select_rows(h[1], h[1])
                self.refresh()
            self.win.context_menu(ev, h)
            return True
        if ev.button != 1:
            return False
        if self.editing():
            if h[0] == "cell" and self.win.formula_mode():
                # pointing at a reference while typing a formula
                self.win.insert_reference(h[1], h[2], extend=shift)
                self._drag = ("point",)
                return True
            if self.commit_edit() is False:
                return True
        if ev.type == Gdk.EventType._2BUTTON_PRESS:
            if h[0] == "cell":
                self.start_edit(mode="edit")
            elif h[0] == "colresize":
                self.win.run("autofit-column-at:%d" % h[1])
            elif h[0] == "rowresize":
                self.win.run("autofit-row-at:%d" % h[1])
            return True
        kind = h[0]
        if kind == "filterbtn":
            self.select_cell(h[1], h[2], refresh=True)
            from .data_ui import filter_menu
            rect = self.book.cell_rect(self.sheet, h[1], h[2], self.params())
            filter_menu(self.win, h[2], rect, row=h[1])
            return True
        if kind == "dvbtn":
            self.select_cell(h[1], h[2], refresh=True)
            rect = self.book.cell_rect(self.sheet, h[1], h[2], self.params())
            self.win.dv.dropdown(h[1], h[2], rect)
            return True
        if kind == "totalsbtn":
            from .table_ui import totals_menu
            rect = self.book.cell_rect(self.sheet, h[1], h[2], self.params())
            totals_menu(self.win, h[1], h[2], rect)
            return True
        if kind == "cell":
            r, c = h[1], h[2]
            if shift:
                self.sel.extend_to(r, c)
            elif ctrl:
                self.sel.add_cell(r, c)
            else:
                self.select_cell(r, c, refresh=False)
                # a plain click on a link follows it on release, unless held to select
                if ev.button == 1 and self.win.links.target_at(r, c):
                    self._link_press = (r, c, time.monotonic())
            self._drag = ("cells",)
        elif kind == "colhdr":
            c = h[1]
            st = self.state()
            self.sel.top_hint = st.top_row
            if shift:
                ar, ac = self.sel.anchor
                self.sel.ranges[-1] = (0, min(ac, c), MAX_ROWS - 1, max(ac, c))
            else:
                self.sel.select_columns(c, c, add=ctrl)
            self._drag = ("cols", self.sel.anchor[1] if shift else c)
        elif kind == "rowhdr":
            r = h[1]
            st = self.state()
            self.sel.left_hint = st.left_col
            if shift:
                ar, ac = self.sel.anchor
                self.sel.ranges[-1] = (min(ar, r), 0, max(ar, r), MAX_COLS - 1)
            else:
                self.sel.select_rows(r, r, add=ctrl)
            self._drag = ("rows", self.sel.anchor[0] if shift else r)
        elif kind == "corner":
            self.sel.select_all()
        elif kind in ("colresize", "rowresize"):
            self._drag = (kind, h[1], ev.x, ev.y)
            return True
        elif kind == "fill":
            self._drag = ("fill", tuple(self.sel.ranges[-1]))
            self.fill_target = None
            return True
        self.end_mode = False
        self.refresh()
        return True

    def on_motion(self, w, ev):
        if self.book is None:
            return False
        self._last_motion = (ev.x, ev.y)
        if self._drag is None:
            h = self.hit(ev.x, ev.y)
            cur = None
            oh = self.book.object_hit(self.sheet, float(ev.x), float(ev.y), self.params())
            if oh is not None:
                h = ("object",)
                cur = self.CURSORS.get(oh[1], "move")
            self._hover(h)
            if h[0] == "colresize":
                cur = "col-resize"
            elif h[0] == "rowresize":
                cur = "row-resize"
            elif h[0] == "fill":
                cur = "crosshair"
            elif h[0] == "cell" and getattr(self.win, "border_mode", None):
                cur = "pencil" if self.win.border_mode != "erase" else "crosshair"
            elif h[0] == "cell":
                cur = "pointer" if self.win.links.hover((h[1], h[2])) else "cell"
            if h[0] != "cell":
                self.win.links.hover(None)
            win = self.area.get_window()
            if win is not None:
                win.set_cursor(Gdk.Cursor.new_from_name(self.area.get_display(), cur) if cur else None)
            return False
        self._drag_to(ev.x, ev.y)
        self._autoscroll_check(ev.x, ev.y)
        return True

    def _drag_to(self, x, y):
        kind = self._drag[0]
        if kind == "obj":
            d = self._drag
            if self.obj_preview is None and abs(x - d[3]) < 3 and abs(y - d[4]) < 3:
                return
            self.obj_preview = self._object_rect_for(d, x, y)
            self.area.queue_draw()
            return
        if kind in ("colresize", "rowresize"):
            self.win.resize_feedback(self._drag, x, y)
            return
        if kind == "splitbar":
            self._split_preview = (y if self._drag[1] else None, x if self._drag[2] else None)
            self.area.queue_draw()
            return
        h = self.hit(x, y)
        if kind == "border":
            h = self.book.hit(self.sheet, float(x), float(y), self.params({"sel": [], "active": (MAX_ROWS - 1, MAX_COLS - 1)}))
            if h[0] == "cell":
                r0, c0 = self._drag[1], self._drag[2]
                self.border_drag = (min(r0, h[1]), min(c0, h[2]), max(r0, h[1]), max(c0, h[2]))
                self.area.queue_draw()
            return
        if kind == "point" and h[0] == "cell":
            self.win.insert_reference(h[1], h[2], extend=True)
            return
        if kind == "fill":
            if h[0] == "cell":
                self.fill_target = self._fill_target(self._drag[1], h[1], h[2])
                self.area.queue_draw()
            return
        if kind == "cells" and h[0] == "cell":
            self.sel.extend_to(h[1], h[2])
            self.refresh()
            self.win.show_drag_size()
        elif kind == "cols":
            c = h[1] if h[0] in ("colhdr", "cell") else None
            if h[0] == "cell":
                c = h[2]
            if c is not None:
                a = self._drag[1]
                self.sel.ranges[-1] = (0, min(a, c), MAX_ROWS - 1, max(a, c))
                self.refresh()
        elif kind == "rows":
            r = h[1] if h[0] in ("rowhdr", "cell") else None
            if r is not None:
                a = self._drag[1]
                self.sel.ranges[-1] = (min(a, r), 0, max(a, r), MAX_COLS - 1)
                self.refresh()

    @staticmethod
    def _fill_target(src, r, c):
        """The range a fill-handle drag to (r, c) covers: src stretched in one direction
        (the one the pointer went further in), or shrunk."""
        r1, c1, r2, c2 = src
        down = r - r2 if r > r2 else (r - r1 if r < r1 else 0)
        across = c - c2 if c > c2 else (c - c1 if c < c1 else 0)
        if down == 0 and across == 0:
            # inside: shrinking clears what is left out
            if r < r2 and (r2 - r) >= (c2 - c):
                return (r1, c1, max(r, r1), c2)
            if c < c2:
                return (r1, c1, r2, max(c, c1))
            return src
        if abs(down) >= abs(across):
            return (min(r1, r), c1, max(r2, r), c2)
        return (r1, min(c1, c), r2, max(c2, c))

    def _autoscroll_check(self, x, y):
        a = self.area.get_allocation()
        need = x < 0 or y < 0 or x > a.width or y > a.height
        if need and not self._autoscroll_id:
            self._autoscroll_id = GLib.timeout_add(60, self._autoscroll)
        elif not need and self._autoscroll_id:
            GLib.source_remove(self._autoscroll_id)
            self._autoscroll_id = 0

    def _autoscroll(self):
        if self._drag is None or self._last_motion is None:
            self._autoscroll_id = 0
            return False
        x, y = self._last_motion
        a = self.area.get_allocation()
        dr = 1 if y > a.height else (-1 if y < 0 else 0)
        dc = 1 if x > a.width else (-1 if x < 0 else 0)
        if dr or dc:
            self.scroll_by(dr, dc)
            self._drag_to(min(max(x, 1), a.width - 2), min(max(y, 1), a.height - 2))
        return True

    def on_release(self, w, ev):
        if self._autoscroll_id:
            GLib.source_remove(self._autoscroll_id)
            self._autoscroll_id = 0
        d = self._drag
        self._drag = None
        if d and d[0] == "splitbar":
            self._split_preview = None
            self.move_split(ev.y if d[1] else None, ev.x if d[2] else None)
            return True
        if d and d[0] == "border":
            rg, self.border_drag = getattr(self, "border_drag", None), None
            if rg:
                self.win.apply_border_pen(rg)
                self.win.update_title()
            self.area.queue_draw()
            return True
        if d and d[0] == "obj":
            prev, self.obj_preview = self.obj_preview, None
            if prev is not None:
                x, y, w, h = prev
                label = "Move Chart" if d[2] == "body" else "Size Chart"
                from .protect_ui import protected, allows
                if protected(self.win) and not allows(self.win, "objects"):
                    self.refresh()
                    self.win.dialogs.message("The chart you're trying to change is on a protected sheet.")
                    return True
                try:
                    self.book.place_object(self.sheet, d[1], float(x), float(y), float(w), float(h), self.params(), label)
                except ValueError as e:
                    self.win.dialogs.message(str(e))
                self.win.update_title()
            self.refresh()
            self.win.on_object_selection()
            return True
        if d and d[0] in ("colresize", "rowresize"):
            self.win.resize_header(d, ev.x, ev.y)
        if d and d[0] == "fill":
            target, self.fill_target = getattr(self, "fill_target", None), None
            if target and tuple(target) != tuple(d[1]):
                ctrl = bool(ev.state & Gdk.ModifierType.CONTROL_MASK)
                self.win.fill_handle(d[1], target, ctrl)
            self.area.queue_draw()
        self.win.on_selection_changed()
        lp, self._link_press = getattr(self, "_link_press", None), None
        if lp and d and d[0] == "cells" and time.monotonic() - lp[2] < HOLD_TO_SELECT:
            h = self.hit(ev.x, ev.y)
            if h[0] == "cell" and (h[1], h[2]) == (lp[0], lp[1]) and self.sel.is_single_cell():
                GLib.idle_add(lambda: (self.win.links.follow(lp[0], lp[1]), False)[1])
        if d and d[0] in ("cells", "cols", "rows") and getattr(self.win, "painter", None):
            self.win.painter_finish()
        return d is not None

    # ------------------------------------------------------------ selection
    def select_cell(self, r, c, refresh=True, extend=False):
        r = min(max(r, 0), MAX_ROWS - 1)
        c = min(max(c, 0), MAX_COLS - 1)
        from .protect_ui import may_select
        if not extend and not may_select(self.win, r, c):
            return          # a protected sheet that doesn't let this cell be selected
        if extend:
            self.sel.extend_to(r, c)
            self.ensure_visible(r, c)
        else:
            m = self.book.merge_at(self.sheet, r, c)
            if m:
                r, c = m[0], m[1]
            self.sel.set_cell(r, c, m)
            self.ensure_visible(r, c)
        if refresh:
            self.refresh()

    def select_range(self, rg, active=None):
        self.sel.set_range(rg, active)
        self.ensure_visible(*(active or (rg[0], rg[1])))
        self.refresh()

    def move(self, dr, dc, extend=False, ctrl=False, keep_tab=False):
        sel = self.sel
        if not keep_tab and not (dc and not dr):
            self.tab_origin = None
        if extend:
            r, c = sel.cursor
        else:
            r, c = sel.active
            # leave a merged block from its far edge
            m = self.book.merge_at(self.sheet, r, c)
            if m:
                if dr > 0:
                    r = m[2]
                if dc > 0:
                    c = m[3]
        if ctrl or self.end_mode:
            r, c = self.book.ctrl_move(self.sheet, r, c, dr, dc)
            self.end_mode = False
        else:
            r += dr
            c += dc
        r = min(max(r, 0), MAX_ROWS - 1)
        c = min(max(c, 0), MAX_COLS - 1)
        self.select_cell(r, c, extend=extend or self.extend_mode)

    def page(self, direction, horizontal=False, extend=False):
        st = self.state()
        if horizontal:
            first, last = self.visible_cols()
            n = max(1, last - first + 1)
            st.left_col = min(MAX_COLS - 1, max(st.freeze_cols, st.left_col + direction * n))
            r, c = self.sel.cursor if extend else self.sel.active
            c = min(MAX_COLS - 1, max(0, c + direction * n))
        else:
            first, last = self.visible_rows()
            n = max(1, last - first + 1)
            st.top_row = min(MAX_ROWS - 1, max(st.freeze_rows, st.top_row + direction * n))
            r, c = self.sel.cursor if extend else self.sel.active
            r = min(MAX_ROWS - 1, max(0, r + direction * n))
        self.select_cell(r, c, extend=extend)

    # ------------------------------------------------------------ editing
    def editing(self):
        return self.editor.cell is not None

    def start_edit(self, initial=None, mode="enter"):
        if self.book is None:
            return
        info = self.book.sheet_info(self.sheet)
        if info["kind"] not in ("work", "macro"):
            return
        r, c = self.sel.active
        from .protect_ui import may_change_cells
        if not may_change_cells(self.win, [(r, c, r, c)]):
            return
        self.ensure_visible(r, c)
        self.area.queue_draw()
        self.editor.cell = (r, c)
        self.edit_sheet = self.sheet
        self.editor.mode = mode
        st = self.book.cell_style(self.sheet, r, c)
        self.editor.style_for(st, self.state().zoom)
        text = self.book.input(self.sheet, r, c) if initial is None else initial
        self.editor.set_text(text)
        self.win.dv.close_popup()
        self.editor.show()
        self.place_editor()
        self.editor.grab_focus()
        self.win.edit_started(text)

    def place_editor(self):
        self.win.dv.place()
        if not self.editing():
            return
        r, c = self.editor.cell
        rect = self.book.cell_rect(self.sheet, r, c, self.params()) if self.edit_sheet == self.sheet else None
        if rect is None:
            self.editor.set_size_request(1, 1)
            self.fixed.move(self.editor, -100, -100)
            return
        x, y, w, h = rect
        self.editor.base = rect
        # grow to fit the text, up to the right edge of the window
        lay = self.editor.create_pango_layout(self.editor.text() or " ")
        tw, th = lay.get_pixel_size()
        a = self.area.get_allocation()
        need_w = max(w, tw + 12)
        need_w = min(need_w, max(w, a.width - x - 2))
        need_h = max(h, th + 2)
        self.fixed.move(self.editor, int(x), int(y))
        self.editor.set_size_request(int(need_w), int(need_h))

    def editor_changed(self):
        self.place_editor()
        self.win.edit_changed(self.editor.text())
        GLib.idle_add(lambda: (self.win.assist.text_changed() if self.editing() else None, False)[1])

    def commit_edit(self, move=None):
        if not self.editing():
            return
        r, c = self.editor.cell
        text = self.editor.text()
        if self.win.formula_bar.view.has_focus():
            text = self.win.formula_bar.text()
        checked = self.win.validate_entry(text)
        if checked is False:
            self.cancel_edit()
            return False
        if checked is None:
            (self.win.formula_bar.view if self.edit_sheet != self.sheet else self.editor).grab_focus()
            return False
        text = checked
        self.editor.cell = None
        self.editor.hide()
        self._back_to_edit_sheet()
        try:
            self.book.set_input(self.sheet, r, c, text)
        except Exception as e:
            self.win.formula_error(text, e)
        self.win.edit_ended()
        self.win.dv.recheck_circles()
        self.area.grab_focus()
        if move is not None:
            self._after_commit_move(*move)
        self.refresh()

    def _after_commit_move(self, dr, dc):
        if not self.sel.is_single_cell():
            self.sel.step_within(dr, dc)
            self.ensure_visible(*self.sel.active)
        else:
            self.tab_step(dr, dc)

    def tab_step(self, dr, dc):
        """Tab/Enter in a single cell: Enter after a run of Tabs goes to the
        next row in the column the run started from."""
        r, c = self.sel.active
        if dc and not dr:
            if self.tab_origin is None:
                self.tab_origin = c
            self.move(0, dc)
            self.tab_origin = self.tab_origin if dc > 0 else None
            return
        if dr > 0 and self.tab_origin is not None:
            col = self.tab_origin
            self.tab_origin = None
            self.select_cell(r + 1, col)
            return
        self.tab_origin = None
        self.move(dr, dc)

    def _back_to_edit_sheet(self):
        es = self.edit_sheet
        self.edit_sheet = None
        if es is not None and es != self.sheet:
            self.sheet = es
            self.book.active_sheet = es
            self.win.sheet_changed()

    def cancel_edit(self):
        if not self.editing():
            return
        self.editor.cell = None
        self.editor.hide()
        self._back_to_edit_sheet()
        self.win.edit_ended()
        self.area.grab_focus()
        self.area.queue_draw()

    def on_editor_key(self, w, ev):
        if self.win.assist.key(ev):
            return True
        kv = ev.keyval
        state = ev.get_state()
        ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK)
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK)
        alt = bool(state & Gdk.ModifierType.MOD1_MASK)
        if kv in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            if alt:
                self.editor.get_buffer().insert_at_cursor("\n")
                return True
            if ctrl:
                self.win.fill_selection_with_edit()
                return True
            self.commit_edit(move=(-1 if shift else 1, 0))
            return True
        if kv in (Gdk.KEY_Tab, Gdk.KEY_ISO_Left_Tab):
            self.commit_edit(move=(0, -1 if shift or kv == Gdk.KEY_ISO_Left_Tab else 1))
            return True
        if kv == Gdk.KEY_Escape:
            self.cancel_edit()
            return True
        if kv == Gdk.KEY_F2:
            self.editor.mode = "edit" if self.editor.mode == "enter" else "enter"
            self.win.edit_mode_changed(self.editor.mode)
            return True
        if self.editor.mode == "enter" and not ctrl and not alt:
            arrows = {Gdk.KEY_Up: (-1, 0), Gdk.KEY_Down: (1, 0), Gdk.KEY_Left: (0, -1),
                      Gdk.KEY_Right: (0, 1)}
            if kv in arrows:
                if self.win.formula_mode() and self.win.point_ready():
                    self.win.point_move(*arrows[kv], extend=shift)
                    return True
                self.commit_edit(move=arrows[kv])
                return True
        return False
