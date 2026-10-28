"""The File view (Backstage): Home, New, Open, Info, Save, Save As, Print,
Export, Close, Options. Shares its look with Folio Writer's."""
import datetime
import os

from gi.repository import GLib, Gtk, Pango

from . import APP_NAME, icons

NAV = [("back", ""), ("home", "Home"), ("new", "New"), ("open", "Open"), None,
       ("info", "Info"), ("save", "Save"), ("saveas", "Save As"), ("print", "Print"),
       ("share", "Share"), ("export", "Export"), ("close", "Close"), "spacer", ("options", "Options")]


def _heading(text, cls="folio-bs-title"):
    lab = Gtk.Label(label=text, xalign=0)
    lab.get_style_context().add_class(cls)
    return lab


def _when(ts):
    d = datetime.datetime.fromtimestamp(ts)
    now = datetime.datetime.now()
    if d.date() == now.date():
        return d.strftime("%-I:%M %p")
    if (now - d).days < 7:
        return d.strftime("%A")
    return d.strftime("%-m/%-d/%Y")


class Backstage(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self)
        self.win = win
        self.get_style_context().add_class("folio-backstage")
        nav = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        nav.get_style_context().add_class("folio-bs-nav")
        nav.set_size_request(180, -1)
        self.nav_buttons = {}
        for entry in NAV:
            if entry is None:
                s = Gtk.Separator()
                s.get_style_context().add_class("folio-bs-sep")
                nav.pack_start(s, False, False, 6)
                continue
            if entry == "spacer":
                nav.pack_start(Gtk.Box(), True, True, 0)
                continue
            key, label = entry
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.get_style_context().add_class("folio-bs-navbtn")
            if key == "back":
                b.add(Gtk.Image.new_from_icon_name("go-previous-symbolic", Gtk.IconSize.LARGE_TOOLBAR))
                b.set_tooltip_text("Back (Esc)")
                b.set_halign(Gtk.Align.START)
                b.get_style_context().add_class("back")
            else:
                b.add(Gtk.Label(label=label, xalign=0))
            b.connect("clicked", lambda _b, k=key: self.nav(k))
            nav.pack_start(b, False, False, 0)
            self.nav_buttons[key] = b
        self.pack_start(nav, False, False, 0)
        self.stack = Gtk.Stack()
        self.stack.get_style_context().add_class("folio-bs-body")
        self.pack_start(self.stack, True, True, 0)
        self.current = None

    # ------------------------------------------------------------ navigation
    def open(self, page="home"):
        self.win.ribbon.set_sensitive(False)
        self.show_all()
        self.nav(page)

    def close(self):
        self.hide()
        self.win.ribbon.set_sensitive(True)
        self.win.grid.area.grab_focus()

    def nav(self, key):
        if key == "back":
            self.close()
            return
        if key == "save":
            self.close()
            self.win.run("save")
            return
        if key == "close":
            self.close()
            self.win.run("close")
            return
        if key == "options":
            self.win.run("options")
            return
        self.current = key
        for k, b in self.nav_buttons.items():
            ctx = b.get_style_context()
            (ctx.add_class if k == key else ctx.remove_class)("active")
        old = self.stack.get_child_by_name(key)
        if old is not None:
            self.stack.remove(old)
        w = getattr(self, "page_" + key)()
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.add(w)
        sw.show_all()
        self.stack.add_named(sw, key)
        self.stack.set_visible_child_name(key)

    def _page(self, title):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        box.set_border_width(36)
        box.pack_start(_heading(title), False, False, 0)
        return box

    # ------------------------------------------------------------ pieces
    def _tile(self, name, cb):
        b = Gtk.Button()
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.get_style_context().add_class("folio-bs-tile")
        v = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        thumb = Gtk.DrawingArea()
        thumb.set_size_request(194, 150)
        thumb.connect("draw", self._draw_blank_thumb)
        v.pack_start(thumb, False, False, 0)
        v.pack_start(Gtk.Label(label=name), False, False, 0)
        b.add(v)
        b.connect("clicked", lambda *_: cb())
        return b

    @staticmethod
    def _draw_blank_thumb(area, cr):
        """A little empty grid, like a blank workbook."""
        w, h = area.get_allocated_width(), area.get_allocated_height()
        cr.set_source_rgb(1, 1, 1)
        cr.rectangle(0, 0, w, h)
        cr.fill()
        cr.set_source_rgb(0.85, 0.85, 0.85)
        cr.set_line_width(1)
        for x in range(0, w, 32):
            cr.move_to(x + 0.5, 0)
            cr.line_to(x + 0.5, h)
        for y in range(0, h, 12):
            cr.move_to(0, y + 0.5)
            cr.line_to(w, y + 0.5)
        cr.stroke()
        cr.set_source_rgb(0.65, 0.49, 0.0)
        cr.set_line_width(2)
        cr.rectangle(33, 13, 30, 10)
        cr.stroke()
        cr.set_source_rgb(0.6, 0.6, 0.6)
        cr.rectangle(0, 0, w, h)
        cr.stroke()
        return False

    def _recent_list(self, limit=50, filter_text=""):
        lb = Gtk.ListBox()
        lb.get_style_context().add_class("folio-bs-recent")
        items = self.win.app.pinned() + [p for p in self.win.app.recent() if p not in self.win.app.pinned()]
        ft = filter_text.lower()
        n = 0
        for p in items:
            if ft and ft not in os.path.basename(p).lower():
                continue
            row = Gtk.Box(spacing=12)
            row.set_border_width(4)
            row.pack_start(icons.image("r-workbook-file", 32), False, False, 0)
            txt = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
            name = Gtk.Label(label=os.path.basename(p), xalign=0)
            name.set_ellipsize(Pango.EllipsizeMode.END)
            folder = Gtk.Label(label=os.path.dirname(p), xalign=0)
            folder.get_style_context().add_class("dim-label")
            folder.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
            txt.pack_start(name, False, False, 0)
            txt.pack_start(folder, False, False, 0)
            row.pack_start(txt, True, True, 0)
            try:
                row.pack_start(Gtk.Label(label=_when(os.path.getmtime(p))), False, False, 8)
            except OSError:
                pass
            lb.add(row)
            row.path = p
            n += 1
            if n >= limit:
                break
        lb.connect("row-activated", lambda _lb, r: self._open_path(r.get_child().path))
        if n == 0:
            lab = Gtk.Label(label="No recent workbooks yet." if not ft else "Nothing matches.", xalign=0)
            lab.get_style_context().add_class("dim-label")
            lb.add(lab)
        return lb

    def _open_path(self, p):
        if not p:
            return
        self.close()
        self.win.app.open_path(p, self.win)

    def _browse(self):
        p = self.win.choose_open_file()
        if p:
            self._open_path(p)

    # ------------------------------------------------------------ pages
    def page_home(self):
        h = datetime.datetime.now().hour
        box = self._page("Good morning" if h < 12 else ("Good afternoon" if h < 18 else "Good evening"))
        strip = Gtk.Box(spacing=14)
        strip.pack_start(self._tile("Blank workbook", lambda: (self.close(), self.win.run("new"))), False, False, 0)
        box.pack_start(strip, False, False, 0)
        search = Gtk.SearchEntry()
        search.set_placeholder_text("Search")
        search.set_size_request(400, -1)
        search.set_halign(Gtk.Align.START)
        box.pack_start(search, False, False, 0)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>Recent</b>")
        box.pack_start(lab, False, False, 0)
        holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        holder.pack_start(self._recent_list(12), False, False, 0)
        box.pack_start(holder, False, False, 0)

        def filt(e):
            for c in holder.get_children():
                holder.remove(c)
            holder.pack_start(self._recent_list(12, e.get_text()), False, False, 0)
            holder.show_all()
        search.connect("search-changed", filt)
        return box

    def page_new(self):
        box = self._page("New")
        flow = Gtk.FlowBox()
        flow.set_selection_mode(Gtk.SelectionMode.NONE)
        flow.add(self._tile("Blank workbook", lambda: (self.close(), self.win.run("new"))))
        box.pack_start(flow, False, False, 0)
        return box

    def page_open(self):
        box = self._page("Open")
        h = Gtk.Box(spacing=30)
        places = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        places.set_size_request(240, -1)
        for label, icon, cb in (("Recent", "r-recent", lambda: None),
                                ("This PC", "r-this-pc", self._browse),
                                ("Browse", "r-browse", self._browse)):
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.get_style_context().add_class("folio-bs-place")
            hb = Gtk.Box(spacing=10)
            hb.pack_start(icons.image(icon, 32), False, False, 0)
            hb.pack_start(Gtk.Label(label=label, xalign=0), True, True, 0)
            b.add(hb)
            b.connect("clicked", lambda _b, cb=cb: cb())
            places.pack_start(b, False, False, 0)
        h.pack_start(places, False, False, 0)
        right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        search = Gtk.SearchEntry()
        search.set_placeholder_text("Search")
        right.pack_start(search, False, False, 0)
        holder = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        holder.pack_start(self._recent_list(50), False, False, 0)
        right.pack_start(holder, True, True, 0)

        def filt(e):
            for c in holder.get_children():
                holder.remove(c)
            holder.pack_start(self._recent_list(50, e.get_text()), False, False, 0)
            holder.show_all()
        search.connect("search-changed", filt)
        h.pack_start(right, True, True, 0)
        box.pack_start(h, True, True, 0)
        return box

    def page_info(self):
        box = self._page("Info")
        w = self.win
        name = os.path.basename(w.path) if w.path else w.untitled
        box.pack_start(_heading(name, "folio-bs-sub"), False, False, 0)
        if w.path:
            loc = Gtk.Label(label=os.path.dirname(w.path), xalign=0)
            loc.get_style_context().add_class("dim-label")
            box.pack_start(loc, False, False, 0)
        grid = Gtk.Grid(column_spacing=24, row_spacing=6)
        rows = [("Sheets", str(w.book.sheet_count))]
        if w.path and os.path.exists(w.path):
            st = os.stat(w.path)
            rows.append(("Size", "%.1f KB" % (st.st_size / 1024)))
            rows.append(("Last Modified", datetime.datetime.fromtimestamp(st.st_mtime).strftime("%-m/%-d/%Y %-I:%M %p")))
        if w.imported_from:
            rows.append(("Opened from", "%s (converted; save as .xlsx)" % os.path.basename(w.imported_from)))
        for i, (k, v) in enumerate(rows):
            grid.attach(Gtk.Label(label=k, xalign=0), 0, i, 1, 1)
            grid.attach(Gtk.Label(label=v, xalign=0), 1, i, 1, 1)
        box.pack_start(grid, False, False, 12)
        return box

    def page_saveas(self):
        box = self._page("Save As")
        b = Gtk.Button(label="Browse...")
        b.set_halign(Gtk.Align.START)
        b.connect("clicked", lambda *_: (self.close(), self.win.run("save-as")))
        box.pack_start(b, False, False, 0)
        return box

    def page_print(self):
        box = self._page("Print")
        from .print_ui import preview_widget
        self.print_view = preview_widget(self.win)
        box.pack_start(self.print_view, True, True, 0)
        return box

    def page_share(self):
        box = self._page("Share")
        box.pack_start(_heading("Email", "folio-bs-sub"), False, False, 0)
        for label, desc, cmd in (("Send as Attachment", "Everyone gets a separate copy of the workbook.", "send-attachment"),
                                 ("Send as PDF", "Everyone gets a PDF of the workbook; it keeps its layout and can't be changed easily.", "send-pdf")):
            row = Gtk.Box(spacing=12)
            b = Gtk.Button(label=label)
            b.set_halign(Gtk.Align.START)
            b.connect("clicked", lambda _b, c=cmd: (self.close(), self.win.run(c)))
            row.pack_start(b, False, False, 0)
            row.pack_start(Gtk.Label(label=desc, xalign=0), False, False, 0)
            box.pack_start(row, False, False, 0)
        return box

    def page_export(self):
        box = self._page("Export")
        for label, cmd in (("Create PDF Document", "export-pdf"),
                           ("Change File Type: CSV (Comma delimited)", "save-as:csv")):
            b = Gtk.Button(label=label)
            b.set_halign(Gtk.Align.START)
            b.connect("clicked", lambda _b, c=cmd: (self.close(), self.win.run(c)))
            box.pack_start(b, False, False, 0)
        return box
