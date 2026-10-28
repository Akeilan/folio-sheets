"""Hyperlinks: the Insert / Edit Hyperlink dialog (Ctrl+K), following a link with a click,
Clear and Remove Hyperlinks, and the pointer and tip over a linked cell."""
import os
import time
from urllib.parse import quote, unquote

from gi.repository import GLib, Gtk

from .core import parse_ref

LINK_COLOR = 0x0563C1
HOLD_TO_SELECT = 0.6        # seconds: a longer click selects the cell instead of following
KINDS = [("web", "Existing File\nor Web Page"), ("place", "Place in This\nDocument"), ("email", "E-mail\nAddress")]


def describe(link):
    """The address as a person reads it."""
    if link.get("target"):
        t = link["target"]
        if t.startswith("mailto:"):
            return unquote(t[7:].split("?", 1)[0])
        return t
    return link.get("location", "")


def parse_mailto(t):
    addr, _, q = t[7:].partition("?")
    subject = ""
    for part in q.split("&"):
        if part.lower().startswith("subject="):
            subject = unquote(part[8:])
    return unquote(addr), subject


class HyperlinkDialog(Gtk.Dialog):
    def __init__(self, win, link, text, editable_text):
        title = "Edit Hyperlink" if link else "Insert Hyperlink"
        Gtk.Dialog.__init__(self, title=title, transient_for=win, modal=True)
        self.win = win
        self.get_style_context().add_class("folio-dialog")
        self.set_resizable(False)
        self.remove_btn = self.add_button("Remove Link", 2) if link else None
        if self.remove_btn is not None:
            bb = self.remove_btn.get_parent()
            if isinstance(bb, Gtk.ButtonBox):
                bb.set_child_secondary(self.remove_btn, True)
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        link = dict(link or {})
        self.tooltip = link.get("tooltip", "")
        box = self.get_content_area()
        box.set_border_width(10)
        box.set_spacing(8)

        top = Gtk.Grid(column_spacing=8, row_spacing=6)
        top.attach(Gtk.Label(label="Text to display:", xalign=0), 0, 0, 1, 1)
        self.text = Gtk.Entry(hexpand=True, activates_default=True)
        self.text.set_text(text)
        self.text.set_sensitive(editable_text)
        top.attach(self.text, 1, 0, 1, 1)
        self.tip_btn = Gtk.Button(label="ScreenTip...")
        self.tip_btn.connect("clicked", lambda *_: self.edit_tip())
        top.attach(self.tip_btn, 2, 0, 1, 1)
        box.pack_start(top, False, False, 0)

        body = Gtk.Box(spacing=10)
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        side.pack_start(Gtk.Label(label="Link to:", xalign=0), False, False, 0)
        self.stack = Gtk.Stack()
        self.kind_buttons = {}
        group = None
        for key, label in KINDS:
            rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
            rb.set_mode(False)
            rb.get_style_context().add_class("folio-linkkind")
            group = group or rb
            rb.connect("toggled", lambda b, k=key: b.get_active() and self.set_kind(k))
            self.kind_buttons[key] = rb
            side.pack_start(rb, False, False, 0)
        body.pack_start(side, False, False, 0)

        # web page or file
        web = Gtk.Grid(column_spacing=8, row_spacing=6)
        web.attach(Gtk.Label(label="Address:", xalign=0), 0, 0, 1, 1)
        self.address = Gtk.Entry(hexpand=True, activates_default=True)
        self.address.set_placeholder_text("https://")
        web.attach(self.address, 1, 0, 1, 1)
        browse = Gtk.Button(label="Browse for File...")
        browse.connect("clicked", lambda *_: self.browse())
        web.attach(browse, 1, 1, 1, 1)
        browse.set_halign(Gtk.Align.START)
        self.stack.add_named(web, "web")

        # a place in the workbook
        place = Gtk.Grid(column_spacing=8, row_spacing=6)
        place.attach(Gtk.Label(label="Type the cell reference:", xalign=0), 0, 0, 1, 1)
        self.cellref = Gtk.Entry(activates_default=True)
        self.cellref.set_text("A1")
        place.attach(self.cellref, 0, 1, 1, 1)
        place.attach(Gtk.Label(label="Or select a place in this document:", xalign=0), 0, 2, 1, 1)
        self.places = Gtk.TreeStore(str, str)
        tv = Gtk.TreeView(model=self.places, headers_visible=False)
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        b = win.book
        sheets = self.places.append(None, ["Cell Reference", ""])
        for i in range(b.sheet_count):
            info = b.sheet_info(i)
            if info["kind"] in ("work", "macro"):
                self.places.append(sheets, [info["name"], "sheet"])
        names = [n for n in b.names() if not n[3]]
        if names:
            dn = self.places.append(None, ["Defined Names", ""])
            for name, ref, local, hidden in names:
                self.places.append(dn, [name, "name"])
        tv.expand_all()
        self.place_view = tv
        tv.get_selection().connect("changed", lambda *_: self._place_picked())
        sw = Gtk.ScrolledWindow(hexpand=True, vexpand=True)
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(320, 150)
        sw.add(tv)
        place.attach(sw, 0, 3, 1, 1)
        self.stack.add_named(place, "place")

        # e-mail
        mail = Gtk.Grid(column_spacing=8, row_spacing=6)
        mail.attach(Gtk.Label(label="E-mail address:", xalign=0), 0, 0, 1, 1)
        self.email = Gtk.Entry(hexpand=True, activates_default=True)
        mail.attach(self.email, 0, 1, 1, 1)
        mail.attach(Gtk.Label(label="Subject:", xalign=0), 0, 2, 1, 1)
        self.subject = Gtk.Entry(hexpand=True, activates_default=True)
        mail.attach(self.subject, 0, 3, 1, 1)
        self.stack.add_named(mail, "email")
        body.pack_start(self.stack, True, True, 0)
        box.pack_start(body, True, True, 0)

        self._text_touched = bool(text)
        self.text.connect("changed", lambda *_: setattr(self, "_text_touched", True))
        self.address.connect("changed", lambda *_: self._mirror(self.address.get_text()))
        self.email.connect("changed", lambda *_: self._mirror("mailto:" + self.email.get_text()))
        kind = "web"
        t = link.get("target", "")
        if t.startswith("mailto:"):
            kind = "email"
            addr, subj = parse_mailto(t)
            self.email.set_text(addr)
            self.subject.set_text(subj)
        elif t:
            self.address.set_text(t)
        elif link.get("location"):
            kind = "place"
            self._set_location(link["location"])
        self._text_touched = bool(text)
        self.kind_buttons[kind].set_active(True)
        self.set_kind(kind)

    def _mirror(self, s):
        # the text follows the address until it is typed in itself
        if self.text.get_sensitive() and not self._text_touched:
            self.text.set_text(s)
            self._text_touched = False

    def _set_location(self, loc):
        sheet, _, ref = loc.rpartition("!")
        sheet = sheet.strip("'").replace("''", "'")
        self.cellref.set_text(ref or "A1")
        it = self.places.get_iter_first()
        while it is not None:
            ch = self.places.iter_children(it)
            while ch is not None:
                name, what = self.places[ch]
                if (what == "sheet" and name.lower() == sheet.lower()) or (what == "name" and not sheet and
                                                                          name.lower() == loc.lower()):
                    self.place_view.get_selection().select_iter(ch)
                    if what == "name":
                        self.cellref.set_text("")
                    return
                ch = self.places.iter_next(ch)
            it = self.places.iter_next(it)

    def _place_picked(self):
        m, it = self.place_view.get_selection().get_selected()
        if it is None:
            return
        what = m[it][1]
        self.cellref.set_sensitive(what != "name")

    def set_kind(self, key):
        self.kind = key
        self.stack.set_visible_child_name(key)

    def edit_tip(self):
        t = self.win.dialogs.ask_text("Set Hyperlink ScreenTip", "ScreenTip text:", self.tooltip)
        if t is not None:
            self.tooltip = t

    def browse(self):
        d = Gtk.FileChooserNative(title="Link to File", transient_for=self, action=Gtk.FileChooserAction.OPEN,
                                  accept_label="OK", cancel_label="Cancel")
        if d.run() == Gtk.ResponseType.ACCEPT and d.get_filename():
            self.address.set_text(self.win.links.file_address(d.get_filename()))
        d.destroy()

    def place_location(self):
        m, it = self.place_view.get_selection().get_selected()
        ref = self.cellref.get_text().strip().replace("$", "") or "A1"
        if it is not None and m[it][1] == "name":
            return m[it][0]
        sheet = m[it][0] if it is not None and m[it][1] == "sheet" else \
            self.win.book.sheet_info(self.win.grid.sheet)["name"]
        q = "'%s'" % sheet.replace("'", "''") if not sheet.replace("_", "").isalnum() else sheet
        return "%s!%s" % (q, ref)

    def result(self):
        """(target, location, text, tooltip) or a message about what is missing."""
        text = self.text.get_text()
        if self.kind == "web":
            a = self.address.get_text().strip()
            if not a:
                return "Type the address of the web page or file."
            if "://" not in a and not a.startswith(("mailto:", "/", ".", "~")) and "." in a.split("/")[0] \
                    and not os.path.splitext(a)[1].lower() in (".xlsx", ".xlsm", ".csv", ".ods", ".pdf", ".docx", ".txt"):
                a = "https://" + a          # www.example.org
            return (a, "", text or a, self.tooltip)
        if self.kind == "email":
            addr = self.email.get_text().strip()
            if not addr:
                return "Type an e-mail address."
            t = "mailto:" + quote(addr, safe="@.+-_")
            if self.subject.get_text():
                t += "?subject=" + quote(self.subject.get_text())
            return (t, "", text or ("mailto:" + addr), self.tooltip)
        ref = self.cellref.get_text().strip()
        loc = self.place_location()
        m, it = self.place_view.get_selection().get_selected()
        is_name = it is not None and m[it][1] == "name"
        if not is_name and parse_ref(ref.replace("$", "") or "A1") is None:
            return "The cell reference is not valid."
        return ("", loc, text or loc, self.tooltip)


