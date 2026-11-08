"""Insert > SmartArt, Signature Line and Object.

SmartArt graphics are drawn as a group of shapes (laid out from the text typed for them), so
any spreadsheet program shows and edits them; the layout and text are kept with the group so
Folio Sheet can lay them out again. A signature line is a picture of the line with the signer's
details, signed by typing a name. Insert > Object puts a file on the sheet as its icon; the
file itself travels inside the icon's picture (or, linked, its path), and a double-click
opens it."""
import io
import math
import os
import struct
import subprocess
import tempfile
import zlib
from xml.sax.saxutils import escape, quoteattr

import cairo
from gi.repository import Gtk

SMARTART_URI = "{7E3D2A41-5C8B-4E6F-9A10-F01105A12701}"
NS_FS = "urn:folio:smartart"
ACCENT = 0x4472C4
ACCENT2 = 0xA5C3E8

LAYOUTS = [("list", "List", "Basic Block List"), ("process", "Process", "Basic Process"),
           ("cycle", "Cycle", "Basic Cycle"), ("hierarchy", "Hierarchy", "Hierarchy"),
           ("relationship", "Relationship", "Basic Venn"), ("matrix", "Matrix", "Basic Matrix"),
           ("pyramid", "Pyramid", "Basic Pyramid")]


# ------------------------------------------------------------------ SmartArt
def _items(text):
    """Lines of the text pane: (level, text); a leading tab (or two spaces) is one level down."""
    out = []
    for line in text.splitlines():
        if not line.strip():
            continue
        raw = line.replace("    ", "\t").replace("  ", "\t")
        lvl = len(raw) - len(raw.lstrip("\t"))
        out.append((min(lvl, 3), line.strip()))
    return out


def _shape(b, s, prst, x, y, w, h, text, fill=ACCENT, size=None):
    i = b.insert_shape(s, prst, float(x), float(y), float(max(w, 4)), float(max(h, 4)))
    b.shape_set_fill(s, [i], fill)
    if fill == 0x7F7F7F:
        b.shape_set_line(s, [i], None, True)        # a connector: no outline
    else:
        b.shape_set_line(s, [i], 0xFFFFFF, False, 1.5)
    if text:
        b.shape_set_text(s, i, text)
    b.shape_set_text_format(s, [i], None, None, size, 0xFFFFFF, 1)
    b.shape_set_anchor(s, [i], "c")
    return i


