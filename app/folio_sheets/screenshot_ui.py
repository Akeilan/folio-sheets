"""Insert › Screenshot (SH-21): a picture of another open window (Available Windows) or of a part
of the screen (Screen Clipping), put on the sheet as a picture.

On X11 the windows are listed from the window manager and Screen Clipping hides the workbook,
greys the screen and lets you drag out the part you want, as in the desktop spreadsheet. Where the
desktop doesn't allow pictures of the screen to be taken directly (Wayland), Screen Clipping asks
the desktop's own screenshot tool, and Available Windows is empty."""
import ctypes
import ctypes.util
import os

import cairo
from gi.repository import Gdk, GdkPixbuf, Gio, GLib, Gtk

from . import APP_NAME

THUMB_W = 160


def is_x11():
    d = Gdk.Display.get_default()
    return d is not None and type(d).__name__ == "X11Display"


# ---------------------------------------------------------------- the windows that are open (X11)
class _Xlib:
    def __init__(self):
        path = ctypes.util.find_library("X11")
        if not path:
            raise OSError("no libX11")
        x = ctypes.CDLL(path)
        x.XOpenDisplay.restype = ctypes.c_void_p
        x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x.XDefaultRootWindow.restype = ctypes.c_ulong
        x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
        x.XInternAtom.restype = ctypes.c_ulong
        x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        x.XGetWindowProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_long, ctypes.c_long,
                                         ctypes.c_int, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong),
                                         ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_ulong),
                                         ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_void_p)]
        x.XFree.argtypes = [ctypes.c_void_p]
        x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self.x = x
        self.dpy = x.XOpenDisplay(None)
        if not self.dpy:
            raise OSError("no display")

    def prop(self, win, name, kind):
        x = self.x
        atom = x.XInternAtom(self.dpy, name.encode(), 0)
        t = ctypes.c_ulong()
        fmt = ctypes.c_int()
        n = ctypes.c_ulong()
        after = ctypes.c_ulong()
        data = ctypes.c_void_p()
        req = x.XInternAtom(self.dpy, kind.encode(), 0) if kind else 0
        if x.XGetWindowProperty(self.dpy, win, atom, 0, 1 << 16, 0, req, ctypes.byref(t), ctypes.byref(fmt),
                                ctypes.byref(n), ctypes.byref(after), ctypes.byref(data)) != 0 or not data.value:
            return None
        try:
            if fmt.value == 32:
                arr = ctypes.cast(data, ctypes.POINTER(ctypes.c_ulong))
                return [arr[i] for i in range(n.value)]
            if fmt.value == 8:
                return ctypes.string_at(data, n.value)
            return None
        finally:
            x.XFree(data)

    def close(self):
        self.x.XCloseDisplay(self.dpy)


def available_windows(exclude=()):
    """[(xid, title)] of the open windows (not minimised), the workbook's own left out."""
    if not is_x11():
        return []
    try:
        xl = _Xlib()
    except OSError:
        return []
    try:
        root = xl.x.XDefaultRootWindow(xl.dpy)
        ids = xl.prop(root, "_NET_CLIENT_LIST_STACKING", "WINDOW") or xl.prop(root, "_NET_CLIENT_LIST", "WINDOW") or []
        hidden = xl.x.XInternAtom(xl.dpy, b"_NET_WM_STATE_HIDDEN", 0)
        out = []
        for w in reversed(ids):
            if w in exclude:
                continue
            st = xl.prop(w, "_NET_WM_STATE", "ATOM") or []
            if hidden in st:
                continue
            name = xl.prop(w, "_NET_WM_NAME", "UTF8_STRING") or xl.prop(w, "WM_NAME", None) or b""
            out.append((w, name.decode("utf-8", "replace") if isinstance(name, bytes) else ""))
        return out
    finally:
        xl.close()


def _grab(gdk_window, x=0, y=0, w=None, h=None):
    """A picture of a window (or of part of it) as a pixbuf."""
    w = w or gdk_window.get_width()
    h = h or gdk_window.get_height()
    img = cairo.ImageSurface(cairo.FORMAT_RGB24, max(1, int(w)), max(1, int(h)))
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, gdk_window, -x, -y)
    cr.paint()
    img.flush()
    return Gdk.pixbuf_get_from_surface(img, 0, 0, img.get_width(), img.get_height())


