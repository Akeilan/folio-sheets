"""Get Data from local sources (SH-25): From Text/CSV, From Workbook and From Folder, loaded as
tables that Refresh and Refresh All read again. As in Excel's Get & Transform: a preview first,
then Load (a table on a new sheet), Load To... (where; a PivotTable or PivotChart; or only a
connection) or Transform Data (the query editor, query_ui). Also XML, JSON, PDF and SQLite files
and From Table/Range (datasources). Web, online and server database sources are not available.

A query's settings are kept in the workbook as a hidden defined name, so they travel with the
file (Excel keeps hidden names) without changing anything Excel reads."""
import csv
import datetime
import json
import os
import re

from gi.repository import GLib, Gtk, Pango

from . import APP_NAME
from .core import MAX_COLS, MAX_ROWS, Book, a1
from . import text_ui

PREFIX = "_folio_query_"
STYLE = "TableStyleMedium2"
NOT_TRANSFORM = "Combining files with Transform Data isn't available in %s. Combine & Load, then use the table." % APP_NAME
ENCODINGS = [("65001: Unicode (UTF-8)", "utf-8-sig"), ("1252: Western European (Windows)", "cp1252"),
             ("28591: Western European (ISO)", "latin-1"), ("1200: Unicode", "utf-16"), ("20127: US-ASCII", "ascii")]
DELIMS = [("Comma", ","), ("Semicolon", ";"), ("Tab", "\t"), ("Space", " "), ("Colon", ":"), ("Equals Sign", "="),
          ("Pipe", "|")]
DETECT = [("Based on first 200 rows", "200"), ("Based on entire dataset", "all"), ("Do not detect data types", "none")]


# ------------------------------------------------------------------ keeping queries in the file
def _encode(q):
    raw = json.dumps(q, separators=(",", ":"), ensure_ascii=False)
    parts = [raw[i:i + 240] for i in range(0, len(raw), 240)] or [""]
    return "=" + "&".join('"%s"' % p.replace('"', '""') for p in parts)


def _decode(text):
    lits = re.findall(r'"((?:[^"]|"")*)"', text)
    return json.loads("".join(p.replace('""', '"') for p in lits))


def _key(name):
    return PREFIX + re.sub(r"[^A-Za-z0-9_]", "_", name)


def queries(book):
    out = []
    for name, text, scope, _hidden in book.names():
        if scope is None and name.lower().startswith(PREFIX):
            try:
                q = _decode(text)
                q["_key"] = name
                out.append(q)
            except (ValueError, TypeError):
                continue
    return out


def find_query(book, name):
    for q in queries(book):
        if q["name"].lower() == name.lower():
            return q
    return None


def save_query(book, q):
    data = {k: v for k, v in q.items() if not k.startswith("_")}
    key = q.get("_key") or _key(q["name"])
    exists = any(n.lower() == key.lower() and s is None for n, _t, s, _h in book.names())
    book.define_name(key, _encode(data), None, None, (key, None) if exists else None)
    book.set_name_hidden(key, True)
    q["_key"] = key


def delete_query(book, q):
    key = q.get("_key") or _key(q["name"])
    try:
        book.delete_name(key)
    except Exception:
        pass


def unique_query_name(book, base):
    base = re.sub(r"[^\w]", "_", base).strip("_") or "Query"
    if base[0].isdigit():
        base = "_" + base
    taken = {q["name"].lower() for q in queries(book)}
    for s in range(book.sheet_count):
        taken |= {t["name"].lower() for t in book.tables(s)}
    name, k = base, 1
    while name.lower() in taken:
        k += 1
        name = "%s_%d" % (base, k)
    return name


# ------------------------------------------------------------------ reading the sources
def _num(v):
    """v as a number input, or None."""
    t = v.strip()
    if not t:
        return None
    c = text_ui.convert(t, "general")
    body = c.lstrip("+-") if c else ""
    if c and body.replace(".", "", 1).isdigit():
        return c
    try:
        float(t)
        if t.lower() not in ("nan", "inf", "-inf", "infinity"):
            return t
    except ValueError:
        pass
    return None


_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}([ T]\d{2}:\d{2}(:\d{2})?)?$")


def _kind(values):
    vals = [v for v in values if v.strip() != ""]
    if not vals:
        return "text"
    if all(_num(v) is not None for v in vals):
        return "number"
    if all(_DATE.match(v.strip()) for v in vals):
        return "date"
    if all(v.strip().upper() in ("TRUE", "FALSE") for v in vals):
        return "bool"
    return "text"


def _headers(first, n):
    names, seen = [], {}
    for j in range(n):
        h = (first[j] if j < len(first) else "").strip() or "Column%d" % (j + 1)
        base, k = h, seen.get(h.lower(), 0)
        while h.lower() in seen:
            k += 1
            h = "%s_%d" % (base, k)
        seen[h.lower()] = 0
        names.append(h)
    return names


class Result:
    """Rows as cell inputs, with each column's kind (number, date, text, bool)."""

    def __init__(self, headers, rows, kinds, formats=None):
        self.headers = headers
        self.rows = rows
        self.kinds = kinds
        self.formats = formats or {}