def build_smartart(win, layout, text, x, y, w=420.0, h=260.0):
    """Lay out the shapes of a SmartArt graphic at sheet pixels (x, y); returns the group's index."""
    b, g = win.book, win.grid
    s = g.sheet
    items = _items(text) or [(0, "[Text]")]
    top = [t for lv, t in items if lv == 0] or [items[0][1]]
    made = []
    b.begin_group("Insert SmartArt")
    try:
        n = len(top)
        if layout == "list":
            cols = min(3, n)
            rows = math.ceil(n / cols)
            bw, bh = w / cols * 0.9, h / rows * 0.86
            for k, t in enumerate(top):
                cx = x + (k % cols) * (w / cols) + (w / cols - bw) / 2
                cy = y + (k // cols) * (h / rows) + (h / rows - bh) / 2
                made.append(_shape(b, s, "rect", cx, cy, bw, bh, t))
        elif layout == "process":
            gap = w * 0.06
            bw = (w - gap * (n - 1)) / n
            bh = min(h, bw * 0.75)
            cy = y + (h - bh) / 2
            for k, t in enumerate(top):
                cx = x + k * (bw + gap)
                made.append(_shape(b, s, "roundRect", cx, cy, bw, bh, t))
                if k < n - 1:
                    made.append(_shape(b, s, "rightArrow", cx + bw + gap * 0.15, cy + bh / 2 - gap * 0.35, gap * 0.7, gap * 0.7, "", ACCENT2))
        elif layout == "cycle":
            r = min(w, h) / 2 - 40
            ox, oy = x + w / 2, y + h / 2
            d = min(90.0, 2 * r * math.sin(math.pi / max(n, 2)) * 0.8)
            for k, t in enumerate(top):
                a = -math.pi / 2 + 2 * math.pi * k / n
                made.append(_shape(b, s, "ellipse", ox + r * math.cos(a) - d / 2, oy + r * math.sin(a) - d / 2, d, d, t))
                a2 = a + math.pi / n
                ar = 22.0
                i = _shape(b, s, "rightArrow", ox + r * math.cos(a2) - ar / 2, oy + r * math.sin(a2) - ar / 2, ar, ar * 0.8, "", ACCENT2)
                b.set_object_props(s, [i], {"rot": math.degrees(a2) + 90.0}, "Rotate")
                made.append(i)
        elif layout == "hierarchy":
            root = items[0][1]
            kids = [t for lv, t in items[1:] if lv >= 1] or top[1:]
            bw, bh = min(140.0, w / max(len(kids), 1) * 0.85), 50.0
            made.append(_shape(b, s, "roundRect", x + w / 2 - bw / 2, y, bw, bh, root))
            if kids:
                step = w / len(kids)
                ky = y + h - bh
                made.append(_shape(b, s, "rect", x + step / 2, y + bh + (ky - y - bh) / 2, step * (len(kids) - 1) + 2, 2, "", 0x7F7F7F))
                made.append(_shape(b, s, "rect", x + w / 2 - 1, y + bh, 2, (ky - y - bh) / 2, "", 0x7F7F7F))
                for k, t in enumerate(kids):
                    cx = x + k * step + step / 2
                    made.append(_shape(b, s, "rect", cx - 1, y + bh + (ky - y - bh) / 2, 2, (ky - y - bh) / 2, "", 0x7F7F7F))
                    made.append(_shape(b, s, "roundRect", cx - bw / 2, ky, bw, bh, t))
        elif layout == "relationship":
            d = min(h, w / (1 + 0.65 * (n - 1)))
            for k, t in enumerate(top):
                made.append(_shape(b, s, "ellipse", x + k * d * 0.65, y + (h - d) / 2, d, d, t,
                                   [0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47][k % 6]))
        elif layout == "matrix":
            cells = (top + [""] * 4)[:4]
            for k, t in enumerate(cells):
                made.append(_shape(b, s, "roundRect", x + (k % 2) * w / 2 + 3, y + (k // 2) * h / 2 + 3, w / 2 - 6, h / 2 - 6, t,
                                   [0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000][k]))
        else:  # pyramid
            lh = h / n
            for k, t in enumerate(top):
                top_w = w * k / n
                bot_w = w * (k + 1) / n
                prst = "triangle" if k == 0 else "trapezoid"
                made.append(_shape(b, s, prst, x + (w - bot_w) / 2, y + k * lh, bot_w, lh - 2, t))
                del top_w
        gi = b.group_objects(s, made) if len(made) > 1 else made[0]
        if len(made) > 1:
            n_d = sum(1 for o in b.objects(s) if o["name"].startswith("Diagram "))
            b.set_object_props(s, [gi], {"name": "Diagram %d" % (n_d + 1), "descr": "SmartArt graphic: " + ", ".join(top)}, "SmartArt")
            inner = '<fs:smartArt xmlns:fs="%s" layout=%s>%s</fs:smartArt>' % (NS_FS, quoteattr(layout), escape(text))
            b.set_object_ext(s, gi, SMARTART_URI, inner, "SmartArt")
    finally:
        b.end_group()
    g.sel_objects = [gi]
    g.refresh()
    win.update_title()
    win.on_object_selection()
    return gi


def smartart_of(win, i):
    """(layout, text) kept with a SmartArt group, or None."""
    import re
    from xml.sax.saxutils import unescape
    x = win.book.object_ext(win.grid.sheet, i, SMARTART_URI)
    if not x:
        return None
    m = re.search(r'<(?:\w+:)?smartArt\b[^>]*\blayout="([^"]*)"[^>]*?(?:/>|>(.*?)</(?:\w+:)?smartArt>)', x, re.S)
    if not m:
        return None
    return unescape(m.group(1), {"&quot;": '"'}), unescape(m.group(2) or "", {"&quot;": '"'})


def smartart_dialog(win, layout="list", text="", title="Choose a SmartArt Graphic"):
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    row = Gtk.Box(spacing=12)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    group = None
    radios = {}
    for key, cat, name in LAYOUTS:
        rb = Gtk.RadioButton.new_with_label_from_widget(group, "%s: %s" % (cat, name))
        group = group or rb
        radios[key] = rb
        side.pack_start(rb, False, False, 0)
    radios.get(layout, radios["list"]).set_active(True)
    row.pack_start(side, False, False, 0)
    right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    right.pack_start(Gtk.Label(label="Type your text here (one item per line; Tab for a lower level):", xalign=0), False, False, 0)
    tv = Gtk.TextView()
    tv.get_buffer().set_text(text or "Item 1\nItem 2\nItem 3")
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(280, 180)
    sw.add(tv)
    right.pack_start(sw, True, True, 0)
    row.pack_start(right, True, True, 0)
    box.pack_start(row, True, True, 0)
    d.radios, d.text = radios, tv
    d.show_all()
    r = d.run()
    lay = next((k for k, rb in radios.items() if rb.get_active()), "list")
    buf = tv.get_buffer()
    t = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)
    d.destroy()
    return (lay, t) if r == Gtk.ResponseType.OK else None


def insert_smartart(win):
    res = smartart_dialog(win)
    if not res:
        return None
    g, b = win.grid, win.book
    r, c = g.sel.active
    return build_smartart(win, res[0], res[1], b.col_x(g.sheet, c), b.row_y(g.sheet, r))


def edit_smartart(win, i):
    cur = smartart_of(win, i)
    if cur is None:
        return False
    res = smartart_dialog(win, cur[0], cur[1], "Edit SmartArt")
    if not res:
        return True
    b, g = win.book, win.grid
    x, y, w, h = b.objects(g.sheet)[i]["rect"]
    b.begin_group("Edit SmartArt")
    try:
        b.delete_objects(g.sheet, [i])
        build_smartart(win, res[0], res[1], x, y, w, h)
    finally:
        b.end_group()
    return True


# ------------------------------------------------------------------ pictures made here
def _png_with_chunk(surface, tag, payload):
    """The surface as PNG bytes with a private ancillary chunk carrying `payload`."""
    buf = io.BytesIO()
    surface.write_to_png(buf)
    data = buf.getvalue()
    if not payload:
        return data
    chunk = struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
    k = data.rfind(b"IEND") - 4
    return data[:k] + chunk + data[k:]


def _png_add_chunk(data, tag, payload):
    """The same private chunk, added to PNG bytes that are already made."""
    if not payload or not data:
        return data
    chunk = struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
    k = data.rfind(b"IEND") - 4
    return data[:k] + chunk + data[k:]


def _pic(win, i):
    got = win.book.picture_bytes(win.grid.sheet, i)
    return bytes(got[0]) if got else b""


def png_chunk(data, tag):
    """A private chunk's bytes from PNG data, or None."""
    if not data or data[:8] != b"\x89PNG\r\n\x1a\n":
        return None
    i = 8
    while i + 8 <= len(data):
        n = struct.unpack(">I", data[i:i + 4])[0]
        t = data[i + 4:i + 8]
        if t == tag:
            return data[i + 8:i + 8 + n]
        i += 12 + n
    return None


def _text(cr, text, x, y, size=13, bold=False, color=(0, 0, 0)):
    cr.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
    cr.set_font_size(size)
    cr.set_source_rgb(*color)
    cr.move_to(x, y)
    cr.show_text(text)


def signature_png(signer="", title="", email="", signed_by=None, date=None):
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, 384, 192)
    cr = cairo.Context(surf)
    cr.set_source_rgb(1, 1, 1)
    cr.paint()
    if signed_by:
        _text(cr, signed_by, 40, 108, 30, False, (0.05, 0.1, 0.35))
    _text(cr, "X", 14, 120, 34, False, (0.3, 0.3, 0.3))
    cr.set_source_rgb(0.2, 0.2, 0.2)
    cr.set_line_width(1.5)
    cr.move_to(12, 128)
    cr.line_to(372, 128)
    cr.stroke()
    yy = 148
    for t in (signer, title, email):
        if t:
            _text(cr, t, 14, yy, 13)
            yy += 17
    if date:
        _text(cr, date, 260, 148, 12, False, (0.3, 0.3, 0.3))
    if signed_by:
        _text(cr, "Signed by: " + signed_by, 14, 20, 11, False, (0.35, 0.35, 0.35))
    meta = "\n".join((signer, title, email, signed_by or "", date or "")).encode("utf-8")
    return _png_with_chunk(surf, b"flSg", meta)


