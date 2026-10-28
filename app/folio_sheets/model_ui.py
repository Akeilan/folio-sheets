"""The Data Model: tables related by key columns (Data > Relationships), and PivotTables
built from several related tables. The related tables are joined into a hidden sheet the
PivotTable reads, so the file opens as a normal PivotTable in any spreadsheet program; the
model itself (tables and relationships) is kept in a hidden name."""
import json

from gi.repository import Gtk

MODEL_NAME = "_Folio_DataModel"
MODEL_SHEET = "Data Model"


def load(win):
    b = win.book
    for n in b.names():
        if n[0] == MODEL_NAME and n[2] is None:
            t = n[1].strip()
            if t.startswith('"') and t.endswith('"'):
                try:
                    m = json.loads(t[1:-1].replace('""', '"'))
                    m.setdefault("tables", [])
                    m.setdefault("rels", [])
                    return m
                except ValueError:
                    break
    return {"tables": [], "rels": []}


def save(win, m):
    b = win.book
    text = '"%s"' % json.dumps(m, separators=(",", ":")).replace('"', '""')
    if len(text) > 8000:
        raise ValueError("The Data Model is too large to keep.")
    exists = any(n[0] == MODEL_NAME and n[2] is None for n in b.names())
    b.define_name(MODEL_NAME, text, None, None, (MODEL_NAME, None) if exists else None)
    b.set_name_hidden(MODEL_NAME, True, None)


def all_tables(win):
    """{name: (sheet, (r1, c1, r2, c2), [headers])} of the workbook's tables."""
    b = win.book
    out = {}
    for s in range(b.sheet_count):
        if b.sheet_info(s)["name"] == MODEL_SHEET:
            continue
        for t in b.tables(s):
            r1, c1, r2, c2 = t["ref"]
            heads = [b.display(s, r1, c) for c in range(c1, c2 + 1)]
            out[t["name"]] = (s, (r1, c1, r2, c2), heads)
    return out


def check_relationship(win, rel):
    """None, or why the relationship can't be made (the related column must have unique values)."""
    tabs = all_tables(win)
    ft, fc, lt, lc = rel
    if ft not in tabs or lt not in tabs:
        return "Both tables must be in the workbook."
    if ft == lt:
        return "A table can't be related to itself."
    if fc not in tabs[ft][2] or lc not in tabs[lt][2]:
        return "Choose a column in each table."
    s, (r1, c1, r2, c2), heads = tabs[lt]
    k = c1 + heads.index(lc)
    seen = set()
    b = win.book
    for r in range(r1 + 1, r2 + 1):
        v = b.display(s, r, k).strip().lower()
        if v in seen:
            return ("The relationship cannot be created because each column contains duplicate values. "
                    "Select at least one column that contains only unique values.")
        seen.add(v)
    return None


def joined(win, fact, m=None):
    """(headers, rows) of the fact table with the columns of the tables it is related to
    (and theirs, in turn), looked up by the key columns."""
    m = m or load(win)
    tabs = all_tables(win)
    b = win.book
    if fact not in tabs:
        raise ValueError("The table %s isn't in the workbook." % fact)

    def rows_of(name):
        s, (r1, c1, r2, c2), heads = tabs[name]
        return heads, [[b.value(s, r, c) for c in range(c1, c2 + 1)] for r in range(r1 + 1, r2 + 1)]
    heads, rows = rows_of(fact)
    heads = ["%s.%s" % (fact, h) for h in heads]
    cols_of = {fact: {h.split(".", 1)[1]: k for k, h in enumerate(heads)}}
    done = {fact}
    queue = [fact]
    while queue:
        cur = queue.pop(0)
        for ft, fc, lt, lc in m["rels"]:
            if ft != cur or lt in done or lt not in tabs:
                continue
            lh, lrows = rows_of(lt)
            if lc not in lh or fc not in cols_of[cur]:
                continue
            key = lh.index(lc)
            index = {}
            for row in lrows:
                index.setdefault(str(row[key]).strip().lower(), row)
            base = len(heads)
            keep = [k for k in range(len(lh)) if k != key]
            heads += ["%s.%s" % (lt, lh[k]) for k in keep]
            fk = cols_of[cur][fc]
            for row in rows:
                hit = index.get(str(row[fk]).strip().lower())
                row.extend([hit[k] if hit else None for k in keep])
            cols_of[lt] = {lh[k]: base + j for j, k in enumerate(keep)}
            cols_of[lt][lc] = fk
            done.add(lt)
            queue.append(lt)
    return heads, rows


def model_sheet(win, create=True):
    b = win.book
    for s in range(b.sheet_count):
        if b.sheet_info(s)["name"] == MODEL_SHEET:
            return s
    if not create:
        return None
    s = b.insert_sheet(b.sheet_count, MODEL_SHEET)
    b.set_sheet_state(s, "veryHidden")
    return s