def read_csv(q):
    path = q["path"]
    lines = text_ui.read_lines(path, q.get("encoding", "utf-8-sig"))
    delim = q.get("delimiter", ",")
    rows = [text_ui.split_delimited(ln, delim, False, '"') for ln in lines if ln != ""]
    if not rows:
        return Result(["Column1"], [], ["text"])
    n = min(max(len(r) for r in rows), MAX_COLS)
    rows = [(r + [""] * n)[:n] for r in rows]
    detect = q.get("detect", "200")
    body = rows[1:]

    def kinds_of(data):
        if detect == "none":
            return ["text"] * n
        sample = data if detect == "all" else data[:200]
        return [_kind([r[j] for r in sample]) for j in range(n)]

    promote = q.get("headers")
    if promote is None:
        # as Power Query does: a first row of text over typed data becomes the headers
        first_text = all(_kind([v]) == "text" and v.strip() for v in rows[0])
        promote = first_text and len(rows) > 1 and (any(k != "text" for k in kinds_of(body)) or detect == "none"
                                                     or all(v.strip() for v in rows[0]))
    if promote:
        headers, data = _headers(rows[0], n), body
    else:
        headers, data = ["Column%d" % (j + 1) for j in range(n)], rows
    kinds = kinds_of(data)
    out = []
    for r in data[:MAX_ROWS - 1]:
        row = []
        for j, v in enumerate(r):
            row.append(_cell(v, kinds[j]))
        out.append(row)
    fmts = {j: "yyyy-mm-dd" for j, k in enumerate(kinds) if k == "date"}
    return Result(headers, out, kinds, fmts)


def _cell(v, kind):
    t = v.strip()
    if t == "":
        return ""
    if kind == "number":
        return _num(t) or t
    if kind in ("date", "bool"):
        return t.upper() if kind == "bool" else t
    # text: kept as text even when it looks like a number or formula
    if _num(t) is not None or t.startswith(("=", "+", "-", "@")) or _DATE.match(t):
        return "'" + v
    return v


def workbook_items(path):
    """[(label, kind, sheet index, table name or None)] of a workbook: its sheets and tables."""
    src = Book.open(path)
    items = []
    for s in range(src.sheet_count):
        info = src.sheet_info(s)
        if info.get("kind") not in (None, "work", "worksheet", "") or info.get("state") not in (None, "visible", ""):
            continue
        for t in src.tables(s):
            items.append((t["name"], "table", s, t["name"]))
        items.append((info["name"], "sheet", s, None))
    return src, items


def read_workbook(q, src=None):
    if src is None:
        src = Book.open(q["path"])
    s, rg = None, None
    if q.get("table"):
        for i in range(src.sheet_count):
            for t in src.tables(i):
                if t["name"].lower() == q["table"].lower():
                    s, rg = i, t["ref"]
        if s is None:
            raise ValueError("The table '%s' isn't in %s any more." % (q["table"], os.path.basename(q["path"])))
        has_header = True
    else:
        for i in range(src.sheet_count):
            if src.sheet_info(i)["name"].lower() == q["sheet"].lower():
                s = i
        if s is None:
            raise ValueError("The sheet '%s' isn't in %s any more." % (q["sheet"], os.path.basename(q["path"])))
        ext = src.used_extent(s)
        rg = (0, 0, ext[0], ext[1]) if ext else (0, 0, 0, 0)
        has_header = None
    r1, c1, r2, c2 = rg
    grid = [[src.value(s, r, c) for c in range(c1, c2 + 1)] for r in range(r1, r2 + 1)]
    n = c2 - c1 + 1
    if has_header is None:
        has_header = len(grid) > 1 and all(isinstance(v, str) and v.strip() for v in grid[0])
    headers = _headers([("" if v is None else str(v)) for v in grid[0]], n) if has_header else ["Column%d" % (j + 1) for j in range(n)]
    data = grid[1:] if has_header else grid
    formats, kinds = {}, []
    for j in range(n):
        col = [r[j] for r in data if r[j] is not None]
        kinds.append("number" if col and all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in col) else "text")
        if data:
            fmt = src.number_format(s, r1 + (1 if has_header else 0), c1 + j)
            if fmt and fmt != "General":
                formats[j] = fmt
    rows = [[_value_input(v) for v in r] for r in data[:MAX_ROWS - 1]]
    return Result(headers, rows, kinds, formats)


def _value_input(v):
    if v is None:
        return ""
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, (int, float)):
        return repr(float(v)) if isinstance(v, float) and not float(v).is_integer() else str(int(v)) if float(v).is_integer() else repr(v)
    if isinstance(v, tuple) and v and v[0] == "error":
        return v[1]
    s = str(v)
    if _num(s) is not None or s.startswith(("=", "+", "-", "@")):
        return "'" + s
    return s


def _stamp(t):
    d = datetime.datetime.fromtimestamp(t)
    return d.strftime("%Y-%m-%d %H:%M:%S")


def folder_files(path):
    out = []
    for root, dirs, files in os.walk(path):
        dirs.sort()
        for f in sorted(files):
            p = os.path.join(root, f)
            try:
                st = os.stat(p)
            except OSError:
                continue
            out.append((f, os.path.splitext(f)[1].lower(), st.st_atime, st.st_mtime, st.st_ctime,
                        os.path.join(root, "")))
    return out


