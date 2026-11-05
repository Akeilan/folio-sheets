"""Cork diagrams inside Page, Sheet and Show (K3-08), one copy for the three apps.

A diagram is an ordinary PNG picture that also carries the whole Cork drawing in a private
chunk ("foLK"). The apps keep picture bytes as they are, so the drawing travels inside the
document (and Office shows the picture). Double-clicking such a picture opens it in Folio Cork;
each save there writes the PNG back and the app swaps the new picture in.
Pasting: Cork's Copy puts a plain picture on the clipboard and leaves the same picture, with the
drawing, as clipboard.png in its settings folder; when the two match, the app pastes that file.
"""
import os
import shutil
import struct
import subprocess
import tempfile
import zlib

CHUNK = b"foLK"
_SIG = b"\x89PNG\r\n\x1a\n"


def is_diagram(data):
    """True when these picture bytes carry a Cork drawing."""
    if not data or data[:8] != _SIG:
        return False
    i = 8
    while i + 12 <= len(data):
        n = struct.unpack(">I", data[i:i + 4])[0]
        name = data[i + 4:i + 8]
        end = i + 12 + n
        if end > len(data):
            return False
        if name == CHUNK:
            crc = struct.unpack(">I", data[end - 4:end])[0]
            return zlib.crc32(data[i + 4:end - 4]) & 0xFFFFFFFF == crc
        if name == b"IEND":
            return False
        i = end
    return False


def png_size(data):
    if data and data[:8] == _SIG and data[12:16] == b"IHDR":
        return struct.unpack(">II", data[16:24])
    return None


def cork_config_dir():
    d = os.environ.get("FOLIO_CONFIG_DIR")
    if d:
        return d
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    return os.path.join(base, "folio-cork")


def _pixels(pb):
    """(width, height, rowstride, channels, bytes) of a GdkPixbuf."""
    return pb.get_width(), pb.get_height(), pb.get_rowstride(), pb.get_n_channels(), pb.get_pixels()


def clipboard_diagram(pixbuf):
    """The diagram PNG behind the picture on the clipboard, when Cork put it there; else None."""
    path = os.path.join(cork_config_dir(), "clipboard.png")
    try:
        with open(path, "rb") as fh:
            data = fh.read()
    except OSError:
        return None
    if pixbuf is None or not is_diagram(data) or png_size(data) != (pixbuf.get_width(), pixbuf.get_height()):
        return None
    try:
        from gi.repository import GdkPixbuf
        loader = GdkPixbuf.PixbufLoader.new_with_type("png")
        loader.write(data)
        loader.close()
        mine = loader.get_pixbuf()
    except Exception:
        return None
    w, h, rs_a, ch_a, pa = _pixels(mine)
    _w, _h, rs_b, ch_b, pb = _pixels(pixbuf)
    # sample a grid; colour of see-through pixels doesn't count
    diff = n = 0
    for y in range(0, h, max(1, h // 24)):
        for x in range(0, w, max(1, w // 24)):
            a = pa[y * rs_a + x * ch_a:y * rs_a + x * ch_a + ch_a]
            b = pb[y * rs_b + x * ch_b:y * rs_b + x * ch_b + ch_b]
            al = a[3] if ch_a == 4 else 255
            bl = b[3] if ch_b == 4 else 255
            if al < 8 and bl < 8:
                continue
            diff += sum(abs(a[k] - b[k]) for k in range(3)) + abs(al - bl)
            n += 1
    if n and diff / (n * 4.0) > 12:
        return None
    return data


def cork_command():
    """How to start Folio Cork, or None when it isn't installed."""
    exe = os.environ.get("FOLIO_CORK") or shutil.which("folio-cork")
    if not exe:
        for p in ("/usr/bin/folio-cork", "/usr/local/bin/folio-cork"):
            if os.path.exists(p):
                exe = p
    return [exe] if exe else None


NOT_INSTALLED = "Folio Cork isn't installed, so the diagram can't be edited. It still shows as a picture."


class Editing:
    """One diagram open in Cork. `on_change(png_bytes)` runs on the main loop after each save
    there; `on_close()` once Cork has closed."""

    def __init__(self, data, on_change, on_close=None, name="Diagram"):
        from gi.repository import GLib
        self.GLib = GLib
        self.dir = tempfile.mkdtemp(prefix="folio-diagram-")
        self.path = os.path.join(self.dir, "%s.png" % name)
        with open(self.path, "wb") as fh:
            fh.write(data)
        self.stamp = os.stat(self.path).st_mtime_ns
        self.on_change, self.on_close = on_change, on_close
        self.proc = subprocess.Popen(cork_command() + [self.path], stdin=subprocess.DEVNULL)
        self.timer = GLib.timeout_add(400, self._poll)

    def _check(self):
        try:
            st = os.stat(self.path).st_mtime_ns
        except OSError:
            return
        if st != self.stamp:
            self.stamp = st
            try:
                with open(self.path, "rb") as fh:
                    data = fh.read()
            except OSError:
                return
            if is_diagram(data):
                try:
                    self.on_change(data)
                except Exception:
                    import traceback
                    traceback.print_exc()   # one bad update never stops the watch

    def _poll(self):
        self._check()
        if self.proc.poll() is None:
            return True
        self.timer = None
        shutil.rmtree(self.dir, ignore_errors=True)
        if self.on_close:
            self.on_close()
        return False

    def running(self):
        return self.proc.poll() is None

    def stop(self):
        if self.proc.poll() is None:
            self.proc.terminate()


def edit(data, on_change, message, on_close=None, name="Diagram"):
    """Open a diagram picture in Cork. `message(text)` reports problems. Returns Editing or None."""
    if not is_diagram(data):
        return None
    if cork_command() is None:
        message(NOT_INSTALLED)
        return None
    try:
        return Editing(data, on_change, on_close, name)
    except OSError as e:
        message("Folio Cork couldn't be started: %s" % e)
        return None