def signature_setup(win):
    d = Gtk.Dialog(title="Signature Setup", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    grid = Gtk.Grid(column_spacing=8, row_spacing=6)
    ents = []
    for k, lab in enumerate(("Suggested signer:", "Suggested signer's title:", "Suggested signer's e-mail address:",
                             "Instructions to the signer:")):
        e = Gtk.Entry()
        e.set_hexpand(True)
        grid.attach(Gtk.Label(label=lab, xalign=0), 0, k, 1, 1)
        grid.attach(e, 1, k, 1, 1)
        ents.append(e)
    ents[3].set_text("Before signing this document, verify that the content you are signing is correct.")
    box.pack_start(grid, False, False, 0)
    show_date = Gtk.CheckButton(label="Show sign date in signature line")
    show_date.set_active(True)
    box.pack_start(show_date, False, False, 0)
    d.entries, d.show_date = ents, show_date
    d.show_all()
    r = d.run()
    vals = [e.get_text() for e in ents]
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return None
    b, g = win.book, win.grid
    rr, cc = g.sel.active
    data = signature_png(vals[0], vals[1], vals[2])
    i = b.insert_picture(g.sheet, data, b.col_x(g.sheet, cc), b.row_y(g.sheet, rr), 384.0, 192.0)
    b.set_object_props(g.sheet, [i], {"name": "Signature Line %d" % (i + 1), "descr": "Signature line for " + (vals[0] or "the signer")},
                       "Signature Line")
    g.sel_objects = [i]
    g.refresh()
    win.update_title()
    return i


def sign(win, i, name=None):
    """Sign a signature line: a typed name, or a certificate the signer points at (S3-16)."""
    import datetime
    b, g = win.book, win.grid
    data = _pic(win, i)
    meta = png_chunk(data, b"flSg")
    if meta is None:
        return False
    signer, title, email, _signed, _date = (meta.decode("utf-8", "replace").split("\n") + [""] * 5)[:5]
    if png_chunk(data, SIG_CHUNK) is not None:
        # already signed with a certificate: say what it says rather than signing over it
        return show_signature(win, i)
    if name is None:
        got = sign_dialog(win, signer)
        if got is None:
            return True
        kind, name, p12, password = got
        if kind == "certificate":
            ok, why = sign_with_certificate(win, i, p12, password)
            if not ok:
                win.dialogs.message(why, "Sign")
            return True
    x, y, w, h = b.objects(g.sheet)[i]["rect"]
    new = signature_png(signer, title, email, name, datetime.date.today().strftime("%m/%d/%Y"))
    b.begin_group("Sign")
    try:
        b.delete_objects(g.sheet, [i])
        k = b.insert_picture(g.sheet, new, x, y, w, h)
        b.set_object_rect(g.sheet, k, x, y, w, h, "Sign")
        b.set_object_props(g.sheet, [k], {"name": "Signature Line %d" % (k + 1), "descr": "Signed by " + name}, "Sign")
    finally:
        b.end_group()
    g.sel_objects = [k]
    g.refresh()
    win.update_title()
    return True


# ------------------------------------------------------------------ Object
def _icon_png(name, payload, tag):
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, 96, 96)
    cr = cairo.Context(surf)
    cr.set_source_rgba(1, 1, 1, 0)
    cr.paint()
    # a page with a folded corner
    cr.move_to(28, 6)
    cr.line_to(60, 6)
    cr.line_to(72, 18)
    cr.line_to(72, 66)
    cr.line_to(28, 66)
    cr.close_path()
    cr.set_source_rgb(1, 1, 1)
    cr.fill_preserve()
    cr.set_source_rgb(0.3, 0.3, 0.3)
    cr.set_line_width(1.5)
    cr.stroke()
    ext = os.path.splitext(name)[1].lstrip(".").upper()[:4]
    _text(cr, ext, 34, 46, 13, True, (0.48, 0.35, 0.0))
    label = name if len(name) <= 16 else name[:13] + "..."
    cr.select_font_face("Sans")
    cr.set_font_size(10)
    xb = cr.text_extents(label)
    _text(cr, label, max(1, 48 - xb.width / 2), 84, 10)
    return _png_with_chunk(surf, tag, payload)



