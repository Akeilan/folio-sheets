"""More local data sources for Get Data (S3-06): XML, JSON, PDF tables and SQLite files on this
computer, and From Table/Range in this workbook. Each source lists its tables (items) for the
Navigator and reads one into a Result."""
import json
import os
import sqlite3
import xml.etree.ElementTree as ET


def _res(headers, rows):
    from .getdata_ui import Result, _headers, _kind
    n = max([len(headers)] + [len(r) for r in rows] + [1])
    headers = _headers([str(h) for h in headers] + [""] * (n - len(headers)), n)
    rows = [[(r[j] if j < len(r) else "") for j in range(n)] for r in rows]
    kinds = [_kind([r[j] for r in rows]) for j in range(n)]
    return Result(headers, rows, kinds)


def _text(v):
    if v is None:
        return ""
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, float):
        return "%.15g" % v
    if isinstance(v, (dict, list)):
        return json.dumps(v, ensure_ascii=False)
    t = str(v)
    return t


# ------------------------------------------------------------------ JSON
def _flatten(obj, pre=""):
    out = {}
    for k, v in obj.items():
        key = "%s.%s" % (pre, k) if pre else str(k)
        if isinstance(v, dict):
            out.update(_flatten(v, key))
        else:
            out[key] = v
    return out


def _json_tables(data, path="", out=None):
    """(label, path) of every list in the document (lists of records first)."""
    out = [] if out is None else out
    if isinstance(data, list):
        out.append((path or "List", path))
        return out
    if isinstance(data, dict):
        if not path:
            out.append(("Record", ""))
        for k, v in data.items():
            if isinstance(v, (list, dict)):
                _json_tables(v, (path + "." if path else "") + str(k), out)
    return out


def _json_at(data, path):
    if not path:
        return data
    for part in path.split("."):
        data = data[part]
    return data


def json_items(path):
    with open(path, encoding="utf-8-sig") as f:
        data = json.load(f)
    items = _json_tables(data)
    # lists first: they are what people load
    items.sort(key=lambda x: 0 if isinstance(_json_at(data, x[1]), list) else 1)
    return items


def read_json(q):
    with open(q["path"], encoding="utf-8-sig") as f:
        data = json.load(f)
    node = _json_at(data, q.get("item", ""))
    if isinstance(node, dict):
        flat = _flatten(node)
        return _res(["Name", "Value"], [[k, _text(v)] for k, v in flat.items()])
    if not isinstance(node, list):
        return _res(["Value"], [[_text(node)]])
    if node and all(isinstance(x, dict) for x in node):
        recs = [_flatten(x) for x in node]
        cols = []
        for r in recs:
            for k in r:
                if k not in cols:
                    cols.append(k)
        return _res(cols, [[_text(r.get(c)) for c in cols] for r in recs])
    return _res(["Value"], [[_text(x)] for x in node])


# ------------------------------------------------------------------ XML
def _local(tag):
    return tag.split("}", 1)[-1]


def _xml_tables(root):
    """Paths of elements that repeat under one parent (the rows of a table)."""
    found = {}

    def walk(el, path):
        kids = list(el)
        counts = {}
        for k in kids:
            counts[_local(k.tag)] = counts.get(_local(k.tag), 0) + 1
        for k in kids:
            p = path + "/" + _local(k.tag)
            if counts[_local(k.tag)] > 1 or len(kids) == 1 and (len(k) or k.attrib):
                found.setdefault(p, 0)
                found[p] += 1
            walk(k, p)
    walk(root, "/" + _local(root.tag))
    return [p for p, n in found.items() if n >= 1]


def xml_items(path):
    root = ET.parse(path).getroot()
    items = [(p.rsplit("/", 1)[-1] + "  (" + p + ")", p) for p in _xml_tables(root)]
    return items or [(_local(root.tag), "/" + _local(root.tag))]


def _find_path(root, path):
    parts = path.strip("/").split("/")
    if not parts or _local(root.tag) != parts[0]:
        return []
    cur = [root]
    for p in parts[1:]:
        cur = [k for el in cur for k in el if _local(k.tag) == p]
    return cur


def read_xml(q):
    root = ET.parse(q["path"]).getroot()
    rows_el = _find_path(root, q.get("item") or "/" + _local(root.tag))
    cols, recs = [], []
    for el in rows_el:
        rec = {}
        for k, v in el.attrib.items():
            rec["Attribute:" + _local(k)] = v
        for k in el:
            name = _local(k.tag)
            if len(k):
                # a nested element: its text children flattened
                for kk in k:
                    rec["%s.%s" % (name, _local(kk.tag))] = (kk.text or "").strip()
            else:
                rec[name] = (k.text or "").strip()
        if not len(el) and not el.attrib:
            rec[_local(el.tag)] = (el.text or "").strip()
        for k in rec:
            if k not in cols:
                cols.append(k)
        recs.append(rec)
    return _res(cols, [[rec.get(c, "") for c in cols] for rec in recs])


# ------------------------------------------------------------------ SQLite
def sqlite_items(path):
    con = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
    try:
        rows = con.execute("SELECT name, type FROM sqlite_master WHERE type IN ('table','view') "
                           "AND name NOT LIKE 'sqlite_%' ORDER BY type, name").fetchall()
    finally:
        con.close()
    return [(n + ("  (view)" if t == "view" else ""), n) for n, t in rows]


def read_sqlite(q):
    con = sqlite3.connect("file:%s?mode=ro" % q["path"], uri=True)
    try:
        cur = con.execute('SELECT * FROM "%s"' % q["item"].replace('"', '""'))
        heads = [d[0] for d in cur.description]
        rows = [[_text(v) if not isinstance(v, bytes) else "" for v in r] for r in cur.fetchmany(1048575)]
    finally:
        con.close()
    return _res(heads, rows)


