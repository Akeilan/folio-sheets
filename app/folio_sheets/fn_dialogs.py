"""Formulas-tab dialogs: Insert Function, Function Arguments, Name
Manager / New Name, Evaluate Formula, Error Checking and the Watch
Window."""
from gi.repository import GLib, Gtk, Pango

from . import fncatalog
from .core import a1, check_formula, ref_spans

DEFAULT_RECENT = ["SUM", "AVERAGE", "IF", "HYPERLINK", "COUNT", "MAX", "SIN", "SUMIF", "PMT", "STDEV"]


def recent_functions(win):
    lst = win.app.settings.get("recent_functions") or list(DEFAULT_RECENT)
    return [n for n in lst if n in fncatalog.FUNCTIONS][:10]


def note_recent(win, name):
    lst = [n for n in recent_functions(win) if n != name]
    win.app.settings["recent_functions"] = ([name] + lst)[:10]


def _dialog(win, title, resizable=False):
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.set_resizable(resizable)
    box = d.get_content_area()
    box.set_border_width(10)
    box.set_spacing(6)
    return d, box


def _wrap_label(text="", markup=False, width=52):
    lab = Gtk.Label(xalign=0, yalign=0)
    lab.set_line_wrap(True)
    lab.set_max_width_chars(width)
    lab.set_width_chars(width)
    if markup:
        lab.set_markup(text)
    else:
        lab.set_text(text)
    return lab


# ---------------------------------------------------------------- Insert Function
def insert_function(win):
    """Shift+F3: pick a function, then fill in its arguments."""
    d, box = _dialog(win, "Insert Function")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box.pack_start(Gtk.Label(label="Search for a function:", xalign=0), False, False, 0)
    srow = Gtk.Box(spacing=6)
    search = Gtk.Entry()
    search.set_placeholder_text("Type a brief description of what you want to do and then click Go")
    go = Gtk.Button(label="Go")
    srow.pack_start(search, True, True, 0)
    srow.pack_start(go, False, False, 0)
    box.pack_start(srow, False, False, 0)
    crow = Gtk.Box(spacing=6)
    crow.pack_start(Gtk.Label(label="Or select a category:"), False, False, 0)
    cat = Gtk.ComboBoxText()
    cats = ["Most Recently Used", "All"] + fncatalog.CATEGORIES
    for c in cats:
        cat.append_text(c)
    cat.set_active(0)
    crow.pack_start(cat, False, False, 0)
    box.pack_start(crow, False, False, 0)
    box.pack_start(Gtk.Label(label="Select a function:", xalign=0), False, False, 0)
    store = Gtk.ListStore(str)
    tv = Gtk.TreeView(model=store)
    tv.set_headers_visible(False)
    tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
    sw = Gtk.ScrolledWindow()
    sw.set_size_request(440, 180)
    sw.add(tv)
    box.pack_start(sw, True, True, 0)
    syn = _wrap_label()
    desc = _wrap_label()
    box.pack_start(syn, False, False, 0)
    box.pack_start(desc, False, False, 0)

    def fill(names):
        store.clear()
        for n in names:
            store.append([n])
        if names:
            tv.get_selection().select_path(Gtk.TreePath(0))
            tv.scroll_to_cell(Gtk.TreePath(0), None, False, 0, 0)

    def on_cat(*_):
        c = cat.get_active_text()
        if c == "Most Recently Used":
            fill(recent_functions(win))
        elif c == "All":
            fill(sorted(fncatalog.FUNCTIONS))
        elif c == "Recommended":
            pass
        else:
            fill(fncatalog.in_category(c))

    def on_go(*_):
        q = search.get_text().strip().lower()
        if not q:
            return
        words = [w for w in q.split() if len(w) > 1]
        scored = []
        for n, (_c, args, de) in fncatalog.FUNCTIONS.items():
            hay = (n + " " + de).lower()
            s = sum(2 if w in n.lower() else 1 for w in words if w in hay)
            if s:
                scored.append((-s, n))
        if "Recommended" not in [r[0] for r in cat.get_model()]:
            cat.prepend_text("Recommended")
        cat.handler_block_by_func(on_cat)
        cat.set_active(0)
        cat.handler_unblock_by_func(on_cat)
        fill([n for _s, n in sorted(scored)][:40])

    def on_sel(s):
        m, it = s.get_selected()
        if it is None:
            syn.set_text("")
            desc.set_text("")
            return
        n = m[it][0]
        syn.set_markup("<b>%s</b>" % GLib.markup_escape_text(fncatalog.syntax(n)))
        desc.set_text(fncatalog.describe(n))

    cat.connect("changed", on_cat)
    go.connect("clicked", on_go)
    search.connect("activate", on_go)
    tv.get_selection().connect("changed", on_sel)
    tv.connect("row-activated", lambda *_: d.response(Gtk.ResponseType.OK))
    on_cat()
    d.show_all()
    search.grab_focus()
    r = d.run()
    m, it = tv.get_selection().get_selected()
    name = m[it][0] if it is not None else None
    d.destroy()
    if r == Gtk.ResponseType.OK and name:
        function_arguments(win, name)


