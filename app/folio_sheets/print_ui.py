"""Page Layout and printing: the ribbon's page commands, Page Setup, the Backstage Print
page with its preview, printing and Export to PDF."""
import os

import cairo
from gi.repository import GLib, Gtk

from . import core
from .core import a1

MARGINS = {"normal": [0.7, 0.7, 0.75, 0.75, 0.3, 0.3], "wide": [1.0, 1.0, 1.0, 1.0, 0.5, 0.5],
           "narrow": [0.25, 0.25, 0.75, 0.75, 0.3, 0.3]}
SIZES = {"letter": 1, "legal": 5, "executive": 7, "tabloid": 3, "a4": 9, "a3": 8, "a5": 11, "b5": 13}


def _set(win, spec, label="Page Setup"):
    g = win.grid
    try:
        win.book.set_page_setup(g.sheet, spec, label)
    except ValueError as e:
        win.dialogs.message(str(e), "Page Setup")
        return False
    g.refresh()
    win.update_title()
    return True


def margins(win, name):
    if name in MARGINS:
        _set(win, {"margins": MARGINS[name]}, "Margins")


def orientation(win, which):
    _set(win, {"landscape": which == "landscape"}, "Orientation")


def size(win, name):
    code = SIZES.get(name)
    if code is None:
        try:
            code = int(name)
        except ValueError:
            return
    _set(win, {"paper": code}, "Size")


def print_area(win, how):
    g = win.grid
    ps = win.book.page_setup(g.sheet)
    rgs = [tuple(r) for r in g.sel.ranges]
    if how == "set":
        _set(win, {"print_area": rgs}, "Set Print Area")
    elif how == "add":
        _set(win, {"print_area": [tuple(x) for x in ps["print_area"]] + rgs}, "Add to Print Area")
    else:
        _set(win, {"print_area": []}, "Clear Print Area")


def breaks(win, how):
    g = win.grid
    ps = win.book.page_setup(g.sheet)
    r, c = g.sel.active
    rows, cols = list(ps["row_breaks"]), list(ps["col_breaks"])
    rg = g.sel.ranges[-1]
    whole_row = rg[1] == 0 and rg[3] == core.MAX_COLS - 1
    whole_col = rg[0] == 0 and rg[2] == core.MAX_ROWS - 1
    if how == "insert":
        if r > 0 and not whole_col:
            rows.append(r)
        if c > 0 and not whole_row:
            cols.append(c)
    elif how == "remove":
        rows = [x for x in rows if x != r]
        cols = [x for x in cols if x != c]
    else:
        rows, cols = [], []
    _set(win, {"row_breaks": sorted(set(rows)), "col_breaks": sorted(set(cols))}, "Breaks")


def scale_fit(win, which, n):
    g = win.grid
    ps = win.book.page_setup(g.sheet)
    fw, fh = ps["fit"] or (0, 0)
    if which == "width":
        fw = n
    else:
        fh = n
    _set(win, {"fit": None if (fw == 0 and fh == 0) else (fw, fh)}, "Scale to Fit")


def toggle_print_option(win, key):
    ps = win.book.page_setup(win.grid.sheet)
    _set(win, {key: not ps[key]}, "Sheet Options")


