"""View and Page Layout extras: Custom Views, Background, the Office Clipboard pane and
Copy as Picture."""
import os
import tempfile
import uuid
import xml.etree.ElementTree as ET
from xml.sax.saxutils import quoteattr

from gi.repository import Gdk, GdkPixbuf, Gtk

from .core import MAX_COLS, MAX_ROWS, a1, col_name, parse_ref, range_a1

NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"


def _sref(name):
    import re
    return name if re.match(r"^[A-Za-z_][A-Za-z0-9_.]*$", name) else "'%s'" % name.replace("'", "''")


# ------------------------------------------------------------------ Custom Views
def _guid_name(guid):
    return "Z_" + guid.strip("{}").replace("-", "_").upper() + "_"


def views(win):
    """[(name, guid, active sheet id)] of the workbook's custom views."""
    x = win.book.workbook_element("customWorkbookViews")
    if not x:
        return []
    try:
        root = ET.fromstring(x)
    except ET.ParseError:
        return []
    return [(e.get("name", ""), e.get("guid", ""), e.get("activeSheetId")) for e in root if e.tag.endswith("customWorkbookView")]


def _sheet_views(win, s):
    x = win.book.sheet_element(s, "customSheetViews")
    if not x:
        return []
    try:
        root = ET.fromstring(x)
    except ET.ParseError:
        return []
    out = []
    for e in root:
        if e.tag.endswith("customSheetView"):
            out.append(e)
    return out


def _write_sheet_views(win, s, elems):
    b = win.book
    if not elems:
        b.set_sheet_element(s, "customSheetViews", None)
        return
    ET.register_namespace("", NS)
    parts = []
    for e in elems:
        e.tag = "{%s}customSheetView" % NS
        for c in e.iter():
            if not c.tag.startswith("{"):
                c.tag = "{%s}%s" % (NS, c.tag)
        parts.append(ET.tostring(e, encoding="unicode"))
    b.set_sheet_element(s, "customSheetViews", '<customSheetViews xmlns="%s">%s</customSheetViews>' % (NS, "".join(
        p.replace(' xmlns="%s"' % NS, "") for p in parts)))


def add_view(win, name, print_settings=True, hidden=True):
    b, g = win.book, win.grid
    win.store_view_state()
    guid = "{%s}" % str(uuid.uuid4()).upper()
    base = _guid_name(guid)
    for s in range(b.sheet_count):
        st = g.state(s)
        e = ET.Element("{%s}customSheetView" % NS, {"guid": guid, "scale": str(int(round(st.zoom * 100)))})
        if not st.show_grid:
            e.set("showGridLines", "0")
        if not st.show_headers:
            e.set("showRowCol", "0")
        if st.view_mode == "pageBreakPreview":
            e.set("view", "pageBreakPreview")
            e.set("showPageBreaks", "1")
        elif st.view_mode == "pageLayout":
            e.set("view", "pageLayout")
        e.set("topLeftCell", a1(st.top_row, st.left_col))
        if b.sheet_info(s).get("state", "visible") != "visible":
            e.set("state", "hidden")
        rows, cols = b.hidden_spans(s)
        if hidden and rows:
            e.set("hiddenRows", "1")
        if hidden and cols:
            e.set("hiddenColumns", "1")
        if not print_settings:
            pass
        sel = ET.SubElement(e, "{%s}selection" % NS, {"activeCell": a1(*st.sel.active),
                                                   "sqref": " ".join(range_a1(tuple(r)) for r in st.sel.ranges)})
        sel.tail = None
        if print_settings:
            pm = b.sheet_element(s, "pageMargins")
            if pm:
                try:
                    e.append(ET.fromstring(pm))
                except ET.ParseError:
                    pass
        others = [x for x in _sheet_views(win, s) if x.get("guid") != guid]
        _write_sheet_views(win, s, others + [e])
        sn = _sref(b.sheet_info(s)["name"])
        if hidden and rows:
            b.define_name(base + ".wvu.Rows", ",".join("%s!$%d:$%d" % (sn, r1 + 1, r2 + 1) for r1, r2 in rows), s)
            b.set_name_hidden(base + ".wvu.Rows", True, s)
        if hidden and cols:
            b.define_name(base + ".wvu.Cols", ",".join("%s!$%s:$%s" % (sn, col_name(c1), col_name(c2)) for c1, c2 in cols), s)
            b.set_name_hidden(base + ".wvu.Cols", True, s)
    items = [v for v in views(win) if v[0].lower() != name.lower()]
    active_id = g.sheet + 1
    xs = "".join('<customWorkbookView name=%s guid=%s maximized="1" windowWidth="1600" windowHeight="900" activeSheetId=%s/>'
                 % (quoteattr(n), quoteattr(gd), quoteattr(str(a or 1))) for n, gd, a in items)
    xs += '<customWorkbookView name=%s guid=%s maximized="1" windowWidth="1600" windowHeight="900" activeSheetId="%s"/>' % (
        quoteattr(name), quoteattr(guid), active_id)
    b.set_workbook_element("customWorkbookViews", '<customWorkbookViews xmlns="%s">%s</customWorkbookViews>' % (NS, xs))
    win.update_title()
    return guid