# ---------------------------------------------------------------- Function Arguments
def function_arguments(win, name):
    """The arguments of one function, with each argument's value and the
    result as you type. OK puts the function into the formula."""
    note_recent(win, name)
    g = win.grid
    book = win.book
    if g.editing():
        r, c = g.editor.cell
        sheet = g.edit_sheet if g.edit_sheet is not None else g.sheet
    else:
        r, c = g.sel.active
        sheet = g.sheet
    args = fncatalog.arg_names(name)
    variadic = any(a.strip() == "..." for a in args)
    fixed = [a for a in args if a.strip() != "..."]
    d, box = _dialog(win, "Function Arguments")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    frame = Gtk.Frame(label=name)
    grid = Gtk.Grid(column_spacing=8, row_spacing=4)
    grid.set_border_width(8)
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(min(5, max(1, len(fixed) + (1 if variadic else 0))) * 30 + 10)
    sw.add(grid)
    frame.add(sw)
    box.pack_start(frame, True, True, 0)
    entries = []
    previews = []
    result_lab = Gtk.Label(xalign=0)
    desc = _wrap_label(fncatalog.describe(name), width=60)
    arg_help = _wrap_label("", width=60)

    def add_row(label):
        i = len(entries)
        lab = Gtk.Label(label=label.strip("[]"), xalign=1)
        if not label.startswith("["):
            lab.set_markup("<b>%s</b>" % GLib.markup_escape_text(label))
        e = Gtk.Entry()
        e.set_width_chars(28)
        e.set_activates_default(True)
        pv = Gtk.Label(xalign=0)
        pv.set_ellipsize(Pango.EllipsizeMode.END)
        pv.set_max_width_chars(26)
        grid.attach(lab, 0, i, 1, 1)
        grid.attach(e, 1, i, 1, 1)
        grid.attach(pv, 2, i, 1, 1)
        entries.append(e)
        previews.append(pv)
        e.connect("changed", lambda *_: update())
        e.connect("focus-in-event", lambda *_ , lb=label: arg_help.set_markup(
            "<b>%s</b>  %s" % (GLib.markup_escape_text(lb.strip("[]")), "is optional." if lb.startswith("[") else "is required.")))
        lab.show()
        e.show()
        pv.show()
        return e

    def labels():
        out = list(fixed)
        k = len(out)
        # variadic: keep one empty slot after the last filled one
        if variadic:
            base = fixed[-1].strip("[]") if fixed else "value"
            stem = base.rstrip("0123456789") or base
            n = 2
            while len(out) < len(entries) + 1 and len(out) < 255:
                out.append("[%s%d]" % (stem, n + len(out) - k))
                if len(out) > len(entries):
                    break
        return out

    def update():
        # grow variadic lists
        if variadic and entries and entries[-1].get_text().strip() and len(entries) < 255:
            base = fixed[-1].strip("[]") if fixed else "value"
            stem = base.rstrip("0123456789") or base
            add_row("[%s%d]" % (stem, len(entries) - len(fixed) + 2))
        vals = [e.get_text().strip() for e in entries]
        while vals and not vals[-1]:
            vals.pop()
        for e, pv in zip(entries, previews):
            t = e.get_text().strip()
            if not t:
                pv.set_text("")
                continue
            v = book.evaluate(sheet, r, c, t)
            pv.set_text("= " + (v if v is not None else "?"))
        text = "%s(%s)" % (name, ",".join(vals))
        v = book.evaluate(sheet, r, c, text)
        result_lab.set_text("Formula result = %s" % (v if v is not None else ""))

    for lb in fixed or ([] if not variadic else []):
        add_row(lb)
    if variadic and not fixed:
        add_row("value1")
    if not args:
        grid.attach(Gtk.Label(label="This function takes no arguments."), 0, 0, 3, 1)
    box.pack_start(desc, False, False, 0)
    box.pack_start(arg_help, False, False, 0)
    box.pack_start(result_lab, False, False, 0)
    # pre-fill from the formula being edited, when the cursor is in this call
    d.show_all()
    update()
    if entries:
        entries[0].grab_focus()
    resp = d.run()
    vals = [e.get_text().strip() for e in entries]
    d.destroy()
    if resp != Gtk.ResponseType.OK:
        return
    while vals and not vals[-1]:
        vals.pop()
    call = "%s(%s)" % (name, ",".join(vals))
    win.insert_into_formula(call)


