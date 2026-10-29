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
    """Sign a signature line: the signer's typed name goes on the line (no certificate)."""
    import datetime
    b, g = win.book, win.grid
    data = _pic(win, i)
    meta = png_chunk(data, b"flSg")
    if meta is None:
        return False
    signer, title, email, _signed, _date = (meta.decode("utf-8", "replace").split("\n") + [""] * 5)[:5]
    if name is None:
        name = win.dialogs.ask_text("Sign", "Type your name to sign:", signer)
        if not name:
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
        new = Gtk.Label(label="Creating a new object needs a program that makes embedded objects;\n"
                              "there are none on this desktop. Use Create from File.", xalign=0)
        new.set_margin_top(12)
        nb.append_page(new, Gtk.Label(label="Create New"))
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
        nb.set_current_page(1)
        r = d.run()
        path, link = fc.get_filename(), lk.get_active()
        d.destroy()
        if r != Gtk.ResponseType.OK or not path:
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
    b, g = win.book, win.grid
    r, c = g.sel.active
    i = b.insert_picture(g.sheet, data, b.col_x(g.sheet, c), b.row_y(g.sheet, r), 96.0, 96.0)
    b.set_object_props(g.sheet, [i], {"name": "Object %d" % (i + 1), "descr": ("Linked file " if link else "Embedded file ") + name},
                       "Insert Object")
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
        if png_chunk(data, b"flSg") is not None:
            return sign(win, i)
        if png_chunk(data, b"flEm") is not None or png_chunk(data, b"flLk") is not None:
            return open_object(win, i) is not None
    return False