def window_picture(xid):
    from gi.repository import GdkX11
    disp = Gdk.Display.get_default()
    try:
        gw = GdkX11.X11Window.foreign_new_for_display(disp, xid)
    except Exception:
        return None
    if gw is None:
        return None
    return _grab(gw)


def own_xids(win):
    out = set()
    for w in Gtk.Window.list_toplevels():
        gw = w.get_window()
        if gw is not None and hasattr(gw, "get_xid"):
            out.add(gw.get_xid())
            tl = gw.get_toplevel()
            if hasattr(tl, "get_xid"):
                out.add(tl.get_xid())
    return out


def _png(pb):
    ok, buf = pb.save_to_bufferv("png", [], [])
    return bytes(buf) if ok else None


def place(win, pixbuf):
    from .clip_ui import _place_picture
    data = _png(pixbuf) if pixbuf is not None else None
    if not data:
        return False
    return _place_picture(win, data)


# ---------------------------------------------------------------- the menu
def menu(win):
    """The Screenshot drop-down: Available Windows (thumbnails) and Screen Clipping."""
    pop = Gtk.Popover()
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    box.set_border_width(8)
    box.pack_start(Gtk.Label(label="Available Windows", xalign=0), False, False, 0)
    wins = available_windows(own_xids(win))
    flow = Gtk.FlowBox()
    flow.set_name("available-windows")
    flow.set_max_children_per_line(3)
    flow.set_selection_mode(Gtk.SelectionMode.NONE)
    shown = 0
    for xid, title in wins[:12]:
        pb = window_picture(xid)
        if pb is None or pb.get_width() < 8:
            continue
        sc = THUMB_W / pb.get_width()
        th = pb.scale_simple(THUMB_W, max(1, int(pb.get_height() * sc)), GdkPixbuf.InterpType.BILINEAR)
        b = Gtk.Button()
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.add(Gtk.Image.new_from_pixbuf(th))
        b.set_tooltip_text(title)
        b.connect("clicked", lambda _b, pb=pb: (pop.popdown(), place(win, pb)))
        flow.add(b)
        shown += 1
    if shown:
        box.pack_start(flow, False, False, 0)
    else:
        lab = Gtk.Label(label="No Available Windows", xalign=0)
        lab.get_style_context().add_class("dim-label")
        box.pack_start(lab, False, False, 0)
    box.pack_start(Gtk.Separator(), False, False, 2)
    clip = Gtk.ModelButton(text="Screen Clipping")
    clip.connect("clicked", lambda *_: (pop.popdown(), GLib.idle_add(lambda: (screen_clipping(win), False)[1])))
    box.pack_start(clip, False, False, 0)
    pop.add(box)
    box.show_all()
    pop.windows_shown = shown
    return pop


# ---------------------------------------------------------------- Screen Clipping
class ClipWindow(Gtk.Window):
    """Covers the screen with a greyed picture of it; drag to choose the part to insert."""
    def __init__(self, shot, done):
        super().__init__(type=Gtk.WindowType.POPUP)
        self.shot = shot
        self.done = done
        self.start = self.end = None
        self.move(0, 0)
        self.set_default_size(shot.get_width(), shot.get_height())
        self.set_size_request(shot.get_width(), shot.get_height())
        area = Gtk.DrawingArea()
        area.set_can_focus(True)
        area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK | Gdk.EventMask.BUTTON_RELEASE_MASK |
                        Gdk.EventMask.POINTER_MOTION_MASK | Gdk.EventMask.KEY_PRESS_MASK)
        area.connect("draw", self.on_draw)
        area.connect("button-press-event", self.on_press)
        area.connect("motion-notify-event", self.on_motion)
        area.connect("button-release-event", self.on_release)
        area.connect("key-press-event", self.on_key)
        self.area = area
        self.add(area)
        self.connect("realize", lambda *_: self.get_window().set_cursor(
            Gdk.Cursor.new_from_name(self.get_display(), "crosshair")))

    def rect(self):
        if not self.start or not self.end:
            return None
        x1, y1 = self.start
        x2, y2 = self.end
        return int(min(x1, x2)), int(min(y1, y2)), int(abs(x2 - x1)), int(abs(y2 - y1))

    def on_draw(self, w, cr):
        Gdk.cairo_set_source_pixbuf(cr, self.shot, 0, 0)
        cr.paint()
        cr.set_source_rgba(1, 1, 1, 0.55)
        cr.paint()
        r = self.rect()
        if r and r[2] > 0 and r[3] > 0:
            cr.save()
            cr.rectangle(*r)
            cr.clip()
            Gdk.cairo_set_source_pixbuf(cr, self.shot, 0, 0)
            cr.paint()
            cr.restore()
            cr.set_source_rgb(0.85, 0.1, 0.1)
            cr.set_line_width(1)
            cr.rectangle(r[0] + 0.5, r[1] + 0.5, r[2], r[3])
            cr.stroke()

    def on_press(self, w, ev):
        self.start = self.end = (ev.x, ev.y)
        w.queue_draw()
        return True

    def on_motion(self, w, ev):
        if self.start is not None:
            self.end = (ev.x, ev.y)
            w.queue_draw()
        return True

    def on_release(self, w, ev):
        if self.start is None:
            return True
        self.end = (ev.x, ev.y)
        self.finish(self.rect())
        return True

    def on_key(self, w, ev):
        if ev.keyval == Gdk.KEY_Escape:
            self.finish(None)
            return True
        return False

    def finish(self, r):
        pb = None
        if r and r[2] >= 2 and r[3] >= 2:
            x, y, w, h = r
            w = min(w, self.shot.get_width() - x)
            h = min(h, self.shot.get_height() - y)
            if w > 0 and h > 0:
                pb = self.shot.new_subpixbuf(x, y, w, h).copy()
        self.destroy()
        self.done(pb)