# ------------------------------------------------------------------ PDF
def _pdf_lines(page):
    """The page's text as lines of words with positions: [(y, [(x1, x2, word)])]."""
    text = page.get_text() or ""
    ok, rects = page.get_text_layout()
    if not ok or not rects:
        return []
    chars = list(text)
    n = min(len(chars), len(rects))
    words, cur = [], None
    for i in range(n):
        ch, r = chars[i], rects[i]
        if ch.isspace():
            if cur:
                words.append(cur)
                cur = None
            continue
        if cur and (r.x1 - cur[1] > max(1.5, (r.x2 - r.x1) * 0.6) or abs(r.y1 - cur[3]) > 2):
            words.append(cur)
            cur = None
        if cur is None:
            cur = [r.x1, r.x2, ch, r.y1, r.y2]
        else:
            cur[1] = r.x2
            cur[2] += ch
    if cur:
        words.append(cur)
    lines = []
    for w in sorted(words, key=lambda w: (round(w[3]), w[0])):
        if lines and abs(lines[-1][0] - w[3]) <= 2.5:
            lines[-1][1].append(w)
        else:
            lines.append([w[3], [w]])
    out = []
    for y, ws in lines:
        ws.sort(key=lambda w: w[0])
        # words close together are one cell
        cells = []
        for w in ws:
            h = w[4] - w[3]
            if cells and w[0] - cells[-1][1] < max(h * 0.9, 4):
                cells[-1][1] = w[1]
                cells[-1][2] += " " + w[2]
            else:
                cells.append([w[0], w[1], w[2]])
        out.append((y, cells))
    return out


def _pdf_tables(page):
    """Runs of three or more lines with two or more cells each, columns lined up by position."""
    lines = _pdf_lines(page)
    tables, run = [], []
    for y, cells in lines + [(None, [])]:
        if len(cells) >= 2:
            run.append(cells)
            continue
        if len(run) >= 3:
            tables.append(run)
        run = []
    out = []
    for run in tables:
        # column starts: every cell's left edge, clustered
        lefts = sorted(c[0] for cells in run for c in cells)
        cols = []
        for x in lefts:
            if not cols or x - cols[-1][-1] > 8:
                cols.append([x])
            else:
                cols[-1].append(x)
        starts = [min(c) for c in cols if len(c) >= max(2, len(run) // 3)] or [min(c) for c in cols]
        grid = []
        for cells in run:
            row = [""] * len(starts)
            for c in cells:
                j = max([k for k, s in enumerate(starts) if s <= c[0] + 4] or [0])
                row[j] = (row[j] + " " + c[2]).strip()
            grid.append(row)
        out.append(grid)
    return out


def _poppler():
    import gi
    gi.require_version("Poppler", "0.18")
    from gi.repository import Poppler
    return Poppler


def pdf_items(path):
    P = _poppler()
    doc = P.Document.new_from_file("file://" + os.path.abspath(path), None)
    items = []
    for i in range(doc.get_n_pages()):
        for k, _t in enumerate(_pdf_tables(doc.get_page(i))):
            items.append(("Table%03d (Page %d)" % (len(items) + 1, i + 1), "%d:%d" % (i, k)))
    for i in range(doc.get_n_pages()):
        items.append(("Page%03d" % (i + 1), "%d:page" % i))
    return items


def read_pdf(q):
    P = _poppler()
    doc = P.Document.new_from_file("file://" + os.path.abspath(q["path"]), None)
    pi, k = q["item"].split(":")
    page = doc.get_page(int(pi))
    if k == "page":
        rows = [[c[2] for c in cells] for _y, cells in _pdf_lines(page)]
        return _res(["Column%d" % (j + 1) for j in range(max([len(r) for r in rows] + [1]))], rows)
    tables = _pdf_tables(page)
    if int(k) >= len(tables):
        raise ValueError("The table isn't in the PDF any more.")
    grid = tables[int(k)]
    first = grid[0]
    if all(v and not v.replace(".", "").replace(",", "").isdigit() for v in first):
        return _res(first, grid[1:])
    return _res(["Column%d" % (j + 1) for j in range(len(first))], grid)


# ------------------------------------------------------------------ From Table/Range
def read_range(q, book):
    """A table (or named range) of this workbook."""
    if book is None:
        raise ValueError("The workbook isn't open.")
    for s in range(book.sheet_count):
        for t in book.tables(s):
            if t["name"].lower() == q["item"].lower():
                r1, c1, r2, c2 = t["ref"]
                heads = [book.display(s, r1, c) for c in range(c1, c2 + 1)]
                rows = [[book.input(s, r, c) for c in range(c1, c2 + 1)] for r in range(r1 + 1, r2 + 1)]
                rows = [[v[1:] if v.startswith("=") else v for v in r] for r in rows]
                rows = [[book.display(s, r1 + 1 + i, c1 + j) if row[j] != "" and book.input(s, r1 + 1 + i, c1 + j).startswith("=") else row[j]
                         for j in range(len(row))] for i, row in enumerate(rows)]
                return _res(heads, rows)
    raise ValueError("The table '%s' wasn't found." % q["item"])


READERS = {"json": read_json, "xml": read_xml, "sqlite": read_sqlite, "pdf": read_pdf}
ITEMS = {"json": json_items, "xml": xml_items, "sqlite": sqlite_items, "pdf": pdf_items}
TITLES = {"json": ("JSON", ("*.json",)), "xml": ("XML", ("*.xml",)), "pdf": ("PDF", ("*.pdf",)),
          "sqlite": ("SQLite Database", ("*.sqlite", "*.sqlite3", "*.db", "*.db3"))}