# ---------------------------------------------------------------- names
def edit_name(win, existing=None):
    """New Name / Edit Name. existing: (name, refers_to, scope)."""
    book = win.book
    d, box = _dialog(win, "Edit Name" if existing else "New Name")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(column_spacing=8, row_spacing=6)
    name_e = Gtk.Entry()
    name_e.set_activates_default(True)
    scope = Gtk.ComboBoxText()
    scope.append_text("Workbook")
    for i in range(book.sheet_count):
        scope.append_text(book.sheet_info(i)["name"])
    comment = Gtk.TextView()
    comment.set_wrap_mode(Gtk.WrapMode.WORD)
    csw = Gtk.ScrolledWindow()
    csw.set_size_request(300, 60)
    csw.set_shadow_type(Gtk.ShadowType.IN)
    csw.add(comment)
    ref_e = Gtk.Entry()
    ref_e.set_activates_default(True)
    g = win.grid
    if existing:
        n, ref, sc = existing
        name_e.set_text(n)
        ref_e.set_text("=" + ref.lstrip("="))
        scope.set_active(0 if sc is None else sc + 1)
        c = book.name_comment(n, sc)
        if c:
            comment.get_buffer().set_text(c)
        scope.set_sensitive(False)
    else:
        scope.set_active(0)
        sname = book.sheet_info(g.sheet)["name"]
        from .formula_edit import quote_sheet
        rg = g.sel.last
        from .core import range_a1
        absref = "$" + range_a1(rg).replace(":", ":$")
        # $A$1 style
        parts = []
        for p in range_a1(rg).split(":"):
            letters = "".join(ch for ch in p if ch.isalpha())
            digits = "".join(ch for ch in p if ch.isdigit())
            parts.append(("$" + letters if letters else "") + ("$" + digits if digits else ""))
        absref = ":".join(parts)
        ref_e.set_text("=%s!%s" % (quote_sheet(sname), absref))
        # suggest a label next to the selection
        r0, c0 = rg[0], rg[1]
        cand = ""
        if c0 > 0:
            cand = book.display(g.sheet, r0, c0 - 1)
        if not cand and r0 > 0:
            cand = book.display(g.sheet, r0 - 1, c0)
        cand = "".join(ch if ch.isalnum() else "_" for ch in cand.strip())
        if cand and (cand[0].isalpha() or cand[0] == "_"):
            name_e.set_text(cand)
    for i, (lab, w) in enumerate((("Name:", name_e), ("Scope:", scope), ("Comment:", csw), ("Refers to:", ref_e))):
        grid.attach(Gtk.Label(label=lab, xalign=0), 0, i, 1, 1)
        grid.attach(w, 1, i, 1, 1)
    box.pack_start(grid, True, True, 0)
    d.show_all()
    name_e.grab_focus()
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        n = name_e.get_text().strip()
        ref = ref_e.get_text().strip().lstrip("=")
        sc = scope.get_active() - 1
        sc = None if sc < 0 else sc
        b = comment.get_buffer()
        cm = b.get_text(b.get_start_iter(), b.get_end_iter(), True)
        try:
            book.define_name(n, ref, sc, cm or None, (existing[0], existing[2]) if existing else None)
        except ValueError as e:
            win.dialogs.message(str(e))
            continue
        d.destroy()
        win.grid.refresh()
        return True