def read_folder(q):
    files = folder_files(q["path"])
    if not q.get("combine"):
        headers = ["Name", "Extension", "Date accessed", "Date modified", "Date created", "Folder Path"]
        rows = [[_cell(n, "text"), _cell(e, "text"), _stamp(a), _stamp(m), _stamp(c), _cell(d, "text")]
                for n, e, a, m, c, d in files]
        fmts = {2: "yyyy-mm-dd hh:mm", 3: "yyyy-mm-dd hh:mm", 4: "yyyy-mm-dd hh:mm"}
        return Result(headers, rows, ["text", "text", "date", "date", "date", "text"], fmts)
    # Combine: every file like the first one, one under the other, with the file it came from
    kinds = {".csv", ".txt", ".tsv"} if (files and files[0][1] in (".csv", ".txt", ".tsv")) else {".xlsx", ".xlsm"}
    headers, rows, kinds_out, fmts = None, [], None, {}
    for n, e, _a, _m, _c, d in files:
        if e not in kinds:
            continue
        p = os.path.join(d, n)
        if e in (".csv", ".txt", ".tsv"):
            lines = text_ui.read_lines(p, q.get("encoding", "utf-8-sig"))
            delim = q.get("delimiter") or ("\t" if e == ".tsv" else (text_ui.guess_delims(lines[:50]) or ","))
            res = read_csv({"path": p, "delimiter": delim, "encoding": q.get("encoding", "utf-8-sig"),
                            "detect": q.get("detect", "200"), "headers": q.get("headers")})
        else:
            src = Book.open(p)
            first = src.sheet_info(0)["name"]
            res = read_workbook({"path": p, "sheet": first}, src)
        if headers is None:
            headers, kinds_out, fmts = res.headers, res.kinds, {j + 1: f for j, f in res.formats.items()}
        for r in res.rows:
            rows.append([_cell(n, "text")] + (r + [""] * len(headers))[:len(headers)])
    if headers is None:
        raise ValueError("There are no files in this folder that can be combined.")
    return Result(["Source.Name"] + headers, rows, ["text"] + kinds_out, fmts)


def read_source(q, book=None):
    """A query's rows as its source gives them (before its steps)."""
    from . import datasources
    kind = q["kind"]
    if kind == "csv":
        return read_csv(q)
    if kind == "workbook":
        return read_workbook(q)
    if kind == "folder":
        return read_folder(q)
    if kind == "range":
        return datasources.read_range(q, book)
    if kind in datasources.READERS:
        return datasources.READERS[kind](q)
    raise ValueError("Unknown query.")


def read(q, book=None, _seen=None):
    """A query's rows: its source, shaped by its applied steps (other queries it merges or
    appends are read the same way)."""
    from . import query_ui
    res = read_source(q, book)
    steps = q.get("steps") or []
    if not steps:
        return res
    seen = set(_seen or ()) | {q.get("name", "").lower()}

    def resolve(name):
        if name.lower() in seen:
            raise ValueError("The query '%s' refers to itself." % name)
        other = find_query(book, name) if book is not None else None
        return read(other, book, seen) if other else None
    return query_ui.apply_steps(res, steps, resolve)


# ------------------------------------------------------------------ loading and refreshing
def _write(win, s, top, left, res, label):
    b = win.book
    nrows = len(res.rows)
    ncols = len(res.headers)
    if top + nrows + 1 > MAX_ROWS or left + ncols > MAX_COLS:
        raise ValueError("The data does not fit on the sheet from there.")
    for j, h in enumerate(res.headers):
        b.set_input(s, top, left + j, h if _num(h) is None else "'" + h)
    for j, k in enumerate(res.kinds):
        if k == "text" and nrows:
            b.set_number_format(s, [(top + 1, left + j, top + nrows, left + j)], "@")
    for j, f in res.formats.items():
        if nrows:
            b.set_number_format(s, [(top + 1, left + j, top + nrows, left + j)], f)
    for i, row in enumerate(res.rows):
        for j, v in enumerate(row):
            if v != "":
                b.set_input(s, top + 1 + i, left + j, v[1:] if res.kinds[j] == "text" and v.startswith("'") else v)
    return (top, left, top + max(nrows, 1), left + ncols - 1)


def load(win, q, res, where="new", at=None):
    """Load a query's result as a table: on a new sheet named after it, or at a cell. Returns the
    sheet and table index, or None for a connection only."""
    b = win.book
    with b.group("Load Query"):
        q["rows"] = len(res.rows)
        q["refreshed"] = datetime.datetime.now().isoformat(timespec="seconds")
        if where == "connection":
            q["table"] = None
            save_query(b, q)
            refresh_pane(win)
            return None
        if where in ("pivot", "pivotchart"):
            return _load_pivot(win, q, res, where, at)
        if where == "new" or at is None:
            s = b.sheet_count
            name = q["name"][:31]
            base, k = name, 1
            while win.book.check_sheet_name(name, None):
                k += 1
                name = ("%s (%d)" % (base, k))[:31]
            at_i = win.grid.sheet + 1
            n = b.sheet_count
            s = b.insert_sheet(at_i, name)
            win._remap_states({k: (k + 1 if k >= at_i else k) for k in range(n)})
            top, left = 0, 0
        else:
            s, top, left = at
        rg = _write(win, s, top, left, res, "Load Query")
        if not res.rows:
            rg = (rg[0], rg[1], rg[0] + 1, rg[3])
        i = b.create_table(s, rg, True, STYLE)
        try:
            b.rename_table(s, i, q["name"])
        except Exception:
            pass
        q["table"] = b.tables(s)[i]["name"]
        try:
            b.autofit_cols(s, rg[1], rg[3], None)
        except Exception:
            pass
        save_query(b, q)
    win._after_structure(s)
    win.grid.refresh()
    refresh_pane(win)
    return s, i