# ------------------------------------------------------------------ Page Setup
def page_setup_dialog(win, page="page"):
    g = win.grid
    b = win.book
    s = g.sheet
    ps = b.page_setup(s)
    d = Gtk.Dialog(title="Page Setup", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    stack = Gtk.Stack()
    sw = Gtk.StackSwitcher()
    sw.set_stack(stack)
    sw.set_halign(Gtk.Align.CENTER)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    box.pack_start(sw, False, False, 0)
    box.pack_start(stack, True, True, 0)
    d.get_content_area().pack_start(box, True, True, 0)
    w = {}

    def named(x, n):
        x.set_name(n)
        w[n] = x
        return x

    # Page
    gp = Gtk.Grid(row_spacing=6, column_spacing=10)
    gp.attach(Gtk.Label(label="Orientation:", xalign=0), 0, 0, 1, 1)
    port = named(Gtk.RadioButton.new_with_label(None, "Portrait"), "portrait")
    land = named(Gtk.RadioButton.new_with_label_from_widget(port, "Landscape"), "landscape")
    (land if ps["landscape"] else port).set_active(True)
    gp.attach(port, 1, 0, 1, 1)
    gp.attach(land, 2, 0, 1, 1)
    gp.attach(Gtk.Label(label="Scaling:", xalign=0), 0, 1, 1, 1)
    adj = named(Gtk.RadioButton.new_with_label(None, "Adjust to"), "adjust")
    fit = named(Gtk.RadioButton.new_with_label_from_widget(adj, "Fit to"), "fit")
    pct = named(Gtk.SpinButton.new_with_range(10, 400, 5), "scale")
    pct.set_value(ps["scale"])
    fw = named(Gtk.SpinButton.new_with_range(0, 100, 1), "fit-width")
    fh = named(Gtk.SpinButton.new_with_range(0, 100, 1), "fit-height")
    if ps["fit"]:
        fit.set_active(True)
        fw.set_value(ps["fit"][0])
        fh.set_value(ps["fit"][1])
    else:
        adj.set_active(True)
        fw.set_value(1)
        fh.set_value(0)
    gp.attach(adj, 1, 1, 1, 1)
    gp.attach(pct, 2, 1, 1, 1)
    gp.attach(Gtk.Label(label="% normal size", xalign=0), 3, 1, 1, 1)
    gp.attach(fit, 1, 2, 1, 1)
    gp.attach(fw, 2, 2, 1, 1)
    gp.attach(Gtk.Label(label="page(s) wide by", xalign=0), 3, 2, 1, 1)
    gp.attach(fh, 4, 2, 1, 1)
    gp.attach(Gtk.Label(label="tall (0: automatic)", xalign=0), 5, 2, 1, 1)
    gp.attach(Gtk.Label(label="Paper size:", xalign=0), 0, 3, 1, 1)
    paper = named(Gtk.ComboBoxText(), "paper")
    for code, name, pw, ph in core.papers():
        paper.append(str(code), "%s (%.1f x %.1f in)" % (name, pw / 72, ph / 72))
    paper.set_active_id(str(ps["paper"]))
    if paper.get_active_id() is None:
        paper.set_active(0)
    gp.attach(paper, 1, 3, 4, 1)
    gp.attach(Gtk.Label(label="First page number:", xalign=0), 0, 4, 1, 1)
    first = named(Gtk.Entry(text=str(ps["first_page"]) if ps["first_page"] else "Auto"), "first-page")
    gp.attach(first, 1, 4, 2, 1)
    stack.add_titled(gp, "page", "Page")
    # Margins
    gm = Gtk.Grid(row_spacing=6, column_spacing=10)
    spins = []
    for k, lab in enumerate(("Left", "Right", "Top", "Bottom", "Header", "Footer")):
        gm.attach(Gtk.Label(label=lab + ":", xalign=0), (k % 2) * 2, k // 2, 1, 1)
        sp = named(Gtk.SpinButton.new_with_range(0, 10, 0.05), "margin-" + lab.lower())
        sp.set_digits(2)
        sp.set_value(ps["margins"][k])
        gm.attach(sp, (k % 2) * 2 + 1, k // 2, 1, 1)
        spins.append(sp)
    ch = named(Gtk.CheckButton(label="Center horizontally"), "center-h")
    ch.set_active(ps["center_h"])
    cv = named(Gtk.CheckButton(label="Center vertically"), "center-v")
    cv.set_active(ps["center_v"])
    gm.attach(Gtk.Label(label="(inches)", xalign=0), 0, 3, 2, 1)
    gm.attach(ch, 0, 4, 2, 1)
    gm.attach(cv, 2, 4, 2, 1)
    stack.add_titled(gm, "margins", "Margins")
    # Header/Footer
    gh = Gtk.Grid(row_spacing=6, column_spacing=10)
    hdr = named(Gtk.Entry(text=ps["header"]), "header")
    ftr = named(Gtk.Entry(text=ps["footer"]), "footer")
    hdr.set_width_chars(40)
    gh.attach(Gtk.Label(label="Header:", xalign=0), 0, 0, 1, 1)
    gh.attach(hdr, 1, 0, 1, 1)
    gh.attach(Gtk.Label(label="Footer:", xalign=0), 0, 1, 1, 1)
    gh.attach(ftr, 1, 1, 1, 1)
    presets = named(Gtk.ComboBoxText(), "footer-preset")
    for code, lab in (("", "(none)"), ("&CPage &P", "Page 1"), ("&CPage &P of &N", "Page 1 of ?"), ("&C&A", "Sheet name"),
                      ("&C&F", "File name"), ("&L&A&RPage &P", "Sheet name, Page 1"), ("&L&D&R&T", "Date, time")):
        presets.append(code, lab)
    presets.connect("changed", lambda c: ftr.set_text(c.get_active_id() or ""))
    gh.attach(Gtk.Label(label="Footer preset:", xalign=0), 0, 2, 1, 1)
    gh.attach(presets, 1, 2, 1, 1)
    help_lab = Gtk.Label(xalign=0)
    help_lab.set_markup("<small>&amp;L, &amp;C, &amp;R: left, centre, right · &amp;P page · &amp;N pages · &amp;D date · &amp;T time · &amp;A sheet · &amp;F file · &amp;B bold · &amp;I italic</small>")
    gh.attach(help_lab, 0, 3, 2, 1)
    stack.add_titled(gh, "header", "Header/Footer")
    # Sheet
    gs = Gtk.Grid(row_spacing=6, column_spacing=10)
    pa = named(Gtk.Entry(text=",".join("%s:%s" % (a1(r[0], r[1]), a1(r[2], r[3])) for r in ps["print_area"])), "print-area")
    gs.attach(Gtk.Label(label="Print area:", xalign=0), 0, 0, 1, 1)
    gs.attach(pa, 1, 0, 2, 1)
    tr = named(Gtk.Entry(text="$%d:$%d" % (ps["title_rows"][0] + 1, ps["title_rows"][1] + 1) if ps["title_rows"] else ""), "title-rows")
    tc = named(Gtk.Entry(text="$%s:$%s" % (core.col_name(ps["title_cols"][0]), core.col_name(ps["title_cols"][1])) if ps["title_cols"] else ""), "title-cols")
    gs.attach(Gtk.Label(label="Rows to repeat at top:", xalign=0), 0, 1, 1, 1)
    gs.attach(tr, 1, 1, 2, 1)
    gs.attach(Gtk.Label(label="Columns to repeat at left:", xalign=0), 0, 2, 1, 1)
    gs.attach(tc, 1, 2, 2, 1)
    gl = named(Gtk.CheckButton(label="Gridlines"), "gridlines")
    gl.set_active(ps["gridlines"])
    hd = named(Gtk.CheckButton(label="Row and column headings"), "headings")
    hd.set_active(ps["headings"])
    gs.attach(gl, 0, 3, 1, 1)
    gs.attach(hd, 1, 3, 2, 1)
    dt = named(Gtk.RadioButton.new_with_label(None, "Down, then over"), "down-over")
    ot = named(Gtk.RadioButton.new_with_label_from_widget(dt, "Over, then down"), "over-down")
    (ot if ps["over_then_down"] else dt).set_active(True)
    gs.attach(Gtk.Label(label="Page order:", xalign=0), 0, 4, 1, 1)
    gs.attach(dt, 1, 4, 1, 1)
    gs.attach(ot, 2, 4, 1, 1)
    stack.add_titled(gs, "sheet", "Sheet")
    d.show_all()
    stack.set_visible_child_name(page)
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        try:
            spec = dict(_parse_dialog(w, spins))
        except ValueError as e:
            win.dialogs.message(str(e), "Page Setup")
            continue
        if _set(win, spec):
            break
    d.destroy()
    return True


def _lines(text, rows):
    """'$1:$2' -> (0, 1) for rows; '$A:$B' -> (0, 1) for columns."""
    t = text.strip().replace("$", "")
    if not t:
        return None
    if "!" in t:
        t = t.split("!", 1)[1]
    a, _, bb = t.partition(":")
    bb = bb or a
    if rows:
        try:
            return (int(a) - 1, int(bb) - 1)
        except ValueError:
            raise ValueError("Rows to repeat must look like $1:$2.")
    try:
        return (core.parse_ref(a + "1")[1], core.parse_ref(bb + "1")[1])
    except Exception:
        raise ValueError("Columns to repeat must look like $A:$B.")


def _parse_dialog(w, spins):
    out = {"landscape": w["landscape"].get_active(), "paper": int(w["paper"].get_active_id() or 1),
           "scale": int(w["scale"].get_value()),
           "fit": (int(w["fit-width"].get_value()), int(w["fit-height"].get_value())) if w["fit"].get_active() else None,
           "margins": [sp.get_value() for sp in spins], "center_h": w["center-h"].get_active(),
           "center_v": w["center-v"].get_active(), "header": w["header"].get_text(), "footer": w["footer"].get_text(),
           "gridlines": w["gridlines"].get_active(), "headings": w["headings"].get_active(),
           "over_then_down": w["over-down"].get_active()}
    fp = w["first-page"].get_text().strip()
    out["first_page"] = int(fp) if fp.isdigit() else None
    areas = []
    for part in w["print-area"].get_text().split(","):
        part = part.strip().lstrip("=").replace("$", "")
        if not part:
            continue
        if "!" in part:
            part = part.split("!", 1)[1]
        x = core.parse_ref(part)
        if not x:
            raise ValueError("The print area isn't a valid reference.")
        areas.append(tuple(x))
    out["print_area"] = areas
    out["title_rows"] = _lines(w["title-rows"].get_text(), True)
    out["title_cols"] = _lines(w["title-cols"].get_text(), False)
    return out


# ------------------------------------------------------------------ PDF and printing
def write_pdf(win, path, sheets=None):
    """Every page of the sheets (default: the active one) into a PDF file."""
    b = win.book
    sheets = sheets if sheets is not None else [win.grid.sheet]
    surf = None
    name = os.path.basename(win.path or "") if getattr(win, "path", None) else ""
    n = 0
    for s in sheets:
        ps = b.page_setup(s)
        w, h = ps["paper_size"]
        for k in range(len(b.pages(s))):
            if surf is None:
                surf = cairo.PDFSurface(path, w, h)
            else:
                surf.set_size(w, h)
            cr = cairo.Context(surf)
            b.render_page(s, k, cr, name)
            cr.show_page()
            n += 1
    if surf is None:
        win.dialogs.message("There's nothing to print on this sheet.", "Export")
        return 0
    surf.finish()
    return n


def export_pdf(win, path=None):
    if path is None:
        d = Gtk.FileChooserNative(title="Export to PDF", transient_for=win, action=Gtk.FileChooserAction.SAVE,
                                  accept_label="Export", cancel_label="Cancel")
        d.set_do_overwrite_confirmation(True)
        base = os.path.splitext(os.path.basename(getattr(win, "path", None) or "Book1"))[0]
        d.set_current_name(base + ".pdf")
        f = Gtk.FileFilter()
        f.set_name("PDF (*.pdf)")
        f.add_pattern("*.pdf")
        d.add_filter(f)
        r = d.run()
        path = d.get_filename() if r == Gtk.ResponseType.ACCEPT else None
        d.destroy()
        if not path:
            return False
        if not path.lower().endswith(".pdf"):
            path += ".pdf"
    n = write_pdf(win, path)
    if n:
        win.status.extra.set_text("Exported %d page%s to %s." % (n, "" if n == 1 else "s", os.path.basename(path)))
    return n > 0


def print_sheet(win, export_to=None):
    """Print through the system's print dialog (or export through it, for tests)."""
    b = win.book
    s = win.grid.sheet
    ps = b.page_setup(s)
    pages = b.pages(s)
    if not pages:
        win.dialogs.message("There's nothing to print on this sheet.", "Print")
        return False
    op = Gtk.PrintOperation()
    op.set_n_pages(len(pages))
    op.set_unit(Gtk.Unit.POINTS)
    op.set_use_full_page(True)
    setup = Gtk.PageSetup()
    w, h = ps["paper_size"]
    portrait = (min(w, h), max(w, h))
    paper = Gtk.PaperSize.new_custom("folio", "Sheet paper", portrait[0], portrait[1], Gtk.Unit.POINTS)
    setup.set_paper_size(paper)
    setup.set_orientation(Gtk.PageOrientation.LANDSCAPE if ps["landscape"] else Gtk.PageOrientation.PORTRAIT)
    op.set_default_page_setup(setup)
    op.set_job_name(os.path.basename(getattr(win, "path", None) or "Book1"))

    def draw(_op, ctx, k):
        cr = ctx.get_cairo_context()
        b.render_page(s, k, cr, os.path.basename(getattr(win, "path", None) or ""))
    op.connect("draw-page", draw)
    if export_to:
        op.set_export_filename(export_to)
        res = op.run(Gtk.PrintOperationAction.EXPORT, win)
    else:
        res = op.run(Gtk.PrintOperationAction.PRINT_DIALOG, win)
    return res in (Gtk.PrintOperationResult.APPLY,)


def preview_widget(win):
    """The Backstage Print page: settings on the left, the pages on the right."""
    b = win.book
    s = win.grid.sheet
    outer = Gtk.Box(spacing=18)
    left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    left.set_size_request(280, -1)
    outer.pack_start(left, False, False, 0)
    pb = Gtk.Button(label="Print")
    pb.set_name("print-button")
    pb.get_style_context().add_class("suggested-action")
    pb.connect("clicked", lambda _b: print_sheet(win))
    left.pack_start(pb, False, False, 0)
    ex = Gtk.Button(label="Export to PDF...")
    ex.connect("clicked", lambda _b: export_pdf(win))
    left.pack_start(ex, False, False, 0)
    left.pack_start(Gtk.Separator(), False, False, 4)
    ps = b.page_setup(s)
    orient = Gtk.ComboBoxText()
    orient.set_name("orientation")
    orient.append("portrait", "Portrait Orientation")
    orient.append("landscape", "Landscape Orientation")
    orient.set_active_id("landscape" if ps["landscape"] else "portrait")
    left.pack_start(orient, False, False, 0)
    paper = Gtk.ComboBoxText()
    paper.set_name("paper")
    for code, name, pw, ph in core.papers():
        paper.append(str(code), name)
    paper.set_active_id(str(ps["paper"]))
    left.pack_start(paper, False, False, 0)
    marg = Gtk.ComboBoxText()
    marg.set_name("margins")
    for key, lab in (("normal", "Normal Margins"), ("wide", "Wide Margins"), ("narrow", "Narrow Margins"), ("custom", "Custom Margins")):
        marg.append(key, lab)
    cur = next((k for k, v in MARGINS.items() if [round(x, 2) for x in v] == [round(x, 2) for x in ps["margins"]]), "custom")
    marg.set_active_id(cur)
    left.pack_start(marg, False, False, 0)
    scl = Gtk.ComboBoxText()
    scl.set_name("scaling")
    for key, lab in (("none", "No Scaling"), ("sheet", "Fit Sheet on One Page"), ("cols", "Fit All Columns on One Page"),
                     ("rows", "Fit All Rows on One Page")):
        scl.append(key, lab)
    fit = ps["fit"]
    scl.set_active_id({None: "none", (1, 1): "sheet", (1, 0): "cols", (0, 1): "rows"}.get(tuple(fit) if fit else None, "none"))
    left.pack_start(scl, False, False, 0)
    setup = Gtk.Button(label="Page Setup")
    setup.set_relief(Gtk.ReliefStyle.NONE)
    setup.connect("clicked", lambda _b: (page_setup_dialog(win), refresh()))
    left.pack_start(setup, False, False, 0)
    right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    outer.pack_start(right, True, True, 0)
    area = Gtk.DrawingArea()
    area.set_name("preview")
    area.set_size_request(420, 520)
    right.pack_start(area, True, True, 0)
    nav = Gtk.Box(spacing=6)
    nav.set_halign(Gtk.Align.CENTER)
    prev = Gtk.Button(label="◀")
    nxt = Gtk.Button(label="▶")
    lab = Gtk.Label()
    lab.set_name("page-label")
    nav.pack_start(prev, False, False, 0)
    nav.pack_start(lab, False, False, 0)
    nav.pack_start(nxt, False, False, 0)
    right.pack_start(nav, False, False, 0)
    state = {"k": 0}

    def refresh(*_):
        n = len(b.pages(s))
        state["k"] = min(state["k"], max(0, n - 1))
        lab.set_text("%d of %d" % (state["k"] + 1 if n else 0, n))
        area.queue_draw()

    def on_draw(a, cr):
        n = len(b.pages(s))
        pw, ph = b.page_setup(s)["paper_size"]
        aw, ah = a.get_allocated_width(), a.get_allocated_height()
        sc = min((aw - 20) / pw, (ah - 20) / ph)
        x0 = (aw - pw * sc) / 2
        y0 = (ah - ph * sc) / 2
        cr.set_source_rgb(0.85, 0.85, 0.85)
        cr.paint()
        cr.set_source_rgba(0, 0, 0, 0.25)
        cr.rectangle(x0 + 3, y0 + 3, pw * sc, ph * sc)
        cr.fill()
        cr.save()
        cr.translate(x0, y0)
        cr.scale(sc, sc)
        if n:
            b.render_page(s, state["k"], cr, os.path.basename(getattr(win, "path", None) or ""))
        else:
            cr.set_source_rgb(1, 1, 1)
            cr.rectangle(0, 0, pw, ph)
            cr.fill()
        cr.restore()
        return False
    area.connect("draw", on_draw)
    prev.connect("clicked", lambda _b: (state.__setitem__("k", max(0, state["k"] - 1)), refresh()))
    nxt.connect("clicked", lambda _b: (state.__setitem__("k", state["k"] + 1), refresh()))

    def on_orient(c):
        _set(win, {"landscape": c.get_active_id() == "landscape"}, "Orientation")
        refresh()

    def on_paper(c):
        _set(win, {"paper": int(c.get_active_id())}, "Size")
        refresh()

    def on_marg(c):
        if c.get_active_id() in MARGINS:
            _set(win, {"margins": MARGINS[c.get_active_id()]}, "Margins")
            refresh()

    def on_scale(c):
        _set(win, {"fit": {"none": None, "sheet": (1, 1), "cols": (1, 0), "rows": (0, 1)}[c.get_active_id()]}, "Scaling")
        refresh()
    orient.connect("changed", on_orient)
    paper.connect("changed", on_paper)
    marg.connect("changed", on_marg)
    scl.connect("changed", on_scale)
    refresh()
    outer.area = area
    outer.label = lab
    outer.refresh = refresh
    outer.show_all()
    return outer
