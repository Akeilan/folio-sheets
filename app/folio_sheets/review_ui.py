"""Review tools: Workbook Statistics, Check Accessibility, Allow Edit Ranges (and the
check that lets those ranges be changed on a protected sheet), Workbook Links."""
import os
import re
import xml.etree.ElementTree as ET
from xml.sax.saxutils import quoteattr

from gi.repository import GLib, Gtk

from . import APP_NAME
from .core import a1, col_name, parse_ref, range_a1

NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"


def _dialog(win, title, buttons=(("OK", Gtk.ResponseType.OK),)):
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    for label, resp in buttons:
        d.add_button(label, resp)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    return d, box


# ------------------------------------------------------------------ Workbook Statistics
def statistics(win):
    """The counts Review > Workbook Statistics shows, for the sheet and for the workbook."""
    b = win.book
    s = win.grid.sheet

    def sheet_stats(i):
        objs = b.objects(i)
        kinds = [o["kind"] for o in objs]
        data, formulas = b.sheet_counts(i)
        ext = b.used_extent(i)
        notes = len(b.notes(i)) if hasattr(b, "notes") else 0
        threads = len(b.threads(i)) if hasattr(b, "threads") else 0
        return {
            "end": a1(ext[0], ext[1]) if ext else "A1",
            "cells": data, "tables": len(b.tables(i)),
            "pivots": sum(1 for k in range(b.pivot_count()) if b.pivot(k)["sheet"] == i),
            "formulas": formulas, "charts": kinds.count("chart"), "images": kinds.count("picture"),
            "objects": sum(1 for k in kinds if k not in ("chart", "picture")),
            "comments": threads, "notes": notes,
        }
    cur = sheet_stats(s)
    allst = [sheet_stats(i) for i in range(b.sheet_count)]
    total = lambda k: sum(x[k] for x in allst)  # noqa: E731
    wb = {"sheets": b.sheet_count, "cells": total("cells"), "tables": total("tables"), "pivots": total("pivots"),
          "formulas": total("formulas"), "charts": total("charts"), "links": len(b.ext_links()),
          "macros": 1 if b.has_macros() else 0}
    return cur, wb


def workbook_statistics(win):
    cur, wb = statistics(win)
    d, box = _dialog(win, "Workbook Statistics")
    d.set_resizable(False)
    name = win.book.sheet_info(win.grid.sheet)["name"]

    def table(rows):
        g = Gtk.Grid(column_spacing=40, row_spacing=3)
        for k, (label, val) in enumerate(rows):
            g.attach(Gtk.Label(label=label, xalign=0), 0, k, 1, 1)
            v = Gtk.Label(label=str(val), xalign=1)
            v.set_hexpand(True)
            g.attach(v, 1, k, 1, 1)
        return g
    t = Gtk.Label(xalign=0)
    t.set_markup("<b>Current sheet:</b>  %s" % GLib.markup_escape_text(name))
    box.pack_start(t, False, False, 0)
    box.pack_start(table([("End of sheet", cur["end"]), ("Cells with data", cur["cells"]), ("Tables", cur["tables"]),
                          ("PivotTables", cur["pivots"]), ("Formulas", cur["formulas"]), ("Charts", cur["charts"]),
                          ("Images", cur["images"]), ("Objects", cur["objects"]), ("Comments", cur["comments"]),
                          ("Notes", cur["notes"])]), False, False, 0)
    box.pack_start(Gtk.Separator(), False, False, 6)
    t = Gtk.Label(xalign=0)
    t.set_markup("<b>Workbook</b>")
    box.pack_start(t, False, False, 0)
    box.pack_start(table([("Sheets", wb["sheets"]), ("Cells with data", wb["cells"]), ("Tables", wb["tables"]),
                          ("PivotTables", wb["pivots"]), ("Formulas", wb["formulas"]), ("Charts", wb["charts"]),
                          ("External connections", wb["links"]), ("Macros", wb["macros"])]), False, False, 0)
    d.stats = (cur, wb)
    d.show_all()
    d.run()
    d.destroy()
    return cur, wb