def name_manager(win):
    book = win.book
    d, box = _dialog(win, "Name Manager", resizable=True)
    d.set_default_size(640, 380)
    bar = Gtk.Box(spacing=6)
    new_b = Gtk.Button(label="New...")
    edit_b = Gtk.Button(label="Edit...")
    del_b = Gtk.Button(label="Delete")
    for b in (new_b, edit_b, del_b):
        bar.pack_start(b, False, False, 0)
    box.pack_start(bar, False, False, 0)
    store = Gtk.ListStore(str, str, str, str, object)
    tv = Gtk.TreeView(model=store)
    for i, h in enumerate(("Name", "Value", "Refers To", "Scope")):
        col = Gtk.TreeViewColumn(h, Gtk.CellRendererText(), text=i)
        col.set_resizable(True)
        col.set_min_width(110 if i != 2 else 180)
        tv.append_column(col)
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.add(tv)
    box.pack_start(sw, True, True, 0)
    refers = Gtk.Entry()
    refers.set_editable(False)
    box.pack_start(Gtk.Label(label="Refers to:", xalign=0), False, False, 0)
    box.pack_start(refers, False, False, 0)
    d.add_button("Close", Gtk.ResponseType.CLOSE)

    def reload():
        store.clear()
        for n, ref, local, hidden in book.names():
            if hidden or n.startswith("_xlnm"):
                continue
            sc = "Workbook" if local is None else book.sheet_info(local)["name"]
            sheet = local if local is not None else win.grid.sheet
            v = book.evaluate(sheet, 0, 0, ref.lstrip("=")) or ""
            store.append([n, v, "=" + ref.lstrip("="), sc, (n, ref, local)])

    def selected():
        m, it = tv.get_selection().get_selected()
        return m[it][4] if it is not None else None

    def on_sel(*_):
        s = selected()
        refers.set_text("=" + s[1].lstrip("=") if s else "")
        edit_b.set_sensitive(bool(s))
        del_b.set_sensitive(bool(s))

    new_b.connect("clicked", lambda *_: (edit_name(win), reload()))
    edit_b.connect("clicked", lambda *_: (selected() and edit_name(win, selected()), reload()))

    def on_del(*_):
        s = selected()
        if not s:
            return
        q = Gtk.MessageDialog(transient_for=d, modal=True, message_type=Gtk.MessageType.QUESTION,
                              buttons=Gtk.ButtonsType.OK_CANCEL,
                              text="Are you sure you want to delete the name %s?" % s[0])
        ok = q.run() == Gtk.ResponseType.OK
        q.destroy()
        if ok:
            book.delete_name(s[0], s[2])
            reload()
            win.grid.refresh()
    del_b.connect("clicked", on_del)
    tv.get_selection().connect("changed", on_sel)
    tv.connect("row-activated", lambda *_: edit_b.clicked())
    reload()
    on_sel()
    d.show_all()
    d.run()
    d.destroy()


