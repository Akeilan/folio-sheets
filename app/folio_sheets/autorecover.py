"""AutoRecover (S6-06): every few minutes a copy of each workbook with unsaved changes is
written to ~/.local/share/folio-sheet/AutoRecover. Saving or closing the workbook removes its
copy; copies left behind by a program that ended without closing them are offered at the next
start (Document Recovery). Workbooks with a password to open stay encrypted in their copy."""
import json
import os
import re
import time

from gi.repository import GLib, Gtk

# the original's default: every 10 minutes (File > Options > Save changes it)
DEFAULT_MINUTES = 10
INTERVAL = os.environ.get("FOLIO_AUTORECOVER_SECONDS")


def folder():
    base = os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")
    return os.path.join(base, "folio-sheet", "AutoRecover")


def _alive(pid):
    from . import folio_os
    return folio_os.process_alive(pid)


class AutoRecover:
    def __init__(self, app):
        self.app = app
        self.files = {}          # id(workbook) -> copy path
        self.n = 0
        self._source = 0
        st = getattr(app, "settings", {}) or {}
        if INTERVAL is not None:
            self.set_interval(int(INTERVAL))
        else:
            self.set_interval(int(st.get("autorecover_minutes", DEFAULT_MINUTES)) * 60 if st.get("autorecover_on", True) else 0)

    def set_interval(self, seconds):
        """How often copies are written (0: never)."""
        if self._source:
            GLib.source_remove(self._source)
            self._source = 0
        self.seconds = max(0, int(seconds))
        if self.seconds > 0:
            self._source = GLib.timeout_add_seconds(self.seconds, self.tick)

    def tick(self):
        for w in list(self.app.windows):
            try:
                self.save(w)
            except Exception:
                pass
        return True

    def save(self, w):
        """Write (or drop) the copy of one window's workbook. Returns the copy's path or None."""
        book = w.book
        if book is None:
            return None
        k = id(book)
        if not book.modified:
            self.forget(w)
            return None
        path = self.files.get(k)
        if path is None:
            os.makedirs(folder(), mode=0o700, exist_ok=True)
            self.n += 1
            safe = re.sub(r"[^\w.-]+", "_", os.path.splitext(w.doc_name())[0])[:60] or "Book"
            ext = ".xlsm" if book.has_macros() else ".xlsx"
            path = os.path.join(folder(), "%s-%d-%d%s" % (safe, os.getpid(), self.n, ext))
            self.files[k] = path
        w.write_file(path)
        meta = {"name": w.doc_name(), "path": w.path, "time": time.time(), "pid": os.getpid()}
        with open(path + ".json", "w") as f:
            json.dump(meta, f)
        return path

    def forget(self, w):
        """The workbook was saved or closed: its copy goes."""
        path = self.files.pop(id(w.book), None) if w.book is not None else None
        for p in (path, path + ".json" if path else None):
            if p and os.path.exists(p):
                try:
                    os.unlink(p)
                except OSError:
                    pass

    @staticmethod
    def left_behind():
        """Copies from programs that are no longer running: [(path, meta)], newest first."""
        d = folder()
        out = []
        if not os.path.isdir(d):
            return out
        for name in os.listdir(d):
            if not name.endswith(".json"):
                continue
            meta_p = os.path.join(d, name)
            path = meta_p[:-5]
            try:
                with open(meta_p) as f:
                    meta = json.load(f)
            except (OSError, ValueError):
                continue
            pid = int(meta.get("pid", 0) or 0)
            if not os.path.exists(path) or pid == os.getpid() or _alive(pid):
                # still in use by this or another running Folio Sheet
                continue
            out.append((path, meta))
        return sorted(out, key=lambda x: -x[1].get("time", 0))


def recover(app, path, meta):
    """Open a recovered copy as an unsaved workbook ("Name (Recovered)")."""
    win = app.new_window()
    win.load(path)
    win.path = None
    win.imported_from = None
    win.untitled = "%s (Recovered)" % os.path.splitext(meta.get("name") or "Book")[0]
    win.book.mark_modified()
    win.update_title()
    for p in (path, path + ".json"):
        try:
            os.unlink(p)
        except OSError:
            pass
    return win


def offer(app, parent):
    """Document Recovery: the copies left behind, each to open or delete."""
    items = AutoRecover.left_behind()
    if not items:
        return []
    d = Gtk.Dialog(title="Document Recovery", transient_for=parent, modal=True)
    d.add_button("Delete All", 2)
    d.add_button("Close", Gtk.ResponseType.CLOSE)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(8)
    box.pack_start(Gtk.Label(label="Folio Sheet ended before these workbooks were saved. Open the ones you want to keep.",
                             xalign=0, wrap=True, max_width_chars=60), False, False, 0)
    opened = []
    for path, meta in items:
        row = Gtk.Box(spacing=12)
        when = time.strftime("%-d %b %Y %H:%M", time.localtime(meta.get("time", 0)))
        lab = Gtk.Label(label="%s  (%s)" % (meta.get("name") or os.path.basename(path), when), xalign=0)
        row.pack_start(lab, True, True, 0)
        b = Gtk.Button(label="Open")

        def go(btn, p=path, m=meta, rw=row):
            opened.append(recover(app, p, m))
            rw.set_sensitive(False)
        b.connect("clicked", go)
        row.pack_start(b, False, False, 0)
        box.pack_start(row, False, False, 0)
    d.show_all()
    r = d.run()
    d.destroy()
    if r == 2:
        for path, _meta in items:
            for p in (path, path + ".json"):
                try:
                    os.unlink(p)
                except OSError:
                    pass
    return opened