# ------------------------------------------------------------------ Check Accessibility
def _luminance(rgb):
    def ch(v):
        v /= 255.0
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = sorted((_luminance(a), _luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def accessibility_issues(win):
    """[(category, kind, sheet, where, why, fix)] — category Errors / Warnings / Tips."""
    b = win.book
    out = []
    for s in range(b.sheet_count):
        info = b.sheet_info(s)
        if info.get("state", "visible") != "visible":
            continue
        name = info["name"]
        for i, o in enumerate(b.objects(s)):
            if o["kind"] in ("ink",):
                continue
            p = b.object_props(s, i) if hasattr(b, "object_props") else {}
            if not p.get("descr") and not p.get("decorative"):
                out.append(("Errors", "Missing alternative text", s, ("object", i), "%s (%s)" % (o["name"] or o["kind"], name),
                            "Describe the object so people who can't see it understand it."))
        for (r1, c1, r2, c2) in b.merges(s):
            out.append(("Warnings", "Merged cells", s, ("cell", r1, c1), "%s (%s)" % (range_a1((r1, c1, r2, c2)), name),
                        "Unmerge the cells: screen readers read merged cells poorly."))
        if re.fullmatch(r"Sheet\d+", name) and b.has_data(s):
            out.append(("Tips", "Default sheet names", s, ("sheet",), name,
                        "Give the sheet a name that says what's on it."))
        for t in b.tables(s):
            if not t.get("header", True):
                r1, c1 = t["ref"][0], t["ref"][1]
                out.append(("Errors", "No header row specified", s, ("cell", r1, c1), "%s (%s)" % (t["name"], name),
                            "Turn on the table's header row."))
        ext = b.used_extent(s)
        if ext:
            seen = 0
            for r in range(0, min(ext[0], 2000) + 1):
                for c in range(0, min(ext[1], 200) + 1):
                    if not b.has_content(s, r, c):
                        continue
                    seen += 1
                    sp = b.style_spec(s, r, c)
                    fg = sp.get("font", {}).get("color")
                    bg = sp.get("fill", {}).get("color")
                    if isinstance(fg, int) and isinstance(bg, int) and contrast(fg, bg) < 4.5:
                        out.append(("Warnings", "Hard-to-read text contrast", s, ("cell", r, c), "%s (%s)" % (a1(r, c), name),
                                    "Make the text and the fill colours contrast more."))
                    if seen > 20000:
                        break
        # links whose text is the address itself
        for (r, c) in getattr(b, "link_cells", lambda _s: [])(s):
            t = b.display(s, r, c)
            if re.match(r"^(https?://|www\.)", t or ""):
                out.append(("Warnings", "Unclear hyperlink text", s, ("cell", r, c), "%s (%s)" % (a1(r, c), name),
                            "Use text that says where the link goes."))
    return out


class AccessibilityPane(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.win = win
        self.set_size_request(300, -1)
        self.get_style_context().add_class("folio-pane")
        self.set_name("accessibility-pane")
        head = Gtk.Box(spacing=6)
        head.set_border_width(6)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<span size='large'>Accessibility</span>")
        lab.get_style_context().add_class("folio-pane-title")
        head.pack_start(lab, True, True, 0)
        close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda _b: self.hide())
        head.pack_end(close, False, False, 0)
        self.pack_start(head, False, False, 0)
        self.store = Gtk.TreeStore(str, object)
        self.tree = Gtk.TreeView(model=self.store)
        self.tree.set_headers_visible(False)
        self.tree.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        self.tree.get_selection().connect("changed", self._picked)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_vexpand(True)
        sw.add(self.tree)
        self.pack_start(sw, True, True, 0)
        self.why = Gtk.Label(xalign=0, wrap=True, max_width_chars=36)
        self.why.set_margin_start(8)
        self.why.set_margin_end(8)
        self.pack_start(self.why, False, False, 0)
        self.fix = Gtk.Button(label="Add a description...")
        self.fix.set_no_show_all(True)
        self.fix.connect("clicked", self._fix)
        self.pack_start(self.fix, False, False, 4)
        self.check_while = Gtk.CheckButton(label="Keep accessibility checker running while I work")
        self.check_while.set_margin_start(8)
        self.pack_start(self.check_while, False, False, 6)
        self.issues = []
        self.current = None

    def load(self):
        self.store.clear()
        self.issues = accessibility_issues(self.win)
        if not self.issues:
            self.store.append(None, ["No accessibility issues found. People with disabilities should not have "
                                     "difficulty reading this workbook.", None])
            self.why.set_text("")
            return
        for cat in ("Errors", "Warnings", "Tips"):
            items = [x for x in self.issues if x[0] == cat]
            if not items:
                continue
            top = self.store.append(None, ["%s (%d)" % (cat, len(items)), None])
            kinds = {}
            for it in items:
                kinds.setdefault(it[1], []).append(it)
            for k, its in kinds.items():
                kk = self.store.append(top, ["%s (%d)" % (k, len(its)), None])
                for it in its:
                    self.store.append(kk, [it[4], it])
        self.tree.expand_all()

    def _picked(self, sel):
        m, it = sel.get_selected()
        if it is None or m[it][1] is None:
            self.fix.hide()
            return
        issue = m[it][1]
        self.current = issue
        self.why.set_text("Why fix: " + issue[5])
        self.fix.set_visible(issue[1] == "Missing alternative text")
        win = self.win
        s, where = issue[2], issue[3]
        if win.grid.sheet != s:
            win.grid.set_sheet(s)
            win.sheet_changed()
        if where[0] == "cell":
            win.grid.select_cell(where[1], where[2])
        elif where[0] == "object":
            win.grid.sel_objects = [where[1]]
            win.grid.refresh()

    def _fix(self, *_):
        if self.current and self.current[3][0] == "object":
            alt_text(self.win, self.current[2], self.current[3][1])
            self.load()


def alt_text(win, s, i):
    """The Alt Text box for an object: its description (or Mark as decorative)."""
    b = win.book
    p = b.object_props(s, i)
    d, box = _dialog(win, "Alt Text", (("OK", Gtk.ResponseType.OK), ("Cancel", Gtk.ResponseType.CANCEL)))
    box.pack_start(Gtk.Label(label="How would you describe this object and its context to someone who is blind?",
                             xalign=0, wrap=True, max_width_chars=50), False, False, 0)
    tv = Gtk.TextView()
    tv.set_wrap_mode(Gtk.WrapMode.WORD)
    tv.get_buffer().set_text(p.get("descr") or "")
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(360, 100)
    sw.add(tv)
    box.pack_start(sw, True, True, 0)
    deco = Gtk.CheckButton(label="Mark as decorative")
    deco.set_active(bool(p.get("decorative")))
    box.pack_start(deco, False, False, 0)
    d.text, d.deco = tv, deco
    d.show_all()
    r = d.run()
    buf = tv.get_buffer()
    text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)
    dec = deco.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return False
    b.set_object_props(s, [i], {"descr": text, "decorative": dec}, "Alt Text")
    win.update_title()
    return True


def check_accessibility(win):
    pane = getattr(win, "access_pane", None)
    if pane is None:
        pane = win.access_pane = AccessibilityPane(win)
        win.mid.pack_start(pane, False, False, 0)
    if pane.get_visible() and getattr(win, "_access_toggle", True):
        pane.hide()
        return pane
    pane.show_all()
    pane.fix.hide()
    pane.load()
    return pane


# ------------------------------------------------------------------ Allow Edit Ranges
def ranges_of(win, s=None):
    """[(title, [rects], attrs)] for a sheet's ranges that may be changed while it is protected."""
    s = win.grid.sheet if s is None else s
    x = win.book.sheet_element(s, "protectedRanges")
    if not x:
        return []
    try:
        root = ET.fromstring(x)
    except ET.ParseError:
        return []
    out = []
    for e in root:
        if not e.tag.endswith("protectedRange"):
            continue
        rects = []
        for tok in (e.get("sqref") or "").split():
            rg = parse_ref(tok.replace("$", ""))
            if rg:
                rects.append(tuple(rg))
        attrs = [(k, v) for k, v in e.attrib.items() if k not in ("name", "sqref")]
        out.append((e.get("name") or "Range", rects, attrs))
    return out


def write_ranges(win, s, items):
    if not items:
        win.book.set_sheet_element(s, "protectedRanges", None, "Allow Edit Ranges")
        return
    parts = []
    for title, rects, attrs in items:
        a = "".join(" %s=%s" % (k, quoteattr(v)) for k, v in attrs)
        parts.append('<protectedRange name=%s sqref="%s"%s/>' % (quoteattr(title), " ".join(range_a1(r) for r in rects), a))
    win.book.set_sheet_element(s, "protectedRanges", '<protectedRanges xmlns="%s">%s</protectedRanges>' % (NS, "".join(parts)),
                               "Allow Edit Ranges")


def _intersect(a, b):
    r1, c1 = max(a[0], b[0]), max(a[1], b[1])
    r2, c2 = min(a[2], b[2]), min(a[3], b[3])
    return (r1, c1, r2, c2) if r1 <= r2 and c1 <= c2 else None


def subtract(rect, cut):
    """rect minus cut, as up to four rectangles."""
    x = _intersect(rect, cut)
    if x is None:
        return [rect]
    r1, c1, r2, c2 = rect
    out = []
    if r1 < x[0]:
        out.append((r1, c1, x[0] - 1, c2))
    if x[2] < r2:
        out.append((x[2] + 1, c1, r2, c2))
    if c1 < x[1]:
        out.append((x[0], c1, x[2], x[1] - 1))
    if x[3] < c2:
        out.append((x[0], x[3] + 1, x[2], c2))
    return out


def locked_cells(win, ranges, ask=True):
    """Whether any of the ranges holds a locked cell that no Allow Edit Range opens.
    A range with a password asks for it once (ask=False: only ranges opened already count)."""
    from . import protect_ui
    s = win.grid.sheet
    rem = [tuple(r) for r in ranges]
    opened = win.__dict__.setdefault("unlocked_ranges", set())
    for title, rects, attrs in ranges_of(win, s):
        if not any(_intersect(a, r) for a in rem for r in rects):
            continue
        if protect_ui.has_password(attrs) and (s, title) not in opened:
            if not ask:
                continue
            pw = win.dialogs.ask_password("Unlock Range", "A cell you are trying to change is password protected.\n\n"
                                                          "Enter the password to change this cell:")
            if pw is None:
                continue
            if not protect_ui.check_password(attrs, pw):
                win.dialogs.message(protect_ui.WRONG)
                continue
            opened.add((s, title))
        for r in rects:
            rem = [p for a in rem for p in subtract(a, r)]
        if not rem:
            return False
    return bool(rem) and win.book.locked_in(s, rem)


def range_dialog(win, title="New Range", name="", ref="", has_pw=False):
    """Title, cells and password of one range; (title, rects, password or None to keep)."""
    d, box = _dialog(win, title, (("OK", Gtk.ResponseType.OK), ("Cancel", Gtk.ResponseType.CANCEL)))
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(column_spacing=8, row_spacing=6)
    t = Gtk.Entry(text=name, activates_default=True)
    r = Gtk.Entry(text=ref, activates_default=True)
    p = Gtk.Entry(visibility=False, activates_default=True)
    if has_pw:
        p.set_text("********")
    for k, (lab, w) in enumerate((("Title:", t), ("Refers to cells:", r), ("Range password:", p))):
        grid.attach(Gtk.Label(label=lab, xalign=0), 0, k, 1, 1)
        w.set_hexpand(True)
        grid.attach(w, 1, k, 1, 1)
    box.pack_start(grid, False, False, 0)
    d.title_e, d.ref_e, d.pw_e = t, r, p
    d.show_all()
    while True:
        resp = d.run()
        if resp != Gtk.ResponseType.OK:
            d.destroy()
            return None
        rects = []
        bad = False
        for tok in re.split(r"[,\s]+", r.get_text().strip().lstrip("=")):
            if not tok:
                continue
            tok = tok.split("!")[-1].replace("$", "")
            rg = parse_ref(tok)
            if rg is None:
                bad = True
                break
            rects.append(tuple(rg))
        if bad or not rects:
            win.dialogs.message("The reference must be to cells on this sheet.", title)
            if getattr(d, "_walk_runs", 0):
                d.destroy()
                return None
            continue
        pw = p.get_text()
        out = (t.get_text().strip() or "Range1", rects, None if has_pw and pw == "********" else pw)
        d.destroy()
        if out[2]:
            from .protect_ui import confirm_password
            if not confirm_password(win, out[2]):
                return None
        return out


def allow_edit_ranges(win):
    from . import protect_ui
    s = win.grid.sheet
    if protect_ui.protected(win):
        win.dialogs.message("Allow Edit Ranges isn't available while the sheet is protected. Unprotect the sheet first.",
                            "Allow Edit Ranges")
        return None
    items = ranges_of(win, s)
    d, box = _dialog(win, "Allow Users to Edit Ranges", (("OK", Gtk.ResponseType.OK), ("Cancel", Gtk.ResponseType.CANCEL),
                                                         ("Apply", Gtk.ResponseType.APPLY)))
    box.pack_start(Gtk.Label(label="Ranges unlocked by a password when sheet is protected:", xalign=0), False, False, 0)
    store = Gtk.ListStore(str, str)
    tv = Gtk.TreeView(model=store)
    for k, h in enumerate(("Title", "Refers to cells")):
        tv.append_column(Gtk.TreeViewColumn(h, Gtk.CellRendererText(), text=k))
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(380, 140)
    sw.add(tv)
    row = Gtk.Box(spacing=8)
    row.pack_start(sw, True, True, 0)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    row.pack_start(side, False, False, 0)
    box.pack_start(row, True, True, 0)

    def fill():
        store.clear()
        for title, rects, _a in items:
            store.append([title, ",".join("$" + range_a1(r).replace(":", ":$") for r in rects).replace("$", "")])

    def selected():
        m, it = tv.get_selection().get_selected()
        return None if it is None else m.get_path(it).get_indices()[0]

    def new(*_):
        n = 1
        while any(x[0] == "Range%d" % n for x in items):
            n += 1
        g = win.grid
        res = range_dialog(win, "New Range", "Range%d" % n, ",".join(range_a1(tuple(r)) for r in g.sel.ranges))
        if res:
            title, rects, pw = res
            items.append((title, rects, protect_ui.hash_password(pw) if pw else []))
            fill()

    def modify(*_):
        k = selected()
        if k is None:
            return
        title, rects, attrs = items[k]
        res = range_dialog(win, "Modify Range", title, ",".join(range_a1(r) for r in rects), protect_ui.has_password(attrs))
        if res:
            t2, r2, pw = res
            items[k] = (t2, r2, attrs if pw is None else (protect_ui.hash_password(pw) if pw else []))
            fill()

    def delete(*_):
        k = selected()
        if k is not None:
            items.pop(k)
            fill()
    for label, fn in (("New...", new), ("Modify...", modify), ("Delete", delete)):
        bt = Gtk.Button(label=label)
        bt.connect("clicked", fn)
        side.pack_start(bt, False, False, 0)
    prot = Gtk.Button(label="Protect Sheet...")
    box.pack_start(prot, False, False, 0)
    prot.connect("clicked", lambda *_: d.response(100))
    d.items, d.new, d.modify, d.delete = items, new, modify, delete
    fill()
    d.show_all()
    while True:
        r = d.run()
        if r in (Gtk.ResponseType.OK, Gtk.ResponseType.APPLY, 100):
            write_ranges(win, s, items)
            win.update_title()
        if r == Gtk.ResponseType.APPLY:
            if getattr(d, "_walk_runs", 0) > 1:
                break
            continue
        break
    d.destroy()
    if r == 100:
        protect_ui.protect_sheet(win)
    return items


# ------------------------------------------------------------------ Workbook Links
def _source_path(win, target):
    t = target
    if t.startswith("file:///"):
        t = GLib.filename_from_uri(t)[0]
    elif t.startswith("file:"):
        t = t[5:]
    if not os.path.isabs(t):
        base = os.path.dirname(win.path or win.imported_from or os.getcwd()) if getattr(win, "path", None) or getattr(win, "imported_from", None) else os.getcwd()
        t = os.path.normpath(os.path.join(base, t.replace("\\", "/")))
    return t


class LinksPane(Gtk.Box):
    """Data > Workbook Links: the workbooks this one takes values from."""
    def __init__(self, win):
        Gtk.Box.__init__(self, orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.win = win
        self.set_size_request(300, -1)
        self.get_style_context().add_class("folio-pane")
        self.set_name("links-pane")
        head = Gtk.Box(spacing=6)
        head.set_border_width(6)
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<span size='large'>Workbook Links</span>")
        lab.get_style_context().add_class("folio-pane-title")
        head.pack_start(lab, True, True, 0)
        close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda _b: self.hide())
        head.pack_end(close, False, False, 0)
        self.pack_start(head, False, False, 0)
        tools = Gtk.Box(spacing=4)
        tools.set_margin_start(8)
        self.refresh_all = Gtk.Button(label="Refresh all")
        self.refresh_all.connect("clicked", lambda *_: self.refresh(None))
        self.break_all = Gtk.Button(label="Break all")
        self.break_all.connect("clicked", lambda *_: self.do_break(None))
        tools.pack_start(self.refresh_all, False, False, 0)
        tools.pack_start(self.break_all, False, False, 0)
        self.pack_start(tools, False, False, 0)
        self.list = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        sw = Gtk.ScrolledWindow()
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        sw.set_vexpand(True)
        sw.add(self.list)
        self.pack_start(sw, True, True, 0)

    def load(self):
        for c in self.list.get_children():
            self.list.remove(c)
        links = self.win.book.ext_links()
        if not links:
            self.list.pack_start(Gtk.Label(label="This workbook doesn't link to other workbooks.", xalign=0,
                                           wrap=True, margin_start=8), False, False, 0)
        for ln in links:
            path = _source_path(self.win, ln["target"])
            ok = os.path.exists(path)
            card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
            card.set_margin_start(8)
            card.set_margin_end(8)
            t = Gtk.Label(xalign=0)
            t.set_markup("<b>%s</b>" % GLib.markup_escape_text(os.path.basename(path)))
            card.pack_start(t, False, False, 0)
            card.pack_start(Gtk.Label(label=path, xalign=0, wrap=True, max_width_chars=36), False, False, 0)
            card.pack_start(Gtk.Label(label="Status: " + ("OK" if ok else "Error: Source not found"), xalign=0), False, False, 0)
            row = Gtk.Box(spacing=4)
            for label, fn in (("Refresh", self.refresh), ("Change source...", self.change), ("Break links", self.do_break)):
                bt = Gtk.Button(label=label)
                bt.connect("clicked", lambda _b, f=fn, n=ln["index"]: f(n))
                row.pack_start(bt, False, False, 0)
            card.pack_start(row, False, False, 0)
            card.pack_start(Gtk.Separator(), False, False, 2)
            self.list.pack_start(card, False, False, 0)
        self.list.show_all()

    def _after(self):
        win = self.win
        win.grid.refresh()
        win.update_title()
        self.load()

    def refresh(self, n):
        b = self.win.book
        bad = []
        for ln in b.ext_links():
            if n is not None and ln["index"] != n:
                continue
            path = _source_path(self.win, ln["target"])
            try:
                b.ext_update(ln["index"], path)
            except Exception:
                bad.append(os.path.basename(path))
        if bad:
            self.win.dialogs.message("These links couldn't be refreshed because the source workbook isn't there: "
                                     + ", ".join(bad), "Workbook Links")
        self._after()

    def change(self, n, path=None):
        if path is None:
            d = Gtk.FileChooserNative(title="Change Source", transient_for=self.win, action=Gtk.FileChooserAction.OPEN)
            r = d.run()
            path = d.get_filename() if r == Gtk.ResponseType.ACCEPT else None
            d.destroy()
            if not path:
                return False
        b = self.win.book
        target = path
        if self.win.path:
            try:
                target = os.path.relpath(path, os.path.dirname(self.win.path))
            except ValueError:
                pass
        b.ext_set_source(n, target)
        try:
            b.ext_update(n, path)
        except Exception:
            pass
        self._after()
        return True

    def do_break(self, n):
        b = self.win.book
        links = b.ext_links()
        if not links:
            return 0
        d = Gtk.MessageDialog(transient_for=self.win, modal=True, message_type=Gtk.MessageType.WARNING,
                              buttons=Gtk.ButtonsType.NONE,
                              text="Breaking links permanently converts formulas and external references to their "
                                   "existing values. Because this can't be undone, you may want to save a version of "
                                   "this file with a new name.")
        d.set_title(APP_NAME)
        d.add_button("Break links", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        r = d.run()
        d.destroy()
        if r != Gtk.ResponseType.OK:
            return 0
        k = 0
        for ln in sorted(links, key=lambda x: -x["index"]):
            if n is None or ln["index"] == n:
                k += b.ext_break(ln["index"])
        self._after()
        return k


def workbook_links(win):
    pane = getattr(win, "links_pane", None)
    if pane is None:
        pane = win.links_pane = LinksPane(win)
        win.mid.pack_start(pane, False, False, 0)
    pane.show_all()
    pane.load()
    return pane