def show_view(win, name):
    b, g = win.book, win.grid
    v = next((x for x in views(win) if x[0].lower() == name.lower()), None)
    if v is None:
        return False
    guid = v[1]
    base = _guid_name(guid).lower()
    names = {(n[0].lower(), n[2]): n[1] for n in b.names()}
    b.begin_group("Show View")
    try:
        for s in range(b.sheet_count):
            e = next((x for x in _sheet_views(win, s) if x.get("guid") == guid), None)
            if e is None:
                continue
            st = g.state(s)
            st.zoom = int(e.get("scale", "100")) / 100.0
            st.show_grid = e.get("showGridLines", "1") not in ("0", "false")
            st.show_headers = e.get("showRowCol", "1") not in ("0", "false")
            st.view_mode = e.get("view", "normal")
            tl = parse_ref(e.get("topLeftCell", "A1"))
            if tl:
                st.top_row, st.left_col = tl[0], tl[1]
            sel = next((c for c in e if c.tag.endswith("selection")), None)
            if sel is not None:
                rs = [tuple(parse_ref(t)) for t in (sel.get("sqref") or "").split() if parse_ref(t)]
                ac = parse_ref(sel.get("activeCell", "A1"))
                if rs and ac:
                    st.sel.ranges = rs
                    st.sel.active = tuple(ac[0:2])
            # hidden rows and columns as they were
            if e.get("hiddenRows") or (base + ".wvu.rows", s) in names:
                b.set_rows_hidden(s, 0, MAX_ROWS - 1, False)
            if e.get("hiddenColumns") or (base + ".wvu.cols", s) in names:
                b.set_cols_hidden(s, 0, MAX_COLS - 1, False)
            for key, rows in ((".wvu.rows", True), (".wvu.cols", False)):
                text = names.get((base + key, s))
                if not text:
                    continue
                for part in text.split(","):
                    ref = part.rsplit("!", 1)[-1].replace("$", "")
                    a, _, bb = ref.partition(":")
                    bb = bb or a
                    try:
                        if rows:
                            b.set_rows_hidden(s, int(a) - 1, int(bb) - 1, True)
                        else:
                            ca, cb = parse_ref(a + "1"), parse_ref(bb + "1")
                            b.set_cols_hidden(s, ca[1], cb[1], True)
                    except (ValueError, TypeError):
                        continue
    finally:
        b.end_group()
    try:
        act = min(max(int(v[2] or 1) - 1, 0), b.sheet_count - 1)
    except ValueError:
        act = g.sheet
    g.set_sheet(act)
    win.sheet_changed()
    g.refresh()
    win.update_title()
    return True