# ---------------------------------------------------------------- Insert > Object > Create New
# There is no OLE here, so "Create New" offers the kinds of document the Folio apps on this
# computer can make: each ships a blank template, and a new object starts as a copy of it.
# Once embedded it behaves like any other object — double-clicking opens it in its own app.
NEW_OBJECT_KINDS = [
    ("Folio Sheet Workbook", "folio-sheet", "Blank.xlsx", "Workbook"),
    ("Folio Page Document", "folio-page", "Normal-A4.docx", "Document"),
    ("Folio Show Presentation", "folio-show", "Blank.pptx", "Presentation"),
]


def _template_path(pkg, leaf):
    """The blank template of another Folio app, wherever it is installed."""
    here = os.path.dirname(__file__)
    for p in (os.path.join("/usr/share", pkg, "templates", leaf),
              os.path.join("/usr/local/share", pkg, "templates", leaf),
              os.path.join(here, "..", "..", "..", pkg.replace("folio-page", "folio-writer")
                           .replace("folio-show", "folio-deliver").replace("folio-sheet", "folio-sheets"),
                           "share", "templates", leaf)):
        if os.path.exists(p):
            return os.path.abspath(p)
    return None


def new_object_kinds():
    """The object types this computer can actually make, newest-looking first."""
    out = []
    for label, pkg, leaf, stem in NEW_OBJECT_KINDS:
        t = _template_path(pkg, leaf)
        if t:
            out.append((label, t, stem, os.path.splitext(leaf)[1]))
    return out