def _load_pivot(win, q, res, where, at):
    """Load To... PivotTable Report / PivotChart: the rows go to a hidden sheet of their own
    (Excel keeps them in the connection's cache) and the PivotTable reads them there."""
    b = win.book
    name = ("Query - " + q["name"])[:31]
    base, k = name, 1
    while b.check_sheet_name(name, None):
        k += 1
        name = ("%s (%d)" % (base[:26], k))[:31]
    n = b.sheet_count
    ds = b.insert_sheet(n, name)
    b.set_sheet_state(ds, "veryHidden")
    rg = _write(win, ds, 0, 0, res, "Load Query")
    if not res.rows:
        rg = (rg[0], rg[1], rg[0] + 1, rg[3])
    ti = b.create_table(ds, rg, True, STYLE)
    try:
        b.rename_table(ds, ti, q["name"])
    except Exception:
        pass
    q["table"] = b.tables(ds)[ti]["name"]
    q["pivot"] = True
    if at is None:
        at_i = win.grid.sheet + 1
        cnt = b.sheet_count
        dest = b.insert_sheet(at_i, None)
        win._remap_states({j: (j + 1 if j >= at_i else j) for j in range(cnt)})
        top, left = 2, 0
        ds = ds + 1 if ds >= at_i else ds
    else:
        dest, top, left = at
    pi = b.create_pivot(ds, tuple(b.tables(ds)[ti]["ref"]), dest, (top, left))
    if where == "pivotchart":
        try:
            b.insert_pivot_chart(pi, "col", 420.0, 40.0)
        except Exception:
            pass
    save_query(b, q)
    win._after_structure(dest)
    win.grid.refresh()
    refresh_pane(win)
    try:
        from . import pivot_ui
        pivot_ui.show_pane(win, True)
    except Exception:
        pass
    return dest, pi


def locate(book, table):
    for s in range(book.sheet_count):
        for i, t in enumerate(book.tables(s)):
            if t["name"].lower() == (table or "").lower():
                return s, i, t
    return None


def refresh(win, q):
    """Read the source again into the query's table. Returns an error text or None."""
    try:
        res = read(q, win.book)
    except Exception as e:
        return "%s couldn't be refreshed: %s" % (q["name"], e)
    b = win.book
    found = locate(b, q.get("table"))
    with b.group("Refresh"):
        q["rows"] = len(res.rows)
        q["refreshed"] = datetime.datetime.now().isoformat(timespec="seconds")
        if found is None:
            save_query(b, q)
            return None
        s, i, t = found
        r1, c1, r2, c2 = t["ref"]
        b.clear_contents(s, [(r1, c1, r2, c2)], "Refresh")
        rg = _write(win, s, r1, c1, res, "Refresh")
        if not res.rows:
            rg = (rg[0], rg[1], rg[0] + 1, rg[3])
        b.resize_table(s, i, rg)
        save_query(b, q)
        if q.get("pivot"):
            try:
                b.refresh_all_pivots()
            except Exception:
                pass
    win.grid.refresh()
    refresh_pane(win)
    return None


def refresh_all(win):
    errors = []
    for q in queries(win.book):
        e = refresh(win, q)
        if e:
            errors.append(e)
    if hasattr(win, "status"):
        win.status.extra.set_text("" if errors else "All queries refreshed.")
    if errors:
        win.dialogs.message("\n\n".join(errors), "Refresh All")
    return errors


def refresh_current(win):
    """Refresh for the table the active cell is in (the table's own Refresh)."""
    g = win.grid
    r, c = g.sel.active
    i = win.book.table_at(g.sheet, r, c)
    if i is None:
        return refresh_all(win)
    name = win.book.tables(g.sheet)[i]["name"]
    q = next((q for q in queries(win.book) if (q.get("table") or "").lower() == name.lower()), None)
    if q is None:
        win.status.extra.set_text("This table isn't connected to a query.")
        return []
    e = refresh(win, q)
    if e:
        win.dialogs.message(e, "Refresh")
        return [e]
    return []


# ------------------------------------------------------------------ dialogs
def _preview_view(res, limit=200):
    n = len(res.headers)
    store = Gtk.ListStore(*([str] * max(n, 1)))
    for row in res.rows[:limit]:
        store.append([(v[1:] if v.startswith("'") else v) for v in (row + [""] * n)[:n]] or [""])
    view = Gtk.TreeView(model=store)
    view.set_grid_lines(Gtk.TreeViewGridLines.BOTH)
    icons_for = {"number": "1.2", "date": "▦", "text": "ABC", "bool": "✓✗"}
    for j, h in enumerate(res.headers):
        cell = Gtk.CellRendererText()
        cell.set_property("ellipsize", Pango.EllipsizeMode.END)
        col = Gtk.TreeViewColumn("%s  %s" % (icons_for.get(res.kinds[j] if j < len(res.kinds) else "text", ""), h), cell, text=j)
        col.set_resizable(True)
        col.set_min_width(70)
        view.append_column(col)
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(640, 300)
    sw.add(view)
    return sw


class _LoadButtons:
    """Load ▾ (Load / Load To...), Transform Data (not available), Cancel."""

    def __init__(self, dialog, area, combine=False):
        self.choice = None
        row = Gtk.Box(spacing=6)
        row.set_halign(Gtk.Align.END)
        if combine:
            comb = Gtk.MenuButton(label="Combine ▾")
            m = Gtk.Menu()
            for label, what in (("Combine & Load", "combine"), ("Combine & Load To...", "combine-to"),
                                ("Combine & Transform Data", None)):
                it = Gtk.MenuItem(label=label)
                if what is None:
                    it.set_sensitive(False)
                    it.set_tooltip_text(NOT_TRANSFORM)
                else:
                    it.connect("activate", lambda _i, w=what: self._go(dialog, w))
                m.append(it)
            m.show_all()
            comb.set_popup(m)
            row.pack_start(comb, False, False, 0)
            self.combine_button = comb
        load = Gtk.Button(label="Load")
        load.get_style_context().add_class("folio-default")
        load.connect("clicked", lambda *_: self._go(dialog, "load"))
        more = Gtk.MenuButton()
        more.set_tooltip_text("Load To...")
        m = Gtk.Menu()
        for label, what in (("Load", "load"), ("Load To...", "load-to")):
            it = Gtk.MenuItem(label=label)
            it.connect("activate", lambda _i, w=what: self._go(dialog, w))
            m.append(it)
        m.show_all()
        more.set_popup(m)
        split = Gtk.Box()
        split.get_style_context().add_class("linked")
        split.pack_start(load, False, False, 0)
        split.pack_start(more, False, False, 0)
        row.pack_start(split, False, False, 0)
        tr = Gtk.Button(label="Transform Data")
        tr.connect("clicked", lambda *_: self._go(dialog, "transform"))
        row.pack_start(tr, False, False, 0)
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", lambda *_: dialog.response(Gtk.ResponseType.CANCEL))
        row.pack_start(cancel, False, False, 0)
        area.pack_end(row, False, False, 0)
        self.load_button, self.transform_button = load, tr

    def _go(self, dialog, what):
        self.choice = what
        dialog.response(Gtk.ResponseType.OK)