def delete_view(win, name):
    b = win.book
    v = next((x for x in views(win) if x[0].lower() == name.lower()), None)
    if v is None:
        return False
    guid = v[1]
    base = _guid_name(guid).lower()
    for s in range(b.sheet_count):
        els = _sheet_views(win, s)
        keep = [e for e in els if e.get("guid") != guid]
        if len(keep) != len(els):
            _write_sheet_views(win, s, keep)
    for n in b.names():
        if n[0].lower().startswith(base):
            b.delete_name(n[0], n[2])
    rest = [x for x in views(win) if x[1] != guid]
    if rest:
        xs = "".join('<customWorkbookView name=%s guid=%s maximized="1" windowWidth="1600" windowHeight="900" activeSheetId=%s/>'
                     % (quoteattr(n), quoteattr(gd), quoteattr(str(a or 1))) for n, gd, a in rest)
        b.set_workbook_element("customWorkbookViews", '<customWorkbookViews xmlns="%s">%s</customWorkbookViews>' % (NS, xs))
    else:
        b.set_workbook_element("customWorkbookViews", None)
    win.update_title()
    return True


def custom_views(win):
    """View > Custom Views: Show, Close, Add, Delete."""
    d = Gtk.Dialog(title="Custom Views", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="Views:", xalign=0), False, False, 0)
    store = Gtk.ListStore(str)
    tv = Gtk.TreeView(model=store)
    tv.set_headers_visible(False)
    tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(300, 160)
    sw.add(tv)
    row = Gtk.Box(spacing=8)
    row.pack_start(sw, True, True, 0)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    row.pack_start(side, False, False, 0)
    box.pack_start(row, True, True, 0)

    def fill():
        store.clear()
        for n, _g, _a in views(win):
            store.append([n])
        if len(store):
            tv.get_selection().select_path(Gtk.TreePath(0))

    def picked():
        m, it = tv.get_selection().get_selected()
        return m[it][0] if it is not None else None
    result = {}

    def show(*_):
        n = picked()
        if n:
            result["show"] = n
            d.response(Gtk.ResponseType.OK)

    def add(*_):
        nd = Gtk.Dialog(title="Add View", transient_for=d, modal=True)
        nd.get_style_context().add_class("folio-dialog")
        nd.add_button("OK", Gtk.ResponseType.OK)
        nd.add_button("Cancel", Gtk.ResponseType.CANCEL)
        nd.set_default_response(Gtk.ResponseType.OK)
        nb = nd.get_content_area()
        nb.set_border_width(12)
        nb.set_spacing(6)
        nb.pack_start(Gtk.Label(label="Name:", xalign=0), False, False, 0)
        e = Gtk.Entry(activates_default=True)
        nb.pack_start(e, False, False, 0)
        nb.pack_start(Gtk.Label(label="Include in view", xalign=0), False, False, 0)
        ps = Gtk.CheckButton(label="Print settings")
        ps.set_active(True)
        hr = Gtk.CheckButton(label="Hidden rows, columns and filter settings")
        hr.set_active(True)
        nb.pack_start(ps, False, False, 0)
        nb.pack_start(hr, False, False, 0)
        nd.name_e, nd.print_cb, nd.hidden_cb = e, ps, hr
        nd.show_all()
        r = nd.run()
        name, p, h = e.get_text().strip(), ps.get_active(), hr.get_active()
        nd.destroy()
        if r == Gtk.ResponseType.OK and name:
            add_view(win, name, p, h)
            result["added"] = name
            d.response(Gtk.ResponseType.CLOSE)

    def delete(*_):
        n = picked()
        if n:
            delete_view(win, n)
            fill()
    for label, fn in (("Show", show), ("Close", lambda *_: d.response(Gtk.ResponseType.CLOSE)), ("Add...", add),
                      ("Delete", delete)):
        bt = Gtk.Button(label=label)
        bt.connect("clicked", fn)
        side.pack_start(bt, False, False, 0)
    d.store, d.tree, d.show_view, d.add_view, d.delete_view = store, tv, show, add, delete
    fill()
    d.show_all()
    r = d.run()
    if r == Gtk.ResponseType.OK and "show" not in result:
        n = picked()
        if n:
            result["show"] = n
    d.destroy()
    if "show" in result:
        show_view(win, result["show"])
    return result