def _new_object_body(template, stem, ext, n=1):
    """A blank document of that kind, and the name it goes in under."""
    with open(template, "rb") as fh:
        return "%s%d%s" % (stem, n, ext), fh.read()


def insert_object(win, path=None, link=False):
    """Insert > Object > Create from File (embedded or linked); Create New needs programs this
    desktop doesn't have, so it says so."""
    if path is None:
        d = Gtk.Dialog(title="Object", transient_for=win, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        box = d.get_content_area()
        box.set_border_width(12)
        nb = Gtk.Notebook()
        kinds = new_object_kinds()
        nbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        nbox.set_border_width(8)
        if kinds:
            nbox.pack_start(Gtk.Label(label="Object type:", xalign=0), False, False, 0)
            store = Gtk.ListBox()
            store.set_selection_mode(Gtk.SelectionMode.SINGLE)
            for label, _t, _stem, _ext in kinds:
                row = Gtk.ListBoxRow()
                lab = Gtk.Label(label=label, xalign=0)
                lab.set_margin_start(6)
                lab.set_margin_top(3)
                lab.set_margin_bottom(3)
                row.add(lab)
                store.add(row)
            sw = Gtk.ScrolledWindow()
            sw.set_min_content_height(110)
            sw.add(store)
            nbox.pack_start(sw, True, True, 0)
            nicon = Gtk.CheckButton(label="Display as icon")
            nicon.set_active(True)
            nicon.set_sensitive(False)
            nbox.pack_start(nicon, False, False, 0)
            store.select_row(store.get_row_at_index(0))
            d.kinds, d.kindlist = kinds, store
        else:
            nbox.pack_start(Gtk.Label(label="No other Folio app is installed here, so there is\n"
                                            "nothing to make a new object with. Use Create from File.",
                                      xalign=0), False, False, 0)
            d.kinds, d.kindlist = [], None
        nb.append_page(nbox, Gtk.Label(label="Create New"))
        fbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        fbox.set_border_width(8)
        fbox.pack_start(Gtk.Label(label="File name:", xalign=0), False, False, 0)
        fc = Gtk.FileChooserButton(title="Browse", action=Gtk.FileChooserAction.OPEN)
        fbox.pack_start(fc, False, False, 0)
        lk = Gtk.CheckButton(label="Link to file")
        fbox.pack_start(lk, False, False, 0)
        icon = Gtk.CheckButton(label="Display as icon")
        icon.set_active(True)
        icon.set_sensitive(False)
        fbox.pack_start(icon, False, False, 0)
        nb.append_page(fbox, Gtk.Label(label="Create from File"))
        box.pack_start(nb, True, True, 0)
        d.chooser, d.link = fc, lk
        d.show_all()
        nb.set_current_page(1 if not d.kinds else 0)
        r = d.run()
        page = nb.get_current_page()
        path, link = fc.get_filename(), lk.get_active()
        made = None
        if r == Gtk.ResponseType.OK and page == 0 and d.kinds and d.kindlist is not None:
            row = d.kindlist.get_selected_row()
            if row is not None:
                label, template, stem, ext = d.kinds[row.get_index()]
                n = 1 + sum(1 for j in range(len(win.book.objects(win.grid.sheet)))
                            if (object_file(win, j) or ("", None, None))[0].startswith(stem))
                made = _new_object_body(template, stem, ext, n)
        d.destroy()
        if r != Gtk.ResponseType.OK:
            return None
        if made is not None:
            name, body = made
            data = _icon_png(name, name.encode("utf-8") + b"\x00" + body, b"flEm")
            return _place_object(win, data, "New " + name)
        if not path:
            return None
    name = os.path.basename(path)
    try:
        if link:
            data = _icon_png(name, os.path.abspath(path).encode("utf-8"), b"flLk")
        else:
            with open(path, "rb") as fh:
                body = fh.read()
            data = _icon_png(name, name.encode("utf-8") + b"\x00" + body, b"flEm")
    except OSError as e:
        win.dialogs.message(str(e), "Object")
        return None
    return _place_object(win, data, ("Linked file " if link else "Embedded file ") + name)


def _place_object(win, data, descr="Embedded file"):
    """Put a made object on the sheet at the cursor, as Create from File does."""
    b, g = win.book, win.grid
    r, c = g.sel.active
    i = b.insert_picture(g.sheet, data, b.col_x(g.sheet, c), b.row_y(g.sheet, r), 96.0, 96.0)
    b.set_object_props(g.sheet, [i], {"name": "Object %d" % (i + 1), "descr": descr}, "Insert Object")
    g.sel_objects = [i]
    g.refresh()
    win.update_title()
    return i


def object_file(win, i):
    """(file name, bytes or None, linked path or None) of an inserted object."""
    data = _pic(win, i)
    em = png_chunk(data, b"flEm")
    if em is not None:
        name, _, body = em.partition(b"\x00")
        return name.decode("utf-8", "replace"), body, None
    lk = png_chunk(data, b"flLk")
    if lk is not None:
        p = lk.decode("utf-8", "replace")
        return os.path.basename(p), None, p
    return None


def open_object(win, i, run=True):
    f = object_file(win, i)
    if f is None:
        return None
    name, body, linked = f
    path = linked
    if body is not None:
        path = os.path.join(tempfile.mkdtemp(prefix="folio-object-"), os.path.basename(name) or "object")
        with open(path, "wb") as fh:
            fh.write(body)
    if run:
        if linked and not os.path.exists(linked):
            win.dialogs.message("The linked file isn't there: %s" % linked, "Object")
            return None
        try:
            subprocess.Popen(["xdg-open", path], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except OSError:
            win.dialogs.message("There is no program here to open %s." % name, "Object")
    return path


def activate(win, i):
    """A double-click on an object made here: edit the SmartArt, sign the line, open the file."""
    o = win.book.objects(win.grid.sheet)[i]
    if o["kind"] == "chartex":
        from . import chartex_ui
        return chartex_ui.change_type(win, i)
    if o["kind"] == "group" and smartart_of(win, i) is not None:
        return edit_smartart(win, i)
    if o["kind"] == "picture":
        data = _pic(win, i)
        from . import corkembed
        if corkembed.is_diagram(data):
            from . import diagram_ui
            diagram_ui.edit(win, i)      # says so when Cork isn't installed
            return True
        if png_chunk(data, b"flSg") is not None:
            return sign(win, i)
        if png_chunk(data, b"flEm") is not None or png_chunk(data, b"flLk") is not None:
            return open_object(win, i) is not None
    return False


# ------------------------------------------------------------------ S3-16 signing with a certificate
# Excel signs with a certificate from the Windows certificate store. There is no such store here,
# so the signer points at their own PKCS#12 file (.p12/.pfx) — the form a certificate and its
# private key are handed out in — and types its password. Nothing leaves this computer: the
# signing, and the checking afterwards, are done here.
SIG_CHUNK = b"flSc"


def _sheet_digest(win, sheet, skip=None):
    """A digest of what is on the sheet, so a signature can be checked against it later.
    The signature pictures themselves are left out: signing changes them."""
    import hashlib
    b = win.book
    h = hashlib.sha256()
    rows, cols = b.used_extent(sheet) or (0, 0)
    for r in range(min(rows + 1, 4096)):
        for c in range(min(cols + 1, 256)):
            v = b.display(sheet, r, c)
            if v:
                h.update(("%d:%d:%s\n" % (r, c, v)).encode("utf-8"))
    for k, o in enumerate(b.objects(sheet)):
        if skip is not None and k == skip:
            continue
        h.update(("obj%d:%s\n" % (k, o.get("kind", ""))).encode("utf-8"))
    return h.hexdigest()


def certificate_of(p12_path, password):
    """(private key, certificate) out of a PKCS#12 file, or an error message."""
    try:
        from cryptography.hazmat.primitives.serialization import pkcs12
    except ImportError:
        return None, "This computer has no python3-cryptography, so certificates can't be used."
    try:
        with open(p12_path, "rb") as fh:
            blob = fh.read()
        key, cert, _chain = pkcs12.load_key_and_certificates(blob, (password or "").encode("utf-8"))
    except Exception as e:                                  # a wrong password lands here too
        return None, "That certificate file couldn't be opened: %s" % e
    if key is None or cert is None:
        return None, "That file has no certificate and private key in it."
    return (key, cert), None


def _name_of(x509name, default=""):
    from cryptography.x509.oid import NameOID
    for oid in (NameOID.COMMON_NAME, NameOID.ORGANIZATION_NAME):
        got = x509name.get_attributes_for_oid(oid)
        if got:
            return got[0].value
    return default


def sign_with_certificate(win, i, p12_path, password):
    """Sign a signature line with a certificate. Returns (True, signer) or (False, why)."""
    import base64
    import datetime
    got, err = certificate_of(p12_path, password)
    if err:
        return False, err
    key, cert = got
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa

    b, g = win.book, win.grid
    data = _pic(win, i)
    meta = png_chunk(data, b"flSg")
    if meta is None:
        return False, "That isn't a signature line."
    signer, title, email, _s, _d = (meta.decode("utf-8", "replace").split("\n") + [""] * 5)[:5]
    who = _name_of(cert.subject, signer or "the certificate holder")
    when = datetime.date.today().strftime("%m/%d/%Y")
    digest = _sheet_digest(win, g.sheet, skip=i)
    payload = "\n".join((signer, title, email, who, when, digest)).encode("utf-8")
    try:
        if isinstance(key, rsa.RSAPrivateKey):
            sig = key.sign(payload, padding.PKCS1v15(), hashes.SHA256())
            algo = "RSA-SHA256"
        elif isinstance(key, ec.EllipticCurvePrivateKey):
            sig = key.sign(payload, ec.ECDSA(hashes.SHA256()))
            algo = "ECDSA-SHA256"
        else:
            return False, "That certificate's key is of a kind Folio Sheet can't sign with."
    except Exception as e:
        return False, "The certificate couldn't sign this: %s" % e
    from cryptography.hazmat.primitives.serialization import Encoding
    record = "\n".join((algo, who, _name_of(cert.issuer, "unknown"),
                        cert.not_valid_before.strftime("%Y-%m-%d"), cert.not_valid_after.strftime("%Y-%m-%d"),
                        digest, base64.b64encode(sig).decode(),
                        base64.b64encode(cert.public_bytes(Encoding.DER)).decode()))
    x, y, w, h = b.objects(g.sheet)[i]["rect"]
    new = signature_png(signer, title, email, who, when)
    new = _png_add_chunk(new, SIG_CHUNK, record.encode("utf-8"))
    b.begin_group("Sign")
    try:
        b.delete_objects(g.sheet, [i])
        k = b.insert_picture(g.sheet, new, x, y, w, h)
        b.set_object_rect(g.sheet, k, x, y, w, h, "Sign")
        b.set_object_props(g.sheet, [k], {"name": "Signature Line %d" % (k + 1),
                                          "descr": "Signed by %s with a certificate" % who}, "Sign")
    finally:
        b.end_group()
    g.sel_objects = [k]
    g.refresh()
    win.update_title()
    return True, who


def check_signature(win, i):
    """What a certificate signature says, and whether it still holds.
    Returns a dict, or None when this object carries no certificate signature."""
    import base64
    import datetime
    data = _pic(win, i)
    rec = png_chunk(data, SIG_CHUNK)
    if rec is None:
        return None
    f = (rec.decode("utf-8", "replace").split("\n") + [""] * 8)[:8]
    algo, who, issuer, start, end, digest, sig_b64, cert_b64 = f
    out = {"algorithm": algo, "signer": who, "issuer": issuer, "from": start, "to": end,
           "valid": False, "unchanged": False, "expired": False, "why": ""}
    meta = png_chunk(data, b"flSg")
    signer, title, email, signed_by, when = (meta.decode("utf-8", "replace").split("\n") + [""] * 5)[:5] \
        if meta else ("", "", "", who, "")
    try:
        from cryptography import x509
        from cryptography.hazmat.primitives import hashes
        from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa
    except ImportError:
        out["why"] = "This computer has no python3-cryptography, so the signature can't be checked."
        return out
    try:
        cert = x509.load_der_x509_certificate(base64.b64decode(cert_b64))
        payload = "\n".join((signer, title, email, who, when, digest)).encode("utf-8")
        pub = cert.public_key()
        sig = base64.b64decode(sig_b64)
        if isinstance(pub, rsa.RSAPublicKey):
            pub.verify(sig, payload, padding.PKCS1v15(), hashes.SHA256())
        elif isinstance(pub, ec.EllipticCurvePublicKey):
            pub.verify(sig, payload, ec.ECDSA(hashes.SHA256()))
        else:
            out["why"] = "The certificate's key is of a kind Folio Sheet can't check."
            return out
        out["valid"] = True
    except Exception as e:
        out["why"] = "The signature doesn't match: %s" % e
        return out
    out["unchanged"] = (_sheet_digest(win, win.grid.sheet, skip=i) == digest)
    today = datetime.date.today().strftime("%Y-%m-%d")
    out["expired"] = bool(end) and end < today
    if not out["unchanged"]:
        out["why"] = "The sheet has been changed since it was signed."
    elif out["expired"]:
        out["why"] = "The certificate had expired on %s." % end
    return out


def sign_dialog(win, suggested=""):
    """Ask how to sign: a typed name, or a certificate file. Returns
    (kind, name, p12 path, password) or None."""
    d = Gtk.Dialog(title="Sign", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("Sign", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    grid = Gtk.Grid(column_spacing=8, row_spacing=6)
    typed = Gtk.RadioButton.new_with_label_from_widget(None, "Type your name:")
    name = Gtk.Entry()
    name.set_text(suggested)
    name.set_hexpand(True)
    grid.attach(typed, 0, 0, 1, 1)
    grid.attach(name, 1, 0, 2, 1)
    cert = Gtk.RadioButton.new_with_label_from_widget(typed, "Sign with a certificate:")
    fc = Gtk.FileChooserButton(title="Certificate (.p12 or .pfx)", action=Gtk.FileChooserAction.OPEN)
    pw = Gtk.Entry()
    pw.set_visibility(False)
    pw.set_placeholder_text("Password")
    grid.attach(cert, 0, 1, 1, 1)
    grid.attach(fc, 1, 1, 1, 1)
    grid.attach(pw, 2, 1, 1, 1)
    box.pack_start(grid, False, False, 0)
    note = Gtk.Label(xalign=0, label="The certificate and its password stay on this computer.")
    note.set_margin_top(8)
    box.pack_start(note, False, False, 0)
    d.typed, d.name, d.cert, d.fc, d.pw = typed, name, cert, fc, pw
    d.show_all()
    r = d.run()
    out = None
    if r == Gtk.ResponseType.OK:
        if cert.get_active() and fc.get_filename():
            out = ("certificate", "", fc.get_filename(), pw.get_text())
        elif name.get_text().strip():
            out = ("typed", name.get_text().strip(), None, None)
    d.destroy()
    return out


def show_signature(win, i):
    """What a certificate signature on this line says."""
    r = check_signature(win, i)
    if r is None:
        return False
    if r["valid"] and r["unchanged"] and not r["expired"]:
        head = "This signature is good."
    elif r["valid"]:
        head = "This signature matches, but:"
    else:
        head = "This signature does not match."
    lines = [head, "",
             "Signed by: %s" % r["signer"],
             "Certificate issued by: %s" % r["issuer"],
             "Certificate good from %s to %s" % (r["from"], r["to"]),
             "Signed using: %s" % r["algorithm"]]
    if r["why"]:
        lines += ["", r["why"]]
    lines += ["", "Folio Sheet checks that the signature matches the certificate and that the",
              "sheet has not changed since. It does not say whether the certificate itself",
              "comes from someone you should trust."]
    win.dialogs.message("\n".join(lines), "Signature")
    return True