class CsvDialog(Gtk.Dialog):
    def __init__(self, win, path):
        super().__init__(title=os.path.basename(path), transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.path = path
        self.q = {"kind": "csv", "path": path, "encoding": "utf-8-sig", "delimiter": ",", "detect": "200"}
        lines = text_ui.read_lines(path, "utf-8-sig")
        if path.lower().endswith(".tsv"):
            self.q["delimiter"] = "\t"
        elif not path.lower().endswith(".csv"):
            self.q["delimiter"] = text_ui.guess_delims(lines[:50]) or ","
        area = self.get_content_area()
        area.set_border_width(12)
        area.set_spacing(8)
        top = Gtk.Box(spacing=14)
        self.enc = self._combo(top, "File Origin", ENCODINGS, self.q["encoding"])
        known = [v for _l, v in DELIMS]
        delims = DELIMS + ([("Custom: %r" % self.q["delimiter"], self.q["delimiter"])] if self.q["delimiter"] not in known else [])
        self.delim = self._combo(top, "Delimiter", delims, self.q["delimiter"])
        self.detect = self._combo(top, "Data Type Detection", DETECT, "200")
        area.pack_start(top, False, False, 0)
        self.holder = Gtk.Box()
        area.pack_start(self.holder, True, True, 0)
        self.buttons = _LoadButtons(self, area)
        self.update()
        for w in (self.enc, self.delim, self.detect):
            w.connect("changed", lambda *_: self.update())
        self.show_all()

    def _combo(self, box, label, items, current):
        v = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        v.pack_start(Gtk.Label(label=label, xalign=0), False, False, 0)
        cb = Gtk.ComboBoxText()
        for text, val in items:
            cb.append(val, text)
        cb.set_active_id(current)
        v.pack_start(cb, False, False, 0)
        box.pack_start(v, False, False, 0)
        return cb

    def update(self):
        self.q["encoding"] = self.enc.get_active_id()
        self.q["delimiter"] = self.delim.get_active_id()
        self.q["detect"] = self.detect.get_active_id()
        self.q.pop("headers", None)
        try:
            self.result = read_csv(self.q)
        except Exception as e:
            self.result = Result(["Error"], [[str(e)]], ["text"])
        for c in self.holder.get_children():
            self.holder.remove(c)
        self.holder.pack_start(_preview_view(self.result), True, True, 0)
        self.holder.show_all()


class NavigatorDialog(Gtk.Dialog):
    """From Workbook: pick a sheet or table, see it, load it."""

    def __init__(self, win, path):
        super().__init__(title="Navigator", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.path = path
        self.src, self.items = workbook_items(path)
        area = self.get_content_area()
        area.set_border_width(12)
        area.set_spacing(8)
        body = Gtk.Box(spacing=10)
        left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.multi = Gtk.CheckButton(label="Select multiple items")
        left.pack_start(self.multi, False, False, 0)
        self.store = Gtk.TreeStore(bool, str, int)
        root = self.store.append(None, [False, "%s [%d]" % (os.path.basename(path), len(self.items)), -1])
        for k, (label, kind, _s, _t) in enumerate(self.items):
            self.store.append(root, [False, ("▦ " if kind == "table" else "▤ ") + label, k])
        self.tree = Gtk.TreeView(model=self.store, headers_visible=False)
        tog = Gtk.CellRendererToggle()
        tog.connect("toggled", self._toggled)
        self.check_col = Gtk.TreeViewColumn("", tog, active=0)
        self.tree.append_column(self.check_col)
        self.tree.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=1))
        self.tree.expand_all()
        self.tree.get_selection().connect("changed", self._picked)
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(240, 300)
        sw.add(self.tree)
        left.pack_start(sw, True, True, 0)
        body.pack_start(left, False, False, 0)
        self.holder = Gtk.Box()
        body.pack_start(self.holder, True, True, 0)
        area.pack_start(body, True, True, 0)
        self.buttons = _LoadButtons(self, area)
        self.multi.connect("toggled", lambda *_: self.check_col.set_visible(self.multi.get_active()))
        self.show_all()
        self.check_col.set_visible(False)
        if self.items:
            self.tree.get_selection().select_path(Gtk.TreePath.new_from_string("0:0"))

    def _toggled(self, cell, path):
        it = self.store.get_iter(path)
        self.store[it][0] = not self.store[it][0]

    def query_for(self, k):
        label, kind, s, table = self.items[k]
        q = {"kind": "workbook", "path": self.path}
        if kind == "table":
            q["table"] = table
        else:
            q["sheet"] = label
        return q

    def _picked(self, sel):
        m, it = sel.get_selected()
        if it is None or m[it][2] < 0:
            return
        k = m[it][2]
        try:
            res = read_workbook(self.query_for(k), self.src)
        except Exception as e:
            res = Result(["Error"], [[str(e)]], ["text"])
        for c in self.holder.get_children():
            self.holder.remove(c)
        self.holder.pack_start(_preview_view(res), True, True, 0)
        self.holder.show_all()

    def chosen(self):
        if self.multi.get_active():
            ks = [row[2] for root in self.store for row in root.iterchildren() if row[0]]
            if ks:
                return ks
        m, it = self.tree.get_selection().get_selected()
        if it is not None and m[it][2] >= 0:
            return [m[it][2]]
        return []


