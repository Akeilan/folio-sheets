"""Folio Sheets application: windows, recent files, settings."""
import json
import os
import subprocess
import sys

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

from . import APP_ID, APP_NAME, SHARE, VERSION  # noqa: E402

# The desktop shell matches windows to their launcher (and so to the app
# icon) by WM_CLASS / application id: both are the app id, as is the name
# of the .desktop file.
GLib.set_prgname(APP_ID)
GLib.set_application_name(APP_NAME)
Gdk.set_program_class(APP_ID)

CONFIG = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"),
                      "folio-sheets")


class SheetsApp(Gtk.Application):
    def __init__(self):
        Gtk.Application.__init__(self, application_id=APP_ID,
                                 flags=Gio.ApplicationFlags.HANDLES_OPEN |
                                 Gio.ApplicationFlags.NON_UNIQUE)
        self.windows = []
        self.settings = self._load_settings()
        self._untitled = 0

    # ------------------------------------------------------------ settings
    def _load_settings(self):
        try:
            with open(os.path.join(CONFIG, "settings.json")) as f:
                return json.load(f)
        except (OSError, ValueError):
            return {"recent": [], "author": os.environ.get("USER", "")}

    def save_settings(self):
        os.makedirs(CONFIG, exist_ok=True)
        tmp = os.path.join(CONFIG, "settings.json.tmp")
        with open(tmp, "w") as f:
            json.dump(self.settings, f, indent=1)
        os.replace(tmp, os.path.join(CONFIG, "settings.json"))

    def add_recent(self, path):
        path = os.path.abspath(path)
        rec = [p for p in self.settings.get("recent", []) if p != path]
        rec.insert(0, path)
        self.settings["recent"] = rec[:50]
        try:
            self.save_settings()
        except OSError:
            pass

    def recent(self):
        return [p for p in self.settings.get("recent", []) if os.path.exists(p)]

    def pinned(self):
        return [p for p in self.settings.get("pinned", []) if os.path.exists(p)]

    def toggle_pin(self, path):
        pins = self.settings.setdefault("pinned", [])
        if path in pins:
            pins.remove(path)
        else:
            pins.insert(0, path)
        self.save_settings()

    def remove_recent(self, path):
        self.settings["recent"] = [p for p in self.settings.get("recent", []) if p != path]
        self.save_settings()

    def user_name(self):
        return self.settings.get("author") or os.environ.get("USER", "")

    def next_untitled(self):
        self._untitled += 1
        return "Book%d" % self._untitled

    def blank_template(self):
        for p in (os.path.join(SHARE, "templates", "Blank.xlsx"),
                  os.path.join(os.path.dirname(__file__), "..", "..", "share", "templates",
                               "Blank.xlsx")):
            if os.path.exists(p):
                return os.path.abspath(p)
        return None

    # ------------------------------------------------------------ life cycle
    def do_startup(self):
        Gtk.Application.do_startup(self)
        css = Gtk.CssProvider()
        css.load_from_path(os.path.join(os.path.dirname(__file__), "theme.css"))
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), css,
                                                 Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        extra = os.path.join(os.path.dirname(__file__), "sheets.css")
        if os.path.exists(extra):
            css2 = Gtk.CssProvider()
            css2.load_from_path(extra)
            Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), css2,
                                                     Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
        # dark mode follows the desktop (S-08), switching while running
        from . import appearance
        self._dark_css = Gtk.CssProvider()
        self._dark_css.load_from_path(os.path.join(os.path.dirname(__file__), "theme-dark.css"))
        appearance.apply_initial()
        self._apply_dark(appearance.is_dark())
        appearance.on_change(self._apply_dark)

    def _apply_dark(self, dark):
        screen = Gdk.Screen.get_default()
        if screen is None:
            return
        if dark:
            Gtk.StyleContext.add_provider_for_screen(screen, self._dark_css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 2)
        else:
            Gtk.StyleContext.remove_provider_for_screen(screen, self._dark_css)
        from . import icons
        icons.restyle()
        for w in Gtk.Window.list_toplevels():
            for fn in ("refresh_ribbon_icons",):
                if hasattr(w, fn):
                    getattr(w, fn)()
            w.queue_draw()

    def do_activate(self):
        if self.windows:
            self.windows[-1].present()
            return
        win = self.new_window()
        self._maybe_script(win)

    def _maybe_script(self, win):
        script = os.environ.get("FOLIO_TEST_SCRIPT")
        if script:
            def run_script():
                ns = {"app": self, "win": win, "GLib": GLib, "Gtk": Gtk, "Gdk": Gdk}
                try:
                    with open(script) as f:
                        exec(compile(f.read(), script, "exec"), ns)
                except Exception:
                    import traceback
                    traceback.print_exc()
                    sys.stdout.flush()
                    os._exit(3)
                return False
            GLib.timeout_add(int(os.environ.get("FOLIO_TEST_DELAY", "2500")), run_script)

    def do_open(self, files, n_files, hint):
        win = None
        for f in files:
            path = f.get_path()
            if path:
                win = self.open_path(path)
        if win is not None:
            self._maybe_script(win)

    def new_window(self, path=None):
        from .window import SheetWindow
        win = SheetWindow(self, path)
        self.windows.append(win)
        win.present()
        return win

    def open_path(self, path, from_window=None):
        path = os.path.abspath(path)
        for w in self.windows:
            if w.path == path:
                w.present()
                return w
        # reuse an untouched blank window, as the original does
        if from_window is not None and from_window.path is None and not from_window.book.modified \
                and not from_window.book.can_undo:
            from_window.load(path)
            return from_window
        return self.new_window(path)

    def window_closed(self, win):
        try:
            self.save_settings()
        except OSError:
            pass
        if win in self.windows:
            self.windows.remove(win)
        if not self.windows:
            GLib.idle_add(self.quit)


MIME_DEFAULTS = [
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel.sheet.macroEnabled.12",
    "application/vnd.ms-excel",
    "application/vnd.oasis.opendocument.spreadsheet",
    "text/csv",
]


def make_default():
    for t in MIME_DEFAULTS:
        subprocess.run(["xdg-mime", "default", APP_ID + ".desktop", t],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main(argv=None):
    argv = list(argv if argv is not None else sys.argv)
    if "--version" in argv:
        print("%s %s" % (APP_NAME, VERSION))
        return 0
    if "--make-default" in argv:
        make_default()
        print("%s is now the default for spreadsheet files." % APP_NAME)
        return 0
    if "--reset-settings" in argv:
        p = os.path.join(CONFIG, "settings.json")
        if os.path.exists(p):
            os.remove(p)
        print("Settings reset.")
        return 0
    app = SheetsApp()
    code = app.run(argv)
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(code or 0)