class LinksUi:
    def __init__(self, win):
        self.win = win
        self.opened = []            # what follow() handed to the desktop (kept for tests)
        self.opener = self._open_uri
        self._tip_cell = None

    # ------------------------------------------------------------ the dialog
    def insert(self):
        win = self.win
        g = win.grid
        b, s = win.book, g.sheet
        r, c = g.sel.active
        link = b.link_at(s, r, c)
        rg = tuple(link["range"]) if link else tuple(g.sel.ranges[-1])
        if len(g.sel.ranges) > 1 and not link:
            rg = (r, c, r, c)
        inp = b.input(s, r, c)
        # a cell with a formula or a number keeps what it holds; text can be retyped
        editable = not inp.startswith("=") and not _is_number(inp)
        text = b.display(s, r, c) if inp else ""
        d = HyperlinkDialog(win, link, text, editable)
        d.show_all()
        d.set_kind(d.kind)
        while True:
            resp = d.run()
            if resp == 2:
                d.destroy()
                self.clear(remove_format=True, ranges=[rg])
                return
            if resp != Gtk.ResponseType.OK:
                d.destroy()
                return
            res = d.result()
            if isinstance(res, str):
                win.dialogs.message(res)
                continue
            break
        target, location, shown, tip = res
        new_text = d.text.get_text() if editable else None
        d.destroy()
        with b.group("Insert Hyperlink"):
            try:
                b.set_link(s, rg, target, location, "", tip)
            except ValueError as e:
                win.dialogs.message(str(e))
                return
            if editable and (new_text or shown) != inp:
                v = new_text or shown
                b.set_input(s, r, c, "'" + v if v[:1] in "=+-" or _is_number(v) else v)
            win.restyle("Insert Hyperlink", [{"font": {"color": LINK_COLOR, "underline": "single"}}], [rg])
        g.refresh()
        win.update_title()

    # ------------------------------------------------------------ clear / remove
    def clear(self, remove_format=False, ranges=None):
        win = self.win
        g = win.grid
        ranges = ranges or [tuple(x) for x in g.sel.ranges]
        b, s = win.book, g.sheet
        with b.group("Remove Hyperlinks" if remove_format else "Clear Hyperlinks"):
            had = [tuple(l["range"]) for l in (self._links_in(ranges))]
            n = b.clear_links(s, ranges)
            if remove_format and n:
                # the link's look goes as well: back to the automatic colour, no underline
                win.restyle("Remove Hyperlinks", [{"font": {"color": None, "underline": None}}],
                            [_clip(h, ranges) for h in had if _clip(h, ranges)])
        g.refresh()
        win.update_title()
        return n

    def _links_in(self, ranges):
        b, s = self.win.book, self.win.grid.sheet
        seen, out = set(), []
        for rg in ranges:
            if (rg[2] - rg[0] + 1) * (rg[3] - rg[1] + 1) > 20000:
                continue
            for r in range(rg[0], rg[2] + 1):
                for c in range(rg[1], rg[3] + 1):
                    lk = b.link_at(s, r, c)
                    if lk and tuple(lk["range"]) not in seen:
                        seen.add(tuple(lk["range"]))
                        out.append(lk)
        return out

    # ------------------------------------------------------------ following
    def target_at(self, r, c, sheet=None):
        """What clicking the cell opens: ('link', dict) for a link, ('formula', address) for
        =HYPERLINK(), or None."""
        b = self.win.book
        s = self.win.grid.sheet if sheet is None else sheet
        lk = b.link_at(s, r, c)
        if lk:
            return ("link", lk)
        try:
            t = b.formula_link(s, r, c)
        except Exception:
            t = None
        return ("formula", t) if t else None

    def follow(self, r=None, c=None):
        g = self.win.grid
        if r is None:
            r, c = g.sel.active
        what = self.target_at(r, c)
        if what is None:
            return False
        if what[0] == "link":
            lk = what[1]
            if lk.get("target"):
                return self.open_address(lk["target"])
            return self.go_to(lk["location"])
        t = what[1]
        if t.startswith("#"):
            return self.go_to(t[1:])
        if "://" not in t and not t.startswith("mailto:") and ("!" in t or parse_ref(t.replace("$", ""))):
            return self.go_to(t)
        return self.open_address(t)

    def go_to(self, loc):
        if not self.win.goto_reference(loc):
            self.win.dialogs.message("Reference isn't valid.")
            return False
        return True

    def open_address(self, t):
        uri = t
        if "://" not in t and not t.startswith("mailto:"):
            # a file, relative to the workbook
            base = os.path.dirname(self.win.path) if getattr(self.win, "path", None) else os.getcwd()
            p = os.path.normpath(os.path.join(base, os.path.expanduser(unquote(t))))
            if not os.path.exists(p):
                self.win.dialogs.message("Cannot open the specified file.")
                return False
            uri = GLib.filename_to_uri(p, None)
        self.opened.append(uri)
        try:
            self.opener(uri)
        except Exception as e:
            self.win.dialogs.message("Cannot open %s.\n\n%s" % (t, e))
            return False
        return True

    def _open_uri(self, uri):
        Gtk.show_uri_on_window(self.win, uri, Gtk.get_current_event_time() or int(time.time()))

    def file_address(self, path):
        """A file's address as a link holds it: relative to the workbook when it is near it."""
        wb = getattr(self.win, "path", None)
        if wb:
            rel = os.path.relpath(path, os.path.dirname(wb))
            if not rel.startswith(".." + os.sep + ".."):
                return rel.replace(os.sep, "/")
        return GLib.filename_to_uri(path, None)

    # ------------------------------------------------------------ the pointer
    def tip_for(self, r, c):
        what = self.target_at(r, c)
        if what is None:
            return None
        if what[0] == "link":
            first = what[1].get("tooltip") or describe(what[1])
        else:
            first = what[1]
        return first + " - Click once to follow. Click and hold to select this cell."

    def hover(self, cell):
        """Over a linked cell: a pointing hand and the address."""
        area = self.win.grid.area
        tip = self.tip_for(*cell) if cell is not None else None
        if (cell, tip) != self._tip_cell:
            self._tip_cell = (cell, tip)
            area.set_tooltip_text(tip)
        return tip is not None

    # ------------------------------------------------------------ context menu
    def menu_items(self):
        g = self.win.grid
        r, c = g.sel.active
        if self.win.book.link_at(g.sheet, r, c):
            return [("Edit Hyperlink...", "link"), ("Open Hyperlink", "open-hyperlink"),
                    ("Remove Hyperlink", "remove-hyperlinks")]
        return [("Link", "link")]


def _clip(a, ranges):
    for b in ranges:
        r1, c1, r2, c2 = max(a[0], b[0]), max(a[1], b[1]), min(a[2], b[2]), min(a[3], b[3])
        if r1 <= r2 and c1 <= c2:
            return (r1, c1, r2, c2)
    return None


def _is_number(t):
    try:
        float(t.replace(",", ""))
        return bool(t.strip())
    except ValueError:
        return False

