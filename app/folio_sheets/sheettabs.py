"""Sheet tabs at the bottom of the window, with the horizontal scroll bar
beside them."""
from gi.repository import Gdk, GLib, Gtk


class SheetTabs(Gtk.Paned):
    def __init__(self, win):
        Gtk.Paned.__init__(self, orientation=Gtk.Orientation.HORIZONTAL)
        self.win = win
        self.get_style_context().add_class("folio-sheetbar")
        left = Gtk.Box(spacing=0)
        self.prev_btn = Gtk.Button(label="◀")
        self.next_btn = Gtk.Button(label="▶")
        for b, tip, d in ((self.prev_btn, "Scroll to the previous sheet", -1),
                          (self.next_btn, "Scroll to the next sheet", 1)):
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_focus_on_click(False)
            b.set_tooltip_text(tip + " (Ctrl+click: first/last)")
            b.get_style_context().add_class("folio-tabnav")
            b.connect("button-press-event", self.on_nav, d)
            left.pack_start(b, False, False, 0)
        self.scroll = Gtk.ScrolledWindow()
        self.scroll.set_policy(Gtk.PolicyType.EXTERNAL, Gtk.PolicyType.NEVER)
        self.tabs = Gtk.Box(spacing=0)
        self.scroll.add(self.tabs)
        left.pack_start(self.scroll, True, True, 0)
        self.add_btn = Gtk.Button(label="+")
        self.add_btn.set_relief(Gtk.ReliefStyle.NONE)
        self.add_btn.set_focus_on_click(False)
        self.add_btn.set_tooltip_text("New sheet (Shift+F11)")
        self.add_btn.get_style_context().add_class("folio-tabadd")
        self.add_btn.connect("clicked", lambda *_: win.run("insert-sheet"))
        left.pack_start(self.add_btn, False, False, 0)
        self.pack1(left, True, False)
        self.hbar = Gtk.Scrollbar(orientation=Gtk.Orientation.HORIZONTAL, adjustment=win.grid.hadj)
        self.pack2(self.hbar, True, False)
        self.set_position(600)
        self.buttons = []

    def rebuild(self):
        for b in self.buttons:
            if b is not None:
                self.tabs.remove(b)
        self.buttons = []
        book = self.win.book
        for i in range(book.sheet_count):
            info = book.sheet_info(i)
            if info["state"] != "visible":
                self.buttons.append(None)
                continue
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_focus_on_click(False)
            b.get_style_context().add_class("folio-sheettab")
            box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
            lab = Gtk.Label(label=info["name"])
            box.pack_start(lab, True, True, 0)
            bar = Gtk.Box()
            bar.set_size_request(-1, 3)
            if info["tab_color"] is not None:
                css = Gtk.CssProvider()
                css.load_from_data(("box { background: #%06x; }" % info["tab_color"]).encode())
                bar.get_style_context().add_provider(css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
            box.pack_start(bar, False, False, 0)
            b.add(box)
            b.connect("button-press-event", self.on_tab_press, i)
            b.connect("motion-notify-event", self.on_tab_motion, i)
            b.connect("button-release-event", self.on_tab_release, i)
            b.add_events(Gdk.EventMask.POINTER_MOTION_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK)
            self.tabs.pack_start(b, False, False, 0)
            self.buttons.append(b)
        self.buttons += []
        self.tabs.show_all()
        self.mark_active()

    def mark_active(self):
        cur = self.win.grid.sheet
        for i, b in enumerate(self.buttons):
            if b is None:
                continue
            ctx = b.get_style_context()
            if i == cur:
                ctx.add_class("active")
                GLib.idle_add(self._scroll_to, b)
            else:
                ctx.remove_class("active")

    def _scroll_to(self, b):
        a = b.get_allocation()
        adj = self.scroll.get_hadjustment()
        if a.x < adj.get_value() or a.x + a.width > adj.get_value() + adj.get_page_size():
            adj.set_value(max(0, a.x - 8))
        return False

    def on_tab_press(self, b, ev, i):
        if ev.button == 1 and ev.type == Gdk.EventType._2BUTTON_PRESS:
            self.win.run("rename-sheet")
            return True
        if ev.button in (1, 3):
            self.win.grid.set_sheet(i)
            self.win.sheet_changed()
        if ev.button == 1:
            self._drag = [i, ev.x_root, False]
        if ev.button == 3:
            self.win.sheet_tab_menu(b, ev)
        return True

    def drop_index(self, x_root):
        """The position a tab dragged to x_root goes before (sheet_count: the end)."""
        n = self.win.book.sheet_count
        for k, b in enumerate(self.buttons):
            if b is None or not b.get_realized():
                continue
            ox, _oy = b.get_window().get_origin()[1:]
            a = b.get_allocation()
            if x_root < ox + a.width / 2:
                return k
        return n

    def on_tab_motion(self, b, ev, i):
        d = getattr(self, "_drag", None)
        if d and not d[2] and abs(ev.x_root - d[1]) > 8:
            d[2] = True
            cur = Gdk.Cursor.new_from_name(b.get_display(), "copy" if ev.get_state() & Gdk.ModifierType.CONTROL_MASK else "move")
            b.get_window().set_cursor(cur)
        return False

    def on_tab_release(self, b, ev, i):
        """Dragging a tab moves the sheet; Ctrl+drag copies it."""
        d = getattr(self, "_drag", None)
        self._drag = None
        if not d or not d[2]:
            return False
        b.get_window().set_cursor(None)
        before = self.drop_index(ev.x_root)
        if ev.get_state() & Gdk.ModifierType.CONTROL_MASK:
            self.win.copy_sheet_to(d[0], before)
        else:
            self.win.move_sheet_to(d[0], before)
        return True

    def on_nav(self, b, ev, d):
        adj = self.scroll.get_hadjustment()
        ctrl = ev.get_state() & Gdk.ModifierType.CONTROL_MASK
        if ctrl:
            adj.set_value(0 if d < 0 else adj.get_upper())
        else:
            adj.set_value(adj.get_value() + d * 80)
        return True

    def visible_order(self):
        return [i for i, b in enumerate(self.buttons) if b is not None]