def create_from_selection(win):
    """Names from the labels in the top row / left column of the selection."""
    g = win.grid
    book = win.book
    r1, c1, r2, c2 = g.sel.last
    d, box = _dialog(win, "Create Names from Selection")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box.pack_start(Gtk.Label(label="Create names from values in the:", xalign=0), False, False, 0)
    checks = {}
    top_is_text = all(book.display(g.sheet, r1, c) and not book.display(g.sheet, r1, c)[0].isdigit()
                      for c in range(c1 + (1 if c2 > c1 else 0), c2 + 1))
    for k, lab in (("top", "Top row"), ("left", "Left column"), ("bottom", "Bottom row"), ("right", "Right column")):
        cb = Gtk.CheckButton(label=lab)
        checks[k] = cb
        box.pack_start(cb, False, False, 0)
    checks["top"].set_active(top_is_text and r2 > r1)
    checks["left"].set_active(not top_is_text and c2 > c1)
    d.show_all()
    r = d.run()
    sel = {k: cb.get_active() for k, cb in checks.items()}
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return
    sname = book.sheet_info(g.sheet)["name"]
    from .formula_edit import quote_sheet

    def absref(ra, ca, rb, cb):
        from .core import col_name
        a = "$%s$%d" % (col_name(ca), ra + 1)
        b = "$%s$%d" % (col_name(cb), rb + 1)
        return "%s!%s" % (quote_sheet(sname), a if (ra, ca) == (rb, cb) else a + ":" + b)

    def clean(t):
        t = "".join(ch if ch.isalnum() or ch in "_." else "_" for ch in t.strip())
        if t and not (t[0].isalpha() or t[0] == "_"):
            t = "_" + t
        return t
    made = 0
    dr1 = r1 + (1 if sel["top"] else 0)
    dr2 = r2 - (1 if sel["bottom"] else 0)
    dc1 = c1 + (1 if sel["left"] else 0)
    dc2 = c2 - (1 if sel["right"] else 0)
    jobs = []
    if sel["top"]:
        jobs += [(book.display(g.sheet, r1, c), absref(dr1, c, dr2, c)) for c in range(dc1, dc2 + 1)]
    if sel["bottom"]:
        jobs += [(book.display(g.sheet, r2, c), absref(dr1, c, dr2, c)) for c in range(dc1, dc2 + 1)]
    if sel["left"]:
        jobs += [(book.display(g.sheet, r, c1), absref(r, dc1, r, dc2)) for r in range(dr1, dr2 + 1)]
    if sel["right"]:
        jobs += [(book.display(g.sheet, r, c2), absref(r, dc1, r, dc2)) for r in range(dr1, dr2 + 1)]
    for label, ref in jobs:
        n = clean(label)
        if not n:
            continue
        try:
            book.define_name(n, ref)
            made += 1
        except ValueError:
            # an existing name of the same scope: replace it, as the original asks
            try:
                book.define_name(n, ref, None, None, (n, None))
                made += 1
            except ValueError:
                pass
    win.grid.refresh()


def paste_names(win):
    names = [n for n, _r, _l, h in win.book.names() if not h and not n.startswith("_xlnm")]
    d, box = _dialog(win, "Paste Name")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box.pack_start(Gtk.Label(label="Paste name", xalign=0), False, False, 0)
    store = Gtk.ListStore(str)
    for n in sorted(names, key=str.lower):
        store.append([n])
    tv = Gtk.TreeView(model=store)
    tv.set_headers_visible(False)
    tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
    sw = Gtk.ScrolledWindow()
    sw.set_size_request(240, 180)
    sw.add(tv)
    box.pack_start(sw, True, True, 0)
    tv.connect("row-activated", lambda *_: d.response(Gtk.ResponseType.OK))
    d.show_all()
    r = d.run()
    m, it = tv.get_selection().get_selected()
    n = m[it][0] if it is not None else None
    d.destroy()
    if r == Gtk.ResponseType.OK and n:
        win.insert_into_formula(n, call=False)


# ---------------------------------------------------------------- Evaluate Formula
def _innermost(text):
    """The span (a, b) to evaluate next: the first reference, else the
    innermost function call or parenthesised group with no calls inside."""
    for (a, b, *_r) in ref_spans(text):
        return a, b
    best = None
    depth = 0
    stack = []
    in_str = False
    for i, ch in enumerate(text):
        if ch == '"':
            in_str = not in_str
        if in_str:
            continue
        if ch == "(":
            j = i
            while j > 0 and (text[j - 1].isalnum() or text[j - 1] in "._"):
                j -= 1
            stack.append(j)
        elif ch == ")" and stack:
            a = stack.pop()
            best = (a, i + 1)
            break
    return best