class FolderDialog(Gtk.Dialog):
    def __init__(self, win, path):
        super().__init__(title=path, transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.path = path
        area = self.get_content_area()
        area.set_border_width(12)
        area.set_spacing(8)
        self.result = read_folder({"kind": "folder", "path": path})
        area.pack_start(_preview_view(self.result), True, True, 0)
        self.buttons = _LoadButtons(self, area, combine=True)
        self.show_all()


class LoadToDialog(Gtk.Dialog):
    """Import Data: how to view it (Table, or only a connection) and where to put it."""

    def __init__(self, win):
        super().__init__(title="Import Data", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.win = win
        area = self.get_content_area()
        area.set_border_width(12)
        area.set_spacing(6)
        area.pack_start(Gtk.Label(label="Select how you want to view this data in your workbook.", xalign=0), False, False, 0)
        self.table = Gtk.RadioButton.new_with_label(None, "Table")
        pivot = Gtk.RadioButton.new_with_label_from_widget(self.table, "PivotTable Report")
        chart = Gtk.RadioButton.new_with_label_from_widget(self.table, "PivotChart")
        self.conn = Gtk.RadioButton.new_with_label_from_widget(self.table, "Only Create Connection")
        self.pivot, self.chart = pivot, chart
        for w in (self.table, pivot, chart, self.conn):
            area.pack_start(w, False, False, 0)
        area.pack_start(Gtk.Label(label="Where do you want to put the data?", xalign=0), False, False, 6)
        self.existing = Gtk.RadioButton.new_with_label(None, "Existing worksheet:")
        self.ref = Gtk.Entry()
        r, c = win.grid.sel.active
        self.ref.set_text("=$%s$%d" % (re.sub(r"\d", "", a1(r, c)), r + 1))
        row = Gtk.Box(spacing=6)
        row.pack_start(self.existing, False, False, 0)
        row.pack_start(self.ref, True, True, 0)
        area.pack_start(row, False, False, 0)
        self.new = Gtk.RadioButton.new_with_label_from_widget(self.existing, "New worksheet")
        self.new.set_active(True)
        area.pack_start(self.new, False, False, 0)
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.conn.connect("toggled", lambda *_: [w.set_sensitive(not self.conn.get_active()) for w in (self.existing, self.ref, self.new)])
        self.show_all()

    def where(self):
        if self.conn.get_active():
            return "connection", None
        kind = "pivot" if self.pivot.get_active() else "pivotchart" if self.chart.get_active() else None
        if self.new.get_active():
            return (kind or "new"), None
        from .core import parse_ref
        t = self.ref.get_text().lstrip("=").replace("$", "")
        try:
            pr = parse_ref(t)
            r, c = pr[0], pr[1]
        except Exception:
            m = re.match(r"([A-Za-z]+)(\d+)", t)
            if not m:
                return "new", None
            c = 0
            for ch in m.group(1).upper():
                c = c * 26 + ord(ch) - 64
            r, c = int(m.group(2)) - 1, c - 1
        return (kind or "existing"), (self.win.grid.sheet, r, c)


def finish(win, q, res, choice):
    """After a source's preview: Load, Load To... or Transform Data (the editor, then load)."""
    if choice == "transform":
        from . import query_ui
        out = query_ui.edit(win, q, read_source(q, win.book))
        if out is None:
            return None
        choice, q, res = out
        q["name"] = unique_query_name(win.book, q["name"]) if find_query(win.book, q["name"]) is not None and not q.get("_editing") else q["name"]
    where, at = _where(win, choice)
    if where is None:
        return None
    q.pop("_editing", None)
    return load(win, q, res, where, at)


def _where(win, choice):
    if choice not in ("load-to", "combine-to"):
        return "new", None
    d = LoadToDialog(win)
    r = d.run()
    w = d.where() if r == Gtk.ResponseType.OK else None
    d.destroy()
    return w if w else (None, None)


def _choose(win, title, action, patterns=None):
    d = Gtk.FileChooserNative.new(title, win, action, "Import" if action == Gtk.FileChooserAction.OPEN else "Open", "Cancel")
    if patterns:
        f = Gtk.FileFilter()
        f.set_name(patterns[0])
        for p in patterns[1]:
            f.add_pattern(p)
            f.add_pattern(p.upper())
        d.add_filter(f)
    r = d.run()
    p = d.get_filename()
    d.destroy()
    return p if r == Gtk.ResponseType.ACCEPT else None


def from_text_csv(win, path=None):
    path = path or _choose(win, "Import Data", Gtk.FileChooserAction.OPEN,
                           ("Text Files (*.prn;*.txt;*.csv;*.tsv)", ("*.prn", "*.txt", "*.csv", "*.tsv")))
    if not path:
        return None
    d = CsvDialog(win, path)
    r = d.run()
    choice, q, res = d.buttons.choice, d.q, d.result
    d.destroy()
    if r != Gtk.ResponseType.OK or not choice:
        return None
    q["name"] = unique_query_name(win.book, os.path.splitext(os.path.basename(path))[0])
    q["headers"] = bool(res.headers and res.headers[0] != "Column1") or None
    return finish(win, q, res, choice)


def from_workbook(win, path=None):
    path = path or _choose(win, "Import Data", Gtk.FileChooserAction.OPEN,
                           ("Excel Files (*.xlsx;*.xlsm;*.xltx)", ("*.xlsx", "*.xlsm", "*.xltx", "*.xltm")))
    if not path:
        return None
    try:
        d = NavigatorDialog(win, path)
    except Exception as e:
        win.dialogs.message("%s can't read '%s'.\n\n%s" % (APP_NAME, os.path.basename(path), e))
        return None
    r = d.run()
    choice, ks = d.buttons.choice, d.chosen()
    qs = [d.query_for(k) for k in ks]
    src = d.src
    d.destroy()
    if r != Gtk.ResponseType.OK or not choice or not qs:
        return None
    if len(qs) == 1:
        q = qs[0]
        q["name"] = unique_query_name(win.book, q.get("table") or q.get("sheet"))
        return finish(win, q, read_workbook(q, src), choice)
    where, at = _where(win, "load-to" if choice == "load-to" else "load")
    if where is None:
        return None
    out = None
    for q in qs:
        q["name"] = unique_query_name(win.book, q.get("table") or q.get("sheet"))
        out = load(win, q, read_workbook(q, src), "connection" if where == "connection" else "new", None)
    return out


def from_folder(win, path=None):
    path = path or _choose(win, "Folder", Gtk.FileChooserAction.SELECT_FOLDER)
    if not path:
        return None
    d = FolderDialog(win, path)
    r = d.run()
    choice, res = d.buttons.choice, d.result
    d.destroy()
    if r != Gtk.ResponseType.OK or not choice:
        return None
    q = {"kind": "folder", "path": path, "combine": choice.startswith("combine")}
    if q["combine"]:
        try:
            res = read_folder(q)
        except ValueError as e:
            win.dialogs.message(str(e), "Combine Files")
            return None
    q["name"] = unique_query_name(win.book, os.path.basename(os.path.normpath(path)) or "Folder")
    return finish(win, q, res, choice)


class ItemsNavigator(Gtk.Dialog):
    """Navigator for XML, JSON, PDF and SQLite files: the file's tables, a preview, Load."""

    def __init__(self, win, kind, path):
        from . import datasources
        super().__init__(title="Navigator", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.kind, self.path = kind, path
        self.items = datasources.ITEMS[kind](path)
        if not self.items:
            raise ValueError("There are no tables in this file.")
        area = self.get_content_area()
        area.set_border_width(12)
        area.set_spacing(8)
        body = Gtk.Box(spacing=10)
        self.store = Gtk.ListStore(str, int)
        for k, (label, _i) in enumerate(self.items):
            self.store.append([label, k])
        self.tree = Gtk.TreeView(model=self.store, headers_visible=False)
        self.tree.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(260, 300)
        sw.add(self.tree)
        body.pack_start(sw, False, False, 0)
        self.holder = Gtk.Box()
        body.pack_start(self.holder, True, True, 0)
        area.pack_start(body, True, True, 0)
        self.buttons = _LoadButtons(self, area)
        self.tree.get_selection().connect("changed", self._picked)
        self.show_all()
        self.tree.get_selection().select_path(Gtk.TreePath(0))

    def query(self):
        m, it = self.tree.get_selection().get_selected()
        k = m[it][1] if it is not None else 0
        return {"kind": self.kind, "path": self.path, "item": self.items[k][1]}

    def _picked(self, _sel):
        from . import datasources
        try:
            res = datasources.READERS[self.kind](self.query())
        except Exception as e:
            res = Result(["Error"], [[str(e)]], ["text"])
        self.result = res
        for c in self.holder.get_children():
            self.holder.remove(c)
        self.holder.pack_start(_preview_view(res), True, True, 0)
        self.holder.show_all()


def from_source(win, kind, path=None, item=None, choice=None):
    """From XML, JSON, PDF or SQLite: the Navigator, then load (tests pass item and choice)."""
    from . import datasources
    title, pats = datasources.TITLES[kind]
    path = path or _choose(win, "Import Data", Gtk.FileChooserAction.OPEN, ("%s files" % title, pats))
    if not path:
        return None
    if item is None:
        try:
            d = ItemsNavigator(win, kind, path)
        except Exception as e:
            win.dialogs.message("%s can't read '%s'.\n\n%s" % (APP_NAME, os.path.basename(path), e))
            return None
        r = d.run()
        choice, q = d.buttons.choice, d.query()
        d.destroy()
        if r != Gtk.ResponseType.OK or not choice:
            return None
    else:
        q = {"kind": kind, "path": path, "item": item}
    try:
        res = datasources.READERS[kind](q)
    except Exception as e:
        win.dialogs.message("%s can't read '%s'.\n\n%s" % (APP_NAME, os.path.basename(path), e))
        return None
    label = q["item"].strip("/").rsplit("/", 1)[-1].rsplit(".", 1)[-1] if kind in ("xml", "json") and q["item"] else None
    if kind == "pdf":
        label = "Table" if not q["item"].endswith("page") else "Page"
    q["name"] = unique_query_name(win.book, label or q["item"] or os.path.splitext(os.path.basename(path))[0])
    return finish(win, q, res, choice or "load")


def from_table_range(win, choice="transform"):
    """From Table/Range: the table at the active cell (a range becomes a table first), opened in
    the query editor."""
    g = win.grid
    b = win.book
    r, c = g.sel.active
    ti = b.table_at(g.sheet, r, c)
    if ti is None:
        rg = tuple(g.sel.ranges[-1])
        if rg[0] == rg[2] and rg[1] == rg[3]:
            rg = tuple(b.current_region(g.sheet, r, c))
        if rg[2] <= rg[0]:
            win.dialogs.message("Select a cell in a table or range of data first.", "From Table/Range")
            return None
        try:
            with b.group("Create Table"):
                ti = b.create_table(g.sheet, rg, True, STYLE)
        except ValueError as e:
            win.dialogs.message(str(e), "From Table/Range")
            return None
        g.refresh()
    t = b.tables(g.sheet)[ti]
    q = {"kind": "range", "item": t["name"], "name": unique_query_name(b, t["name"])}
    try:
        res = read_source(q, b)
    except ValueError as e:
        win.dialogs.message(str(e), "From Table/Range")
        return None
    return finish(win, q, res, choice)


def edit_query(win, q):
    """Edit a query from the Queries & Connections pane: the editor, then its table follows."""
    from . import query_ui
    import copy
    q2 = copy.deepcopy(q)
    try:
        base = read_source(q2, win.book)
    except Exception as e:
        win.dialogs.message("%s couldn't be read: %s" % (q["name"], e), "Edit")
        return None
    out = query_ui.edit(win, q2, base)
    if out is None:
        return None
    _choice, q2, _res = out
    with win.book.group("Edit Query"):
        if q2["name"] != q["name"]:
            delete_query(win.book, q)
        save_query(win.book, q2)
    return refresh(win, q2)


def not_available(win, what):
    win.dialogs.info("%s isn't available in %s. Folio Sheets gets data from files on this computer: "
                     "Text/CSV, Excel workbooks, folders, XML, JSON, PDF and SQLite files." % (what, APP_NAME), "Get Data")


# ------------------------------------------------------------------ Queries & Connections pane
def make_pane(win):
    outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    outer.set_size_request(300, -1)
    outer.get_style_context().add_class("folio-pane")
    head = Gtk.Box(spacing=6)
    head.set_border_width(6)
    lab = Gtk.Label(xalign=0)
    lab.set_markup("<span size='large'>Queries &amp; Connections</span>")
    lab.get_style_context().add_class("folio-pane-title")
    head.pack_start(lab, True, True, 0)
    close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
    close.set_relief(Gtk.ReliefStyle.NONE)
    close.connect("clicked", lambda _b: outer.hide())
    head.pack_end(close, False, False, 0)
    outer.pack_start(head, False, False, 0)
    outer.count = Gtk.Label(xalign=0)
    outer.count.set_margin_start(8)
    outer.pack_start(outer.count, False, False, 0)
    sw = Gtk.ScrolledWindow()
    sw.set_vexpand(True)
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    outer.list = Gtk.ListBox()
    outer.list.set_selection_mode(Gtk.SelectionMode.SINGLE)
    sw.add(outer.list)
    outer.pack_start(sw, True, True, 0)
    outer.set_no_show_all(True)
    head.show_all()
    outer.count.show()
    sw.show_all()
    return outer


def describe(q):
    if not q.get("table"):
        return "Connection only."
    n = q.get("rows", 0)
    return "%d row%s loaded." % (n, "" if n == 1 else "s")


def refresh_pane(win):
    pane = getattr(win, "queries_pane", None)
    if pane is None or not pane.get_visible():
        return
    for ch in pane.list.get_children():
        pane.list.remove(ch)
    qs = queries(win.book)
    pane.count.set_text("%d quer%s" % (len(qs), "y" if len(qs) == 1 else "ies"))
    for q in qs:
        row = Gtk.ListBoxRow()
        row.query = q
        box = Gtk.Box(spacing=6)
        box.set_border_width(6)
        txt = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        name = Gtk.Label(xalign=0)
        name.set_markup("<b>%s</b>" % GLib.markup_escape_text(q["name"]))
        txt.pack_start(name, False, False, 0)
        txt.pack_start(Gtk.Label(label=describe(q), xalign=0), False, False, 0)
        box.pack_start(txt, True, True, 0)
        rb = Gtk.Button.new_from_icon_name("view-refresh-symbolic", Gtk.IconSize.MENU)
        rb.set_tooltip_text("Refresh")
        rb.set_relief(Gtk.ReliefStyle.NONE)
        rb.connect("clicked", lambda _b, q=q: _pane_refresh(win, q))
        box.pack_end(rb, False, False, 0)
        eb = Gtk.Button.new_from_icon_name("document-edit-symbolic", Gtk.IconSize.MENU)
        eb.set_tooltip_text("Edit")
        eb.set_relief(Gtk.ReliefStyle.NONE)
        eb.connect("clicked", lambda _b, q=q: edit_query(win, q))
        box.pack_end(eb, False, False, 0)
        db = Gtk.Button.new_from_icon_name("user-trash-symbolic", Gtk.IconSize.MENU)
        db.set_tooltip_text("Delete")
        db.set_relief(Gtk.ReliefStyle.NONE)
        db.connect("clicked", lambda _b, q=q: _pane_delete(win, q))
        box.pack_end(db, False, False, 0)
        row.add(box)
        pane.list.add(row)
    pane.list.show_all()


def _pane_refresh(win, q):
    e = refresh(win, q)
    if e:
        win.dialogs.message(e, "Refresh")


def _pane_delete(win, q):
    """Delete the query; its table stays as ordinary data (as in Excel)."""
    with win.book.group("Delete Query"):
        delete_query(win.book, q)
    refresh_pane(win)


def toggle_pane(win, show=None):
    pane = win.queries_pane
    vis = (not pane.get_visible()) if show is None else show
    pane.set_visible(vis)
    if vis:
        refresh_pane(win)