def screen_clipping(win):
    if is_x11():
        return _clip_x11(win)
    return _clip_portal(win)


def _clip_x11(win):
    """Hide the workbook, take the screen, let the user drag out a part."""
    win.hide()

    def shoot():
        root = Gdk.get_default_root_window()
        shot = _grab(root)
        cw = ClipWindow(shot, lambda pb: _clipped(win, pb))
        win._clip_window = cw
        cw.show_all()
        cw.present()
        cw.area.grab_focus()
        seat = Gdk.Display.get_default().get_default_seat()
        if seat is not None and cw.get_window() is not None:
            seat.grab(cw.get_window(), Gdk.SeatCapabilities.ALL, True, None, None, None)
        return False
    GLib.timeout_add(350, shoot)
    return True


def _clipped(win, pb):
    seat = Gdk.Display.get_default().get_default_seat()
    if seat is not None:
        seat.ungrab()
    win._clip_window = None
    win.show()
    win.present()
    if pb is not None:
        place(win, pb)


def _clip_portal(win):
    """Ask the desktop's screenshot tool (it lets the user choose the area)."""
    try:
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    except GLib.Error:
        bus = None
    if bus is None:
        return _unavailable(win)
    token = "folio%d" % os.getpid()
    sender = bus.get_unique_name()[1:].replace(".", "_")
    handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (sender, token)

    def on_response(conn, snd, path, iface, sig, params, *_):
        bus.signal_unsubscribe(sub[0])
        code, results = params.unpack()
        if code != 0:
            return
        uri = results.get("uri")
        if not uri:
            return
        path_ = Gio.File.new_for_uri(uri).get_path()
        try:
            pb = GdkPixbuf.Pixbuf.new_from_file(path_)
        except GLib.Error:
            return
        place(win, pb)
    sub = [bus.signal_subscribe("org.freedesktop.portal.Desktop", "org.freedesktop.portal.Request", "Response",
                                handle, None, Gio.DBusSignalFlags.NO_MATCH_RULE, on_response)]
    opts = {"handle_token": GLib.Variant("s", token), "interactive": GLib.Variant("b", True),
            "modal": GLib.Variant("b", True)}
    try:
        bus.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                      "org.freedesktop.portal.Screenshot", "Screenshot",
                      GLib.Variant("(sa{sv})", ("", opts)), GLib.VariantType("(o)"),
                      Gio.DBusCallFlags.NONE, 5000, None)
    except GLib.Error:
        bus.signal_unsubscribe(sub[0])
        return _unavailable(win)
    return True


def _unavailable(win):
    win.dialogs.info("Screen Clipping isn't available here: the desktop doesn't allow %s to take a picture of "
                     "the screen. Take a screenshot with the desktop's own tool and paste it." % APP_NAME, "Screenshot")
    return False


