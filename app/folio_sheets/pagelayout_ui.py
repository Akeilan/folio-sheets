"""View > Page Layout: the header and footer typed on the page, and the ruler."""
import re

from gi.repository import Gdk, Gtk

SECTIONS = ("&L", "&C", "&R")


def split_code(code):
    """'&Lleft&Ccentre&Rright' -> [left, centre, right] (codes inside kept)."""
    out = ["", "", ""]
    cur = 1 if code and not re.match(r"^&[LCR]", code) else None
    i = 0
    buf = ""
    while i < len(code):
        if code[i] == "&" and i + 1 < len(code) and code[i + 1] in "LCR":
            if cur is not None:
                out[cur] += buf
            buf = ""
            cur = "LCR".index(code[i + 1])
            i += 2
            continue
        if code[i] == "&" and i + 1 < len(code) and code[i + 1] == "&":
            buf += "&&"
            i += 2
            continue
        buf += code[i]
        i += 1
    if cur is not None:
        out[cur] += buf
    return out


def join_code(parts):
    return "".join(SECTIONS[k] + t for k, t in enumerate(parts) if t)


def edit_on_page(win, kind, section, ev=None):
    """A click in a page's header or footer: type that section's text there."""
    b, g = win.book, win.grid
    s = g.sheet
    ps = b.page_setup(s)
    parts = split_code(ps.get(kind, ""))
    pop = Gtk.Popover()
    pop.set_relative_to(g.area)
    if ev is not None:
        r = Gdk.Rectangle()
        r.x, r.y, r.width, r.height = int(ev.x), int(ev.y), 1, 1
        pop.set_pointing_to(r)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(6)
    box.pack_start(Gtk.Label(label="%s %s" % (("Left", "Center", "Right")[section], "Header" if kind == "header" else "Footer"),
                             xalign=0), False, False, 0)
    e = Gtk.Entry(text=parts[section])
    e.set_width_chars(28)
    box.pack_start(e, False, False, 0)
    tools = Gtk.Box(spacing=2)
    for label, code in (("Page Number", "&P"), ("Number of Pages", "&N"), ("Current Date", "&D"),
                        ("Current Time", "&T"), ("Sheet Name", "&A"), ("File Name", "&F")):
        bt = Gtk.Button(label=label)
        bt.set_relief(Gtk.ReliefStyle.NONE)
        bt.connect("clicked", lambda _b, c=code: (e.insert_text(c, e.get_position()), e.set_position(e.get_position() + len(c))))
        tools.pack_start(bt, False, False, 0)
    box.pack_start(tools, False, False, 0)
    pop.add(box)

    def done(*_):
        new = list(parts)
        new[section] = e.get_text()
        code = join_code(new)
        if code != ps.get(kind, ""):
            spec = dict(ps)
            spec[kind] = code
            b.set_page_setup(s, spec, "Header" if kind == "header" else "Footer")
            win.update_title()
        pop.popdown()
        g.refresh()
    e.connect("activate", done)
    pop.connect("closed", lambda *_: None)
    pop.entry, pop.apply = e, done
    pop.show_all()
    pop.popup()
    e.grab_focus()
    win.hf_popover = pop
    return pop