def build(win, fact):
    """Write the joined table to the hidden model sheet; returns (sheet, rect)."""
    b = win.book
    heads, rows = joined(win, fact)
    s = model_sheet(win)
    ext = b.used_extent(s)
    if ext:
        b.clear_contents(s, [(0, 0, ext[0], ext[1])], "Data Model")
    for c, h in enumerate(heads):
        b.set_input(s, 0, c, h)
    for r, row in enumerate(rows, start=1):
        for c, v in enumerate(row):
            if v is None or v == "":
                continue
            if isinstance(v, bool):
                t = "TRUE" if v else "FALSE"
            elif isinstance(v, float):
                t = "%.15g" % v
            else:
                t = str(v)
                if t[:1] in "=+-@" or (t[:1].isdigit() and not isinstance(v, (int, float))):
                    t = "'" + t
            b.set_input(s, r, c, t)
    m = load(win)
    m["fact"] = fact
    if fact not in m["tables"]:
        m["tables"].append(fact)
    save(win, m)
    return s, (0, 0, max(len(rows), 1), len(heads) - 1)


def rebuild(win):
    """Before a refresh: the model sheet follows its tables again."""
    m = load(win)
    if m.get("fact") and model_sheet(win, create=False) is not None:
        try:
            build(win, m["fact"])
        except ValueError:
            pass


def relationships(win):
    """Data > Relationships: Manage Relationships, with New, Edit and Delete."""
    m = load(win)
    tabs = all_tables(win)
    d = Gtk.Dialog(title="Manage Relationships", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("Close", Gtk.ResponseType.CLOSE)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    store = Gtk.ListStore(str, str, str, str)
    tv = Gtk.TreeView(model=store)
    for k, h in enumerate(("Table", "Column (Foreign)", "Related Table", "Related Column (Primary)")):
        tv.append_column(Gtk.TreeViewColumn(h, Gtk.CellRendererText(), text=k))
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(560, 160)
    sw.add(tv)
    row = Gtk.Box(spacing=8)
    row.pack_start(sw, True, True, 0)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    row.pack_start(side, False, False, 0)
    box.pack_start(row, True, True, 0)

    def fill():
        store.clear()
        for r in m["rels"]:
            store.append(list(r))

    def picked():
        mm, it = tv.get_selection().get_selected()
        return None if it is None else mm.get_path(it).get_indices()[0]

    def edit(k=None):
        cur = m["rels"][k] if k is not None else None
        res = relationship_dialog(win, tabs, cur)
        if res is None:
            return False
        err = check_relationship(win, res)
        if err:
            win.dialogs.message(err, "Create Relationship")
            return False
        if k is None:
            m["rels"].append(list(res))
        else:
            m["rels"][k] = list(res)
        for t in (res[0], res[2]):
            if t not in m["tables"]:
                m["tables"].append(t)
        save(win, m)
        fill()
        return True

    def delete(*_):
        k = picked()
        if k is not None:
            m["rels"].pop(k)
            save(win, m)
            fill()
    for label, fn in (("New...", lambda *_: edit()), ("Edit...", lambda *_: picked() is not None and edit(picked())),
                      ("Delete", delete)):
        bt = Gtk.Button(label=label)
        bt.connect("clicked", fn)
        side.pack_start(bt, False, False, 0)
    d.new, d.edit, d.delete, d.store = edit, edit, delete, store
    fill()
    d.show_all()
    d.run()
    d.destroy()
    win.update_title()
    return m


def relationship_dialog(win, tabs, cur=None):
    """Create Relationship: table and column, related table and related column."""
    d = Gtk.Dialog(title="Create Relationship" if cur is None else "Edit Relationship", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    grid = Gtk.Grid(column_spacing=12, row_spacing=6)
    combos = []
    for k, lab in enumerate(("Table:", "Column (Foreign):", "Related Table:", "Related Column (Primary):")):
        grid.attach(Gtk.Label(label=lab, xalign=0), k % 2, (k // 2) * 2, 1, 1)
        cb = Gtk.ComboBoxText()
        cb.set_hexpand(True)
        grid.attach(cb, k % 2, (k // 2) * 2 + 1, 1, 1)
        combos.append(cb)
    for t in sorted(tabs):
        combos[0].append(t, t)
        combos[2].append(t, t)

    def cols(tcb, ccb):
        ccb.remove_all()
        t = tcb.get_active_id()
        for h in (tabs[t][2] if t in tabs else []):
            ccb.append(h, h)
        ccb.set_active(0)
    combos[0].connect("changed", lambda *_: cols(combos[0], combos[1]))
    combos[2].connect("changed", lambda *_: cols(combos[2], combos[3]))
    if cur:
        for cb, v in zip(combos, cur):
            cb.set_active_id(v)
    else:
        combos[0].set_active(0)
        combos[2].set_active(1 if len(tabs) > 1 else 0)
    box.pack_start(grid, False, False, 0)
    d.combos = combos
    d.show_all()
    r = d.run()
    vals = [cb.get_active_id() for cb in combos]
    d.destroy()
    if r != Gtk.ResponseType.OK or None in vals:
        return None
    return tuple(vals)
