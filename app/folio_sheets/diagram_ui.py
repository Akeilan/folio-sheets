"""Cork diagrams in Sheet (K3-08): double-click opens one in Folio Cork, each save there updates
it here. The format lives in corkembed."""
from . import corkembed

_open = {}


def _find(b, s, data):
    """Index of the picture on sheet s holding exactly these bytes."""
    for k, o in enumerate(b.objects(s)):
        if o["kind"] == "picture":
            got = b.picture_bytes(s, k)
            if got and bytes(got[0]) == data:
                return k
    return None


def edit(win, i):
    g, b = win.grid, win.book
    s = g.sheet
    got = b.picture_bytes(s, i)
    data = bytes(got[0]) if got else b""
    if not corkembed.is_diagram(data):
        return None
    state = {"data": data, "size": corkembed.png_size(data)}
    key = (id(b), s, data)
    old = _open.get(key)
    if old is not None and old.running():
        return old

    def changed(png):
        if win.book is not b or s >= b.sheet_count:
            return
        k = _find(b, s, state["data"])
        if k is None:
            return
        x, y, w, h = b.objects(s)[k]["rect"]
        new = corkembed.png_size(png)
        if new and state["size"]:
            w, h = new[0] * w / max(1, state["size"][0]), new[1] * h / max(1, state["size"][1])
        props = {kk: b.objects(s)[k].get(kk) for kk in ("name", "descr") if b.objects(s)[k].get(kk)}
        b.begin_group("Edit Diagram")
        try:
            b.delete_objects(s, [k])
            n = b.insert_picture(s, png, x, y, w, h)
            b.set_object_rect(s, n, x, y, w, h, "Edit Diagram")
            if props:
                b.set_object_props(s, [n], props, "Edit Diagram")
        finally:
            b.end_group()
        state["data"], state["size"] = png, new
        if g.sheet == s:
            g.sel_objects = [n]
        g.refresh()
        win.update_title()

    ed = corkembed.edit(data, changed, lambda t: win.dialogs.message(t, "Edit Diagram"),
                        on_close=lambda: _open.pop(key, None))
    if ed is not None:
        _open[key] = ed
    return ed