def evaluate_formula(win):
    g = win.grid
    book = win.book
    r, c = g.sel.active
    text = book.input(g.sheet, r, c)
    if not text.startswith("="):
        win.dialogs.info("The active cell doesn't contain a formula.", "Evaluate Formula")
        return
    expr = text[1:].lstrip("{").rstrip("}") if text.startswith("{") else text[1:]
    d, box = _dialog(win, "Evaluate Formula", resizable=True)
    d.set_default_size(560, 260)
    box.pack_start(Gtk.Label(label="Reference: %s!%s" % (book.sheet_info(g.sheet)["name"], a1(r, c)), xalign=0),
                   False, False, 0)
    box.pack_start(Gtk.Label(label="Evaluation:", xalign=0), False, False, 0)
    view = Gtk.TextView()
    view.set_editable(False)
    view.set_wrap_mode(Gtk.WrapMode.CHAR)
    buf = view.get_buffer()
    tag = buf.create_tag("next", underline=Pango.Underline.SINGLE, weight=Pango.Weight.BOLD)
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.add(view)
    box.pack_start(sw, True, True, 0)
    hint = _wrap_label("To show the result of the underlined expression, click Evaluate. The most recent "
                       "result appears italicized.", width=70)
    box.pack_start(hint, False, False, 0)
    bb = Gtk.Box(spacing=6)
    ev_b = Gtk.Button(label="Evaluate")
    restart = Gtk.Button(label="Restart")
    close_b = Gtk.Button(label="Close")
    bb.pack_end(close_b, False, False, 0)
    bb.pack_end(ev_b, False, False, 0)
    bb.pack_end(restart, False, False, 0)
    box.pack_start(bb, False, False, 0)
    state = {"text": expr, "done": False}

    def show():
        t = state["text"]
        buf.set_text("=" + t)
        span = None if state["done"] else _innermost(t)
        if span:
            buf.apply_tag(tag, buf.get_iter_at_offset(span[0] + 1), buf.get_iter_at_offset(span[1] + 1))
        elif not state["done"]:
            buf.apply_tag(tag, buf.get_iter_at_offset(1), buf.get_end_iter())
        ev_b.set_label("Restart" if state["done"] else "Evaluate")

    def step(*_):
        if state["done"]:
            state["text"] = expr
            state["done"] = False
            show()
            return
        t = state["text"]
        span = _innermost(t)
        if span is None:
            v = book.evaluate(g.sheet, r, c, t, text.startswith("{"))
            state["text"] = v if v is not None else t
            state["done"] = True
        else:
            a, b = span
            v = book.evaluate(g.sheet, r, c, t[a:b], text.startswith("{"))
            if v is None:
                v = "#VALUE!"
            state["text"] = t[:a] + v + t[b:]
        show()
    ev_b.connect("clicked", step)
    restart.connect("clicked", lambda *_: (state.update(text=expr, done=False), show()))
    close_b.connect("clicked", lambda *_: d.response(Gtk.ResponseType.CLOSE))
    show()
    d.show_all()
    d.run()
    d.destroy()


