# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): open a file and save a screenshot of the window.
import os
import time
from gi.repository import Gdk, Gtk
import cairo


def pump(t=0.3):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.005)


path = os.environ.get("SHOT_FILE")
if path:
    win.open_path(path) if hasattr(win, "open_path") else win.load(path)
for cmd in os.environ.get("SHOT_CMDS", "").split(";"):
    if cmd:
        win.run(cmd)
pump(0.8)
gw = win.get_window()
img = cairo.ImageSurface(cairo.FORMAT_ARGB32, gw.get_width(), gw.get_height())
cr = cairo.Context(img)
Gdk.cairo_set_source_window(cr, gw, 0, 0)
cr.paint()
img.write_to_png(os.environ.get("SHOT_OUT", "/tmp/shot.png"))
print("SHOT OK")
os._exit(0)