# ------------------------------------------------------------------ Background
def background(win, path=None):
    b, g = win.book, win.grid
    s = g.sheet
    if b.has_background(s):
        b.set_background(s, None)
        win.ribbon.update("background", "false")
        _bg_label(win, False)
        g.refresh()
        win.update_title()
        return "deleted"
    if path is None:
        d = Gtk.FileChooserNative(title="Sheet Background", transient_for=win, action=Gtk.FileChooserAction.OPEN)
        f = Gtk.FileFilter()
        f.set_name("Pictures")
        for pat in ("*.png", "*.jpg", "*.jpeg", "*.gif", "*.bmp"):
            f.add_pattern(pat)
            f.add_pattern(pat.upper())
        d.add_filter(f)
        path = d.get_filename() if d.run() == Gtk.ResponseType.ACCEPT else None
        d.destroy()
        if not path:
            return None
    try:
        with open(path, "rb") as fh:
            data = fh.read()
        b.set_background(s, data)
    except (OSError, ValueError) as e:
        win.dialogs.message(str(e), "Sheet Background")
        return None
    _bg_label(win, True)
    g.refresh()
    win.update_title()
    return "set"


def _bg_label(win, has):
    for c in getattr(win.ribbon, "controls", []):
        if getattr(c, "action", None) == "background" and hasattr(c, "set_label_text"):
            c.set_label_text("Delete\nBackground" if has else "Background")


def sync_background(win):
    if win.book is not None:
        _bg_label(win, win.book.has_background(win.grid.sheet))


# ------------------------------------------------------------------ Office Clipboard
class ClipItem:
    def __init__(self, text, data, html=None, picture=None):
        self.text, self.data, self.html, self.picture = text, data, html, picture


HISTORY = []          # newest first, at most 24 (shared by the windows)


def remember(item):
    HISTORY.insert(0, item)
    del HISTORY[24:]
    for w in Gtk.Window.list_toplevels():
        pane = getattr(w, "clipboard_pane", None)
        if pane is not None and pane.get_visible():
            pane.load()


class ClipboardPane(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.win = win
        self.set_size_request(260, -1)
        self.get_style_context().add_class("folio-pane")
        self.set_name("clipboard-pane")
        head = Gtk.Box(spacing=6)
        head.set_border_width(6)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<span size='large'>Clipboard</span>")
        lab.get_style_context().add_class("folio-pane-title")
        head.pack_start(lab, True, True, 0)
        close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda _b: self.hide())
        head.pack_end(close, False, False, 0)
        self.pack_start(head, False, False, 0)
        tools = Gtk.Box(spacing=4)
        tools.set_margin_start(6)
        pa = Gtk.Button(label="Paste All")
        pa.connect("clicked", lambda *_: self.paste_all())
        ca = Gtk.Button(label="Clear All")
        ca.connect("clicked", lambda *_: self.clear())
        tools.pack_start(pa, False, False, 0)
        tools.pack_start(ca, False, False, 0)
        self.pack_start(tools, False, False, 0)
        self.count = Gtk.Label(xalign=0, margin_start=8)
        self.pack_start(self.count, False, False, 0)
        self.pack_start(Gtk.Label(label="Click an item to paste:", xalign=0, margin_start=8), False, False, 0)
        self.list = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_vexpand(True)
        sw.add(self.list)
        self.pack_start(sw, True, True, 0)

    def load(self):
        for c in self.list.get_children():
            self.list.remove(c)
        self.count.set_text("%d of 24 items" % len(HISTORY))
        for k, it in enumerate(HISTORY):
            t = (it.text or "").strip().replace("\t", "  ").replace("\r\n", " ").replace("\n", " ")
            bt = Gtk.Button(label=(t[:60] + "…") if len(t) > 60 else (t or "(picture)"))
            bt.set_relief(Gtk.ReliefStyle.NONE)
            bt.get_child().set_xalign(0)
            bt.connect("clicked", lambda _b, k=k: self.paste(k))
            self.list.pack_start(bt, False, False, 0)
        self.list.show_all()

    def paste(self, k):
        from . import clip_ui
        it = HISTORY[k]
        win = self.win
        g = win.grid
        dst = tuple(g.sel.ranges[0])
        if it.data is not None:
            out = win.book.paste_from_bytes(it.data, g.sheet, dst, None)
            g.select_range(tuple(out), tuple(out[0:2]))
        elif it.picture is not None:
            clip_ui._place_picture(win, it.picture)
        else:
            with win.book.group("Paste"):
                for i, line in enumerate((it.text or "").rstrip("\r\n").split("\n")):
                    for j, v in enumerate(line.rstrip("\r").split("\t")):
                        win.book.set_input(g.sheet, dst[0] + i, dst[1] + j, v)
        g.refresh()
        win.update_title()
        return True

    def paste_all(self):
        """Every item, oldest first, one under the other."""
        win = self.win
        g = win.grid
        r, c = g.sel.active
        with win.book.group("Paste All"):
            for it in reversed(HISTORY):
                if it.data is None:
                    continue
                out = win.book.paste_from_bytes(it.data, g.sheet, (r, c, r, c), None)
                r = out[2] + 1
        g.refresh()
        win.update_title()

    def clear(self):
        del HISTORY[:]
        self.load()


