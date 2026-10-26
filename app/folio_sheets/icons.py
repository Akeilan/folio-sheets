"""Icons: Folio's own SVGs (folio_sheets/icons); LibreOffice's open-licensed
Colibre theme (MPL 2.0 / CC BY-SA 4.0), if installed, is only a fallback."""
import os
import zipfile

from gi.repository import GdkPixbuf, Gtk

ARCHIVES = [
    ("/usr/lib/libreoffice/share/config/images_colibre_svg.zip", "svg"),
    ("/usr/share/libreoffice/share/config/images_colibre_svg.zip", "svg"),
    ("/usr/lib/libreoffice/share/config/images_colibre.zip", "png"),
]
OWN = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icons")

_zip = None
_kind = None
_links = {}
_cache = {}


def _open():
    global _zip, _kind
    if _zip is not None:
        return
    for path, kind in ARCHIVES:
        if os.path.exists(path):
            try:
                _zip = zipfile.ZipFile(path)
                _kind = kind
                break
            except (OSError, zipfile.BadZipFile):
                continue
    if _zip is None:
        return
    try:
        for line in _zip.read("links.txt").decode().splitlines():
            parts = line.split()
            if len(parts) == 2:
                _links[parts[0]] = parts[1]
    except KeyError:
        pass


def _resolve(name):
    seen = 0
    while name in _links and seen < 8:
        name = _links[name]
        seen += 1
    return name


_LOG = os.environ.get("FOLIO_ICON_LOG")


def pixbuf(name, size):
    """name: a command ('bold', '.uno:Bold') or a path inside the theme
    ('cmd/lc_bold'); size 16 or 32 (scaled to fit)."""
    key = (name, size)
    if _LOG:
        with open(_LOG, "a") as f:
            f.write("%s %d\n" % (name, size))
    if key in _cache:
        return _cache[key]
    _open()
    pb = None
    base = name.replace(".uno:", "").lower()
    own = os.path.join(OWN, base + ".svg")
    candidates = []
    if os.path.exists(own):
        try:
            pb = GdkPixbuf.Pixbuf.new_from_file_at_size(own, size, size)
        except Exception:
            pb = None
    if pb is None and _zip is not None:
        prefix = "lc_" if size > 20 else "sc_"
        if "/" in base:
            candidates.append(base)
        else:
            candidates += ["cmd/%s%s" % (prefix, base),
                           "cmd/%s%s" % ("sc_" if prefix == "lc_" else "lc_", base)]
        ext = "." + _kind
        for c in candidates:
            path = _resolve(c + ext)
            if path.endswith(".png") and _kind == "svg":
                continue
            try:
                data = _zip.read(path)
            except KeyError:
                continue
            loader = GdkPixbuf.PixbufLoader()
            try:
                if _kind == "svg":
                    loader.set_size(size, size)
                loader.write(data)
                loader.close()
                pb = loader.get_pixbuf()
                if pb is not None and (pb.get_width() != size or pb.get_height() != size):
                    w, h = pb.get_width(), pb.get_height()
                    scale = size / max(w, h)
                    pb = pb.scale_simple(max(1, int(w * scale)), max(1, int(h * scale)),
                                         GdkPixbuf.InterpType.BILINEAR)
                break
            except Exception:
                try:
                    loader.close()
                except Exception:
                    pass
                pb = None
    _cache[key] = pb
    return pb


def exists(name):
    return pixbuf(name, 16) is not None


def swatch_pixbuf(name, size, color):
    """An own icon with its colour bar (the rect at the bottom) in `color` (0xRRGGBB)."""
    import re
    path = os.path.join(OWN, name + ".svg")
    try:
        with open(path) as f:
            svg = f.read()
    except OSError:
        return pixbuf(name, size)
    hexc = "#%06x" % (color & 0xFFFFFF)
    svg2, n = re.subn(r'(<rect x="1" y="13"[^>]*fill=")#[0-9a-fA-F]{6}', lambda m: m.group(1) + hexc, svg)
    if not n:
        return pixbuf(name, size)
    from gi.repository import GdkPixbuf
    loader = GdkPixbuf.PixbufLoader.new_with_type("svg")
    loader.set_size(size, size)
    loader.write(svg2.encode())
    loader.close()
    return loader.get_pixbuf()


def image(name, size):
    pb = pixbuf(name, size)
    if pb is None:
        img = Gtk.Image.new_from_icon_name("image-missing", Gtk.IconSize.MENU)
        img.set_pixel_size(size)
        return img
    return Gtk.Image.new_from_pixbuf(pb)


def white_image(name, size):
    """The icon as a white silhouette (for the coloured title bar)."""
    pb = pixbuf(name, size)
    if pb is None:
        return image(name, size)
    if not pb.get_has_alpha():
        pb = pb.add_alpha(False, 0, 0, 0)
    w, h, rs, n = pb.get_width(), pb.get_height(), pb.get_rowstride(), pb.get_n_channels()
    data = bytearray(pb.get_pixels())
    for y in range(h):
        for x in range(w):
            i = y * rs + x * n
            data[i] = data[i + 1] = data[i + 2] = 255
    out = GdkPixbuf.Pixbuf.new_from_data(bytes(data), GdkPixbuf.Colorspace.RGB, True, 8,
                                         w, h, rs)
    return Gtk.Image.new_from_pixbuf(out.copy())