# ---------------------------------------------------------------- Error Checking
def error_checking(win):
    """Steps through cells with error values on the active sheet."""
    g = win.grid
    book = win.book
    cells = book.error_cells(g.sheet)
    errs = [(r, c) for r, c, _e in cells]
    kinds = {(r, c): e for r, c, e in cells}
    if not errs:
        win.dialogs.info("The error check of the entire sheet is complete.", "Error Checking")
        return
    d, box = _dialog(win, "Error Checking")
    head = Gtk.Label(xalign=0)
    formula = Gtk.Label(xalign=0)
    formula.set_selectable(True)
    why = _wrap_label(width=48)
    box.pack_start(head, False, False, 0)
    box.pack_start(formula, False, False, 0)
    box.pack_start(why, False, False, 0)
    d.add_button("Previous", 1)
    d.add_button("Next", 2)
    d.add_button("Edit in Formula Bar", 3)
    d.add_button("Close", Gtk.ResponseType.CLOSE)
    reasons = {"#DIV/0!": "The formula or function divides by zero or by an empty cell.",
               "#N/A": "A value is not available to the formula or function.",
               "#NAME?": "The formula contains unrecognized text.",
               "#NULL!": "The ranges in the formula don't intersect.",
               "#NUM!": "The formula or function contains invalid numeric values.",
               "#REF!": "The formula refers to cells that are not valid.",
               "#VALUE!": "A value used in the formula is of the wrong data type.",
               "#SPILL!": "The formula's results would overwrite other data.",
               "#CALC!": "The calculation produced an empty array."}
    idx = [0]

    def show():
        r, c = errs[idx[0]]
        g.select_cell(r, c)
        v = kinds.get((r, c), "")
        head.set_markup("<b>Error in cell %s</b>" % a1(r, c))
        formula.set_text(book.input(g.sheet, r, c))
        why.set_text(reasons.get(v, "The cell holds an error value."))
    show()
    d.show_all()
    while True:
        resp = d.run()
        if resp == 1:
            idx[0] = (idx[0] - 1) % len(errs)
            show()
        elif resp == 2:
            if idx[0] + 1 >= len(errs):
                d.destroy()
                win.dialogs.info("The error check of the entire sheet is complete.", "Error Checking")
                return
            idx[0] += 1
            show()
        elif resp == 3:
            d.destroy()
            g.start_edit(mode="edit")
            return
        else:
            break
    d.destroy()


# ---------------------------------------------------------------- Watch Window
class WatchWindow(Gtk.Window):
    def __init__(self, win):
        Gtk.Window.__init__(self, title="Watch Window", transient_for=win)
        self.win = win
        self.get_style_context().add_class("folio-dialog")
        self.set_default_size(620, 180)
        self.set_type_hint(1)  # dialog
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.set_border_width(6)
        bar = Gtk.Box(spacing=6)
        add = Gtk.Button(label="Add Watch...")
        rem = Gtk.Button(label="Delete Watch")
        bar.pack_start(add, False, False, 0)
        bar.pack_start(rem, False, False, 0)
        box.pack_start(bar, False, False, 0)
        self.store = Gtk.ListStore(str, str, str, str, str, object)
        tv = Gtk.TreeView(model=self.store)
        for i, h in enumerate(("Sheet", "Name", "Cell", "Value", "Formula")):
            col = Gtk.TreeViewColumn(h, Gtk.CellRendererText(), text=i)
            col.set_resizable(True)
            col.set_min_width(80)
            tv.append_column(col)
        self.tv = tv
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        self.add(box)
        self.watches = []
        add.connect("clicked", lambda *_: self.add_selection())
        rem.connect("clicked", lambda *_: self.remove_selected())
        self.connect("delete-event", lambda *_: (self.hide(), True)[1])
        self._version = None
        GLib.timeout_add(500, self._tick)

    def add_selection(self):
        g = self.win.grid
        r1, c1, r2, c2 = g.sel.last
        for r in range(r1, min(r2, r1 + 200) + 1):
            for c in range(c1, min(c2, c1 + 50) + 1):
                if (g.sheet, r, c) not in self.watches:
                    self.watches.append((g.sheet, r, c))
        self.reload()

    def remove_selected(self):
        m, it = self.tv.get_selection().get_selected()
        if it is not None:
            self.watches.remove(m[it][5])
            self.reload()

    def reload(self):
        book = self.win.book
        self.store.clear()
        for (s, r, c) in self.watches:
            if s >= book.sheet_count:
                continue
            name = ""
            for n, ref, local, hidden in book.names():
                if ref.replace("$", "").lower().endswith("!" + a1(r, c).lower()):
                    name = n
            f = book.input(s, r, c)
            self.store.append([book.sheet_info(s)["name"], name, a1(r, c), book.display(s, r, c),
                               f if f.startswith("=") else "", (s, r, c)])
        self._version = book.version

    def _tick(self):
        if not self.get_visible():
            return True
        try:
            if self.win.book.version != self._version:
                self.reload()
        except Exception:
            return False
        return True


__all__ = ["insert_function", "function_arguments", "name_manager", "edit_name", "create_from_selection",
           "paste_names", "evaluate_formula", "error_checking", "WatchWindow", "recent_functions"]