def clipboard_pane(win):
    pane = getattr(win, "clipboard_pane", None)
    if pane is None:
        pane = win.clipboard_pane = ClipboardPane(win)
        win.mid.pack_start(pane, False, False, 0)
    if pane.get_visible() and not getattr(win, "_pane_force", False):
        pane.hide()
        return pane
    pane.show_all()
    pane.load()
    return pane


# ------------------------------------------------------------------ Copy as Picture
def range_png(win, s, rg, printed=False):
    """A cell range drawn as a PNG (as on screen, or as printed: no gridlines unless printed)."""
    b = win.book
    r1, c1, r2, c2 = rg
    w = b.col_x(s, c2 + 1) - b.col_x(s, c1)
    h = b.row_y(s, r2 + 1) - b.row_y(s, r1)
    if w < 1 or h < 1:
        return None
    st = win.grid.state(s)
    path = os.path.join(tempfile.mkdtemp(prefix="folio-pic-"), "range.png")
    show_grid = st.show_grid if not printed else bool(b.page_setup(s).get("gridlines", False))
    b.render_png(s, path, {"width": float(w) + 1, "height": float(h) + 1, "zoom": 1.0, "top_row": r1, "left_col": c1,
                           "show_headers": False, "show_grid": show_grid, "print": True, "sel": [], "active": (MAX_ROWS - 1, MAX_COLS - 1)})
    return path


def copy_as_picture(win):
    from . import clip_ui
    g = win.grid
    if g.sel_objects and clip_ui._copy_picture_object(win):
        return "object"
    d = Gtk.Dialog(title="Copy Picture", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(4)
    box.pack_start(Gtk.Label(label="Appearance", xalign=0), False, False, 0)
    screen = Gtk.RadioButton.new_with_label_from_widget(None, "As shown on screen")
    printed = Gtk.RadioButton.new_with_label_from_widget(screen, "As shown when printed")
    box.pack_start(screen, False, False, 0)
    box.pack_start(printed, False, False, 0)
    box.pack_start(Gtk.Label(label="Format", xalign=0), False, False, 4)
    pic = Gtk.RadioButton.new_with_label_from_widget(None, "Picture")
    bmp = Gtk.RadioButton.new_with_label_from_widget(pic, "Bitmap")
    box.pack_start(pic, False, False, 0)
    box.pack_start(bmp, False, False, 0)
    d.printed = printed
    d.show_all()
    r = d.run()
    pr = printed.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return None
    rg = tuple(g.sel.ranges[0])
    ext = win.book.used_extent(g.sheet) or (0, 0)
    rg = (rg[0], rg[1], min(rg[2], max(ext[0], rg[0])), min(rg[3], max(ext[1], rg[1])))
    path = range_png(win, g.sheet, rg, pr)
    if not path:
        return None
    pb = GdkPixbuf.Pixbuf.new_from_file(path)
    Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_image(pb)
    with open(path, "rb") as fh:
        remember(ClipItem("", None, picture=fh.read()))
    win.status.extra.set_text("Copied the cells as a picture.")
    return path



