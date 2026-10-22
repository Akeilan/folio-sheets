"""Cell formatting, done on the workbook file itself.

The engine draws every kind of cell formatting it reads from a file (fonts, fills,
borders, alignment, merges) but only exposes number formats for changing. So a
formatting command takes the workbook as the engine saves it, edits the styles and
the sheet parts here, and hands the result back to the engine (see history.py for
how that stays undoable).

Only the parts that change are rewritten; everything else in the package is copied
byte for byte, and the root tags keep their original namespace declarations.
"""
import copy
import io
import re
import zipfile
import xml.etree.ElementTree as ET

MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL = "http://schemas.openxmlformats.org/package/2006/relationships"
M = "{%s}" % MAIN

MAX_ROWS = 1048576
MAX_COLS = 16384
CELL_LIMIT = 2_000_000  # cells a single command may create

# styleSheet children, in schema order
STYLE_ORDER = ["numFmts", "fonts", "fills", "borders", "cellStyleXfs", "cellXfs", "cellStyles", "dxfs",
               "tableStyles", "colors", "extLst"]
# worksheet children, in schema order
SHEET_ORDER = ["sheetPr", "dimension", "sheetViews", "sheetFormatPr", "cols", "sheetData", "sheetCalcPr",
               "sheetProtection", "protectedRanges", "scenarios", "autoFilter", "sortState", "dataConsolidate",
               "customSheetViews", "mergeCells", "phoneticPr", "conditionalFormatting", "dataValidations",
               "hyperlinks", "printOptions", "pageMargins", "pageSetup", "headerFooter", "rowBreaks", "colBreaks",
               "customProperties", "cellWatches", "ignoredErrors", "smartTags", "drawing", "legacyDrawing",
               "legacyDrawingHF", "drawingHF", "picture", "oleObjects", "controls", "webPublishItems",
               "tableParts", "extLst"]
BORDER_SIDES = ["left", "right", "top", "bottom", "diagonal"]
BORDER_ORDER = ["start", "left", "end", "right", "top", "bottom", "diagonal", "vertical", "horizontal"]

BUILTIN_FORMATS = {
    0: "General", 1: "0", 2: "0.00", 3: "#,##0", 4: "#,##0.00", 9: "0%", 10: "0.00%", 11: "0.00E+00",
    12: "# ?/?", 13: "# ??/??", 14: "m/d/yyyy", 15: "d-mmm-yy", 16: "d-mmm", 17: "mmm-yy", 18: "h:mm AM/PM",
    19: "h:mm:ss AM/PM", 20: "h:mm", 21: "h:mm:ss", 22: "m/d/yyyy h:mm", 37: "#,##0 ;(#,##0)",
    38: "#,##0 ;[Red](#,##0)", 39: "#,##0.00;(#,##0.00)", 40: "#,##0.00;[Red](#,##0.00)",
    41: '_(* #,##0_);_(* \\(#,##0\\);_(* "-"_);_(@_)',
    42: '_("$"* #,##0_);_("$"* \\(#,##0\\);_("$"* "-"_);_(@_)',
    43: '_(* #,##0.00_);_(* \\(#,##0.00\\);_(* "-"??_);_(@_)',
    44: '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)',
    45: "mm:ss", 46: "[h]:mm:ss", 47: "mmss.0", 48: "##0.0E+0", 49: "@",
}
BUILTIN_IDS = {v: k for k, v in BUILTIN_FORMATS.items()}


# --------------------------------------------------------------------------- cell references

def col_letters(c):
    s = ""
    c += 1
    while c:
        c, r = divmod(c - 1, 26)
        s = chr(65 + r) + s
    return s


_REF = re.compile(r"^([A-Za-z]+)(\d+)$")


def parse_cell(ref):
    m = _REF.match(ref)
    col = 0
    for ch in m.group(1).upper():
        col = col * 26 + ord(ch) - 64
    return int(m.group(2)) - 1, col - 1


def cell_ref(r, c):
    return "%s%d" % (col_letters(c), r + 1)


def range_ref(rg):
    r1, c1, r2, c2 = rg
    if (r1, c1) == (r2, c2):
        return cell_ref(r1, c1)
    return "%s:%s" % (cell_ref(r1, c1), cell_ref(r2, c2))


def parse_range(ref):
    a, _, b = ref.partition(":")
    r1, c1 = parse_cell(a)
    r2, c2 = parse_cell(b) if b else (r1, c1)
    return min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2)


def whole_cols(rg):
    return rg[0] == 0 and rg[2] >= MAX_ROWS - 1


def whole_rows(rg):
    return rg[1] == 0 and rg[3] >= MAX_COLS - 1


# --------------------------------------------------------------------------- XML plumbing

# the usual prefixes, so namespaces declared deep inside a part keep their familiar names
for _p, _u in (("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
               ("x14ac", "http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"),
               ("x14", "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"),
               ("x15", "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main"),
               ("x15ac", "http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac"),
               ("xm", "http://schemas.microsoft.com/office/excel/2006/main"),
               ("xr", "http://schemas.microsoft.com/office/spreadsheetml/2014/revision"),
               ("xr2", "http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"),
               ("xr3", "http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"),
               ("x16r2", "http://schemas.microsoft.com/office/spreadsheetml/2015/02/main"),
               ("r", REL)):
    ET.register_namespace(_p, _u)

_ROW = re.compile(rb"<row\b[^>]*?(?:/>|>.*?</row>)", re.S)
_ROW_R = re.compile(rb'<row\b[^>]*?\sr="(\d+)"')
_CELL_REF = re.compile(rb'<c\b[^>]*?\sr="([A-Z]+)\d+"')
_NSDECL = re.compile(rb'\sxmlns(?::([\w.-]+))?\s*=\s*"([^"]*)"')
_ROOT = re.compile(rb"<(?![?!])[^>]*>")


class Part:
    """One XML part, parsed, that can be written back with its original root tag."""

    def __init__(self, data, lazy=None):
        """lazy: the local name of a big child (sheetData) whose children are kept as raw
        byte chunks, parsed only when something asks for them."""
        self.raw = data
        m = _ROOT.search(data)
        self.prolog = data[:m.start()]
        self.root_tag = m.group(0)
        self.decls = {(p or b"").decode(): u.decode() for p, u in _NSDECL.findall(self.root_tag)}
        for p, u in self.decls.items():
            if p:
                ET.register_namespace(p, u)
        self.lazy_inner = None
        self.lazy_emit = None
        if lazy:
            o = re.search(rb"<%s\b[^>]*?(/?)>" % lazy.encode(), data)
            if o is not None and not o.group(1):
                c = data.find(b"</%s>" % lazy.encode(), o.end())
                if c > 0:
                    self.lazy_inner = data[o.end():c]
                    data = data[:o.end()] + data[c:]
            elif o is not None:
                self.lazy_inner = b""
        self.root = ET.fromstring(data)

    def wrap_decls(self):
        """Namespace declarations for parsing a fragment on its own."""
        return "".join(' xmlns%s="%s"' % ((":" + p) if p else "", _esc_attr(u)) for p, u in self.decls.items())

    def to_bytes(self):
        """Write the tree back. The main namespace stays the default one (unprefixed),
        exactly as the engine and other readers expect, and the original root tag is kept."""
        prefixes = {u: p for p, u in self.decls.items()}
        extra = {}

        def qn(name):
            if name[0] != "{":
                return name
            uri, local = name[1:].split("}", 1)
            p = prefixes.get(uri)
            if p is None:
                p = ET._namespace_map.get(uri) or "ns%d" % len(extra)
                prefixes[uri] = p
                extra[p] = uri
            return local if p == "" else "%s:%s" % (p, local)

        out = []
        w = out.append

        def emit(el):
            tag = qn(el.tag)
            w("<" + tag)
            for k, v in el.attrib.items():
                w(' %s="%s"' % (qn(k), _esc_attr(v)))
            if self.lazy_emit is not None and el is self.lazy_emit[0]:
                w(">")
                self.lazy_emit[1](emit, w)
                w("</%s>" % tag)
                return
            if len(el) or el.text:
                w(">")
                if el.text:
                    w(_esc(el.text))
                for ch in el:
                    emit(ch)
                    if ch.tail:
                        w(_esc(ch.tail))
                w("</%s>" % tag)
            else:
                w("/>")

        root = self.root
        if root.text:
            w(_esc(root.text))
        for ch in root:
            emit(ch)
            if ch.tail:
                w(_esc(ch.tail))
        tag = self.root_tag
        if extra:
            decl = "".join(' xmlns:%s="%s"' % (p, _esc_attr(u)) for p, u in extra.items()).encode()
            tag = tag[:-2] + decl + b"/>" if tag.endswith(b"/>") else tag[:-1] + decl + b">"
        if tag.endswith(b"/>"):
            tag = tag[:-2] + b">"
        local = root.tag.split("}", 1)[-1]
        close = ("</%s>" % (qn(root.tag) if root.tag[0] == "{" else local)).encode()
        return self.prolog + tag + "".join(out).encode("utf-8") + close


def _esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _esc_attr(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace('"', "&quot;").replace("\n", "&#10;") \
        .replace("\r", "&#13;").replace("\t", "&#9;")


def _child(parent, name, order, create=True):
    el = parent.find(M + name)
    if el is not None or not create:
        return el
    el = ET.Element(M + name)
    idx = order.index(name)
    pos = 0
    for i, ch in enumerate(list(parent)):
        tag = ch.tag.split("}", 1)[-1]
        if tag in order and order.index(tag) < idx:
            pos = i + 1
    parent.insert(pos, el)
    return el


def _key(el):
    return ET.tostring(el)


def _rgb(c):
    """int 0xRRGGBB or 'RRGGBB' -> 'FFRRGGBB'."""
    if isinstance(c, int):
        return "FF%06X" % c
    c = c.lstrip("#").upper()
    return c if len(c) == 8 else "FF" + c


def _color_int(el):
    if el is None:
        return None
    rgb = el.get("rgb")
    if rgb:
        return int(rgb[-6:], 16)
    if el.get("auto") == "1":
        return None
    if el.get("theme") is not None:
        return ("theme", int(el.get("theme")), float(el.get("tint", "0")))
    if el.get("indexed") is not None:
        return ("indexed", int(el.get("indexed")))
    return None


# --------------------------------------------------------------------------- styles

class Styles:
    def __init__(self, part):
        self.part = part
        s = part.root
        self.numfmts = _child(s, "numFmts", STYLE_ORDER)
        self.fonts = _child(s, "fonts", STYLE_ORDER)
        self.fills = _child(s, "fills", STYLE_ORDER)
        self.borders = _child(s, "borders", STYLE_ORDER)
        self.xfs = _child(s, "cellXfs", STYLE_ORDER)
        if len(self.fonts) == 0:
            f = ET.SubElement(self.fonts, M + "font")
            ET.SubElement(f, M + "sz", val="11")
            ET.SubElement(f, M + "name", val="Calibri")
        if len(self.fills) == 0:
            ET.SubElement(ET.SubElement(self.fills, M + "fill"), M + "patternFill", patternType="none")
            ET.SubElement(ET.SubElement(self.fills, M + "fill"), M + "patternFill", patternType="gray125")
        if len(self.borders) == 0:
            b = ET.SubElement(self.borders, M + "border")
            for side in BORDER_SIDES:
                ET.SubElement(b, M + side)
        if len(self.xfs) == 0:
            ET.SubElement(self.xfs, M + "xf", numFmtId="0", fontId="0", fillId="0", borderId="0", xfId="0")
        self._index = {}
        for name, coll in (("font", self.fonts), ("fill", self.fills), ("border", self.borders), ("xf", self.xfs)):
            d = self._index[name] = {}
            for i, el in enumerate(coll):
                d.setdefault(_key(el), i)
        self.changed = False
        self._memo = {}

    def count(self):
        return len(self.xfs)

    # ---- reading
    def _get(self, coll, i):
        i = int(i or 0)
        return coll[i] if 0 <= i < len(coll) else coll[0]

    def numfmt_code(self, nid):
        nid = int(nid or 0)
        for nf in self.numfmts:
            if int(nf.get("numFmtId", -1)) == nid:
                return nf.get("formatCode", "General")
        return BUILTIN_FORMATS.get(nid, "General")

    def spec(self, xi):
        """Everything Format Cells shows for cellXfs[xi], as plain values."""
        xf = self._get(self.xfs, xi)
        f = self._get(self.fonts, xf.get("fontId"))

        def flag(tag):
            e = f.find(M + tag)
            return e is not None and e.get("val", "1") not in ("0", "false")
        u = f.find(M + "u")
        va = f.find(M + "vertAlign")
        font = {
            "name": (f.find(M + "name").get("val") if f.find(M + "name") is not None else "Calibri"),
            "size": float(f.find(M + "sz").get("val")) if f.find(M + "sz") is not None else 11.0,
            "bold": flag("b"), "italic": flag("i"), "strike": flag("strike"),
            "underline": (u.get("val", "single") if u is not None and u.get("val") != "none" else None),
            "color": _color_int(f.find(M + "color")),
            "vert": va.get("val") if va is not None and va.get("val") != "baseline" else None,
        }
        fl = self._get(self.fills, xf.get("fillId")).find(M + "patternFill")
        fill = {"pattern": "none", "fg": None, "bg": None}
        if fl is not None:
            fill = {"pattern": fl.get("patternType", "solid" if fl.find(M + "fgColor") is not None else "none"),
                    "fg": _color_int(fl.find(M + "fgColor")), "bg": _color_int(fl.find(M + "bgColor"))}
        b = self._get(self.borders, xf.get("borderId"))
        border = {}
        for side in BORDER_SIDES:
            e = b.find(M + side)
            if e is None and side in ("left", "right"):
                e = b.find(M + ("start" if side == "left" else "end"))
            if e is not None and e.get("style") and e.get("style") != "none":
                border[side] = (e.get("style"), _color_int(e.find(M + "color")))
        border["diagonalUp"] = b.get("diagonalUp") == "1"
        border["diagonalDown"] = b.get("diagonalDown") == "1"
        al = xf.find(M + "alignment")
        a = dict(al.attrib) if al is not None else {}
        align = {"horizontal": a.get("horizontal", "general"), "vertical": a.get("vertical", "bottom"),
                 "wrap": a.get("wrapText") in ("1", "true"), "shrink": a.get("shrinkToFit") in ("1", "true"),
                 "indent": int(a.get("indent", 0)), "rotation": int(a.get("textRotation", 0)),
                 "reading": int(a.get("readingOrder", 0))}
        pr = xf.find(M + "protection")
        prot = {"locked": not (pr is not None and pr.get("locked") in ("0", "false")),
                "hidden": pr is not None and pr.get("hidden") in ("1", "true")}
        return {"font": font, "fill": fill, "border": border, "align": align, "protection": prot,
                "numfmt": self.numfmt_code(xf.get("numFmtId"))}

    # ---- writing
    def _intern(self, kind, coll, el):
        k = _key(el)
        d = self._index[kind]
        if k in d:
            return d[k]
        coll.append(el)
        d[k] = len(coll) - 1
        coll.set("count", str(len(coll)))
        self.changed = True
        return len(coll) - 1

    def _numfmt_id(self, code):
        if code in BUILTIN_IDS:
            return BUILTIN_IDS[code]
        used = set()
        for nf in self.numfmts:
            if nf.get("formatCode") == code:
                return int(nf.get("numFmtId"))
            used.add(int(nf.get("numFmtId", 0)))
        nid = max([163] + list(used)) + 1
        ET.SubElement(self.numfmts, M + "numFmt", numFmtId=str(nid), formatCode=code)
        self.numfmts.set("count", str(len(self.numfmts)))
        self.changed = True
        return nid

    def derive(self, xi, ch):
        """cellXfs[xi] with the changes in ch applied; returns the (possibly new) index.

        ch keys: font {name,size,bold,italic,underline,strike,color,vert,grow},
        fill {pattern,fg,bg} or None, border {side: (style, color) | None, diagonalUp, diagonalDown},
        align {horizontal,vertical,wrap,shrink,indent,rotation,reading,indent_step},
        protection {locked,hidden}, numfmt code, reset True (start from the default style)."""
        mk = (xi, repr(sorted(ch.items())))
        if mk in self._memo:
            return self._memo[mk]
        xi = int(xi or 0)
        base = self.xfs[0] if ch.get("reset") else self._get(self.xfs, xi)
        xf = copy.deepcopy(base)
        if "font" in ch:
            f = copy.deepcopy(self._get(self.fonts, xf.get("fontId")))
            self._edit_font(f, ch["font"])
            xf.set("fontId", str(self._intern("font", self.fonts, f)))
            xf.set("applyFont", "1")
        if "fill" in ch:
            fill = ET.Element(M + "fill")
            v = ch["fill"]
            if not v or v.get("pattern", "solid") == "none" or (v.get("fg") is None and v.get("pattern", "solid") == "solid"):
                ET.SubElement(fill, M + "patternFill", patternType="none")
            else:
                pf = ET.SubElement(fill, M + "patternFill", patternType=v.get("pattern", "solid"))
                if v.get("fg") is not None:
                    ET.SubElement(pf, M + "fgColor", rgb=_rgb(v["fg"]))
                if v.get("bg") is not None:
                    ET.SubElement(pf, M + "bgColor", rgb=_rgb(v["bg"]))
                else:
                    ET.SubElement(pf, M + "bgColor", indexed="64")
            fid = self._intern("fill", self.fills, fill)
            xf.set("fillId", str(fid))
            xf.set("applyFill", "1")
        if "border" in ch:
            b = copy.deepcopy(self._get(self.borders, xf.get("borderId")))
            self._edit_border(b, ch["border"])
            xf.set("borderId", str(self._intern("border", self.borders, b)))
            xf.set("applyBorder", "1")
        if "align" in ch:
            al = xf.find(M + "alignment")
            if al is None:
                al = ET.Element(M + "alignment")
                xf.insert(0, al)
            self._edit_align(al, ch["align"])
            if not al.attrib:
                xf.remove(al)
            xf.set("applyAlignment", "1")
        if "protection" in ch:
            pr = xf.find(M + "protection")
            if pr is None:
                pr = ET.Element(M + "protection")
                al = xf.find(M + "alignment")
                xf.insert(1 if al is not None else 0, pr)
            for k in ("locked", "hidden"):
                if k in ch["protection"]:
                    v = bool(ch["protection"][k])
                    if (k == "locked" and v) or (k == "hidden" and not v):
                        pr.attrib.pop(k, None)
                    else:
                        pr.set(k, "1" if v else "0")
            if not pr.attrib:
                xf.remove(pr)
            xf.set("applyProtection", "1")
        if "numfmt" in ch:
            xf.set("numFmtId", str(self._numfmt_id(ch["numfmt"])))
            xf.set("applyNumberFormat", "1")
        out = self._intern("xf", self.xfs, xf)
        self._memo[mk] = out
        return out

    @staticmethod
    def _set_flag(f, tag, on):
        e = f.find(M + tag)
        if on:
            if e is None:
                f.insert(0, ET.Element(M + tag))
            else:
                e.attrib.pop("val", None)
        elif e is not None:
            f.remove(e)

    def _edit_font(self, f, d):
        for k, tag in (("bold", "b"), ("italic", "i"), ("strike", "strike")):
            if k in d:
                self._set_flag(f, tag, d[k])
        if "underline" in d:
            e = f.find(M + "u")
            if e is not None:
                f.remove(e)
            if d["underline"]:
                u = ET.Element(M + "u")
                if d["underline"] != "single":
                    u.set("val", d["underline"])
                f.insert(0, u)
        if "vert" in d:
            e = f.find(M + "vertAlign")
            if e is not None:
                f.remove(e)
            if d["vert"]:
                f.insert(0, ET.Element(M + "vertAlign", val=d["vert"]))
        if "size" in d or "grow" in d:
            e = f.find(M + "sz")
            if e is None:
                e = ET.SubElement(f, M + "sz", val="11")
            size = float(e.get("val", 11))
            if "size" in d:
                size = float(d["size"])
            if "grow" in d:
                size = _step_size(size, d["grow"])
            e.set("val", ("%g" % size))
        if "name" in d and d["name"]:
            e = f.find(M + "name")
            if e is None:
                e = ET.SubElement(f, M + "name")
            e.set("val", d["name"])
            # a theme font slot would override the explicit name
            for tag in ("scheme",):
                s = f.find(M + tag)
                if s is not None:
                    f.remove(s)
        if "color" in d:
            e = f.find(M + "color")
            if e is not None:
                f.remove(e)
            if d["color"] is not None:
                f.append(ET.Element(M + "color", rgb=_rgb(d["color"])))

    def _edit_border(self, b, d):
        sides = {}
        for el in list(b):
            tag = el.tag.split("}", 1)[-1]
            tag = {"start": "left", "end": "right"}.get(tag, tag)
            sides[tag] = el
            b.remove(el)
        for side, v in d.items():
            if side in ("diagonalUp", "diagonalDown"):
                if v:
                    b.set(side, "1")
                else:
                    b.attrib.pop(side, None)
                continue
            el = ET.Element(M + side)
            if v is not None:
                style, color = v
                if style and style != "none":
                    el.set("style", style)
                    ce = ET.SubElement(el, M + "color")
                    if color is None:
                        ce.set("indexed", "64")
                    else:
                        ce.set("rgb", _rgb(color))
            sides[side] = el
        for side in BORDER_SIDES:
            if side not in sides:
                sides[side] = ET.Element(M + side)
        for side in BORDER_ORDER:
            if side in sides:
                b.append(sides[side])

    @staticmethod
    def _edit_align(al, d):
        def put(attr, val, default):
            if val == default or val is None:
                al.attrib.pop(attr, None)
            else:
                al.set(attr, str(val))
        if "horizontal" in d:
            put("horizontal", d["horizontal"], "general")
        if "vertical" in d:
            put("vertical", d["vertical"], "bottom")
        if "wrap" in d:
            put("wrapText", "1" if d["wrap"] else None, None)
        if "shrink" in d:
            put("shrinkToFit", "1" if d["shrink"] else None, None)
        if "rotation" in d:
            put("textRotation", int(d["rotation"]), 0)
        if "reading" in d:
            put("readingOrder", int(d["reading"]), 0)
        indent = int(al.get("indent", 0))
        if "indent" in d:
            indent = int(d["indent"])
        if "indent_step" in d:
            indent = max(0, min(250, indent + d["indent_step"]))
            if indent and al.get("horizontal") not in ("left", "right", "distributed"):
                al.set("horizontal", "left")
        put("indent", indent, 0)


FONT_STEPS = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72]


def _step_size(size, d):
    if d > 0:
        bigger = [s for s in FONT_STEPS if s > size]
        return bigger[0] if bigger else min(409, size + 10)
    smaller = [s for s in FONT_STEPS if s < size]
    return smaller[-1] if smaller else max(1, size - 1)


# --------------------------------------------------------------------------- one worksheet

class Sheet:
    def __init__(self, part, styles):
        self.part = part
        self.styles = styles
        ws = part.root
        self.ws = ws
        self.data = _child(ws, "sheetData", SHEET_ORDER)
        self.rows = {}   # r -> parsed <row> element
        self.raw = {}    # r -> untouched <row> bytes
        if part.lazy_inner is not None:
            for m in _ROW.finditer(part.lazy_inner):
                chunk = m.group(0)
                r = int(_ROW_R.search(chunk).group(1)) - 1
                self.raw[r] = chunk
        for row in list(self.data):
            r = int(row.get("r")) - 1
            self.rows[r] = row
            self.data.remove(row)
        self.row_keys = sorted(set(self.rows) | set(self.raw))
        part.lazy_emit = (self.data, self._emit_rows)
        self._wrap = None
        self._cells = {}  # r -> {c: el}
        self._cols = None
        self.changed = False

    def _emit_rows(self, emit, w):
        for r in self.row_keys:
            el = self.rows.get(r)
            if el is not None:
                emit(el)
            else:
                w(self.raw[r].decode("utf-8"))

    def _parse_row(self, r):
        if self._wrap is None:
            self._wrap = "<x%s>" % self.part.wrap_decls()
        el = ET.fromstring((self._wrap + self.raw.pop(r).decode("utf-8") + "</x>").encode("utf-8"))[0]
        self.rows[r] = el
        return el

    def _row_el(self, r):
        el = self.rows.get(r)
        if el is None and r in self.raw:
            el = self._parse_row(r)
        return el

    def row_attr(self, r, name, default=None):
        el = self.rows.get(r)
        if el is not None:
            return el.get(name, default)
        chunk = self.raw.get(r)
        if chunk is None:
            return default
        head = chunk[:chunk.index(b">") + 1]
        m = re.search(rb'\s%s="([^"]*)"' % name.encode(), head)
        return m.group(1).decode() if m else default

    def row_styled(self, r):
        return self.row_attr(r, "customFormat") in ("1", "true")

    def raw_cols(self, r):
        """Columns that have cells in a row, without parsing it."""
        el = self.rows.get(r)
        if el is not None:
            return set(self.cells(r))
        chunk = self.raw.get(r)
        if chunk is None:
            return set()
        out = set()
        for m in _CELL_REF.finditer(chunk):
            col = 0
            for ch in m.group(1):
                col = col * 26 + ch - 64
            out.add(col - 1)
        return out

    # ---- rows and cells
    def cells(self, r):
        d = self._cells.get(r)
        if d is None:
            d = {}
            row = self._row_el(r)
            if row is not None:
                for c in row.findall(M + "c"):
                    d[parse_cell(c.get("r"))[1]] = c
            self._cells[r] = d
        return d

    def row(self, r, create=True):
        row = self._row_el(r)
        if row is None and create:
            import bisect
            row = ET.Element(M + "row", r=str(r + 1))
            i = bisect.bisect_left(self.row_keys, r)
            self.row_keys.insert(i, r)
            self.rows[r] = row
            self._cells.setdefault(r, {})
        return row

    def cell(self, r, c, create=True):
        d = self.cells(r)
        el = d.get(c)
        if el is None and create:
            row = self.row(r)
            el = ET.Element(M + "c", r=cell_ref(r, c))
            inherit = self.empty_xf(r, c)
            if inherit:
                el.set("s", str(inherit))
            after = [k for k in d if k < c]
            if after:
                pos = list(row).index(d[max(after)]) + 1
            else:
                pos = 0
            row.insert(pos, el)
            row.attrib.pop("spans", None)
            d[c] = el
        return el

    def cell_xf(self, r, c):
        if r in self.raw and c not in self.raw_cols(r):
            return self.empty_xf(r, c)
        el = self.cells(r).get(c)
        if el is not None:
            return int(el.get("s", 0))
        return self.empty_xf(r, c)

    def empty_xf(self, r, c):
        if self.row_styled(r):
            return int(self.row_attr(r, "s", 0))
        return self.col_xf(c)

    def set_cell_xf(self, r, c, xi):
        el = self.cell(r, c)
        if xi:
            el.set("s", str(xi))
        else:
            el.attrib.pop("s", None)
        self.changed = True

    # ---- columns
    def _load_cols(self):
        if self._cols is None:
            self._cols = []  # [min, max, attrib] in order
            cols = self.ws.find(M + "cols")
            if cols is not None:
                for col in cols:
                    self._cols.append([int(col.get("min")) - 1, int(col.get("max")) - 1, dict(col.attrib)])
        return self._cols

    def col_xf(self, c):
        for a, b, at in self._load_cols():
            if a <= c <= b:
                return int(at.get("style", 0))
        return 0

    def map_cols(self, c1, c2, fn):
        """fn(col_index, style) -> new style, for every column c1..c2 (spans kept compact)."""
        spans = self._load_cols()
        pieces = []
        cur = c1
        for a, b, at in spans:
            if b < c1 or a > c2:
                continue
            if a > cur:
                pieces.append((cur, a - 1, None))
            pieces.append((max(a, c1), min(b, c2), at))
            cur = min(b, c2) + 1
        if cur <= c2:
            pieces.append((cur, c2, None))
        new = []
        for a, b, at in spans:  # parts of existing spans outside c1..c2 survive unchanged
            if b < c1 or a > c2:
                new.append([a, b, at])
            else:
                if a < c1:
                    new.append([a, c1 - 1, dict(at)])
                if b > c2:
                    new.append([c2 + 1, b, dict(at)])
        for a, b, at in pieces:
            base = dict(at) if at else {}
            # style may differ per column only when fn depends on the column; group runs
            run_start, run_style = a, None
            for c in range(a, b + 2):
                s = fn(c, int(base.get("style", 0))) if c <= b else None
                if c == a:
                    run_style = s
                    continue
                if s != run_style or c > b:
                    d = dict(base)
                    if run_style:
                        d["style"] = str(run_style)
                    else:
                        d.pop("style", None)
                    if at is None:
                        d.setdefault("width", "9.140625")
                    new.append([run_start, c - 1, d])
                    run_start, run_style = c, s
        new.sort(key=lambda x: x[0])
        # merge neighbours with identical attributes
        merged = []
        for a, b, at in new:
            if at is None:
                continue
            if merged and merged[-1][1] + 1 == a and merged[-1][2] == at:
                merged[-1][1] = b
            else:
                merged.append([a, b, at])
        self._cols = merged
        cols = _child(self.ws, "cols", SHEET_ORDER)
        for el in list(cols):
            cols.remove(el)
        for a, b, at in merged:
            at = dict(at)
            at["min"], at["max"] = str(a + 1), str(b + 1)
            el = ET.SubElement(cols, M + "col")
            for k in ("min", "max", "width", "style", "hidden", "bestFit", "customWidth", "phonetic",
                      "outlineLevel", "collapsed"):
                if k in at:
                    el.set(k, at.pop(k))
            for k, v in at.items():
                el.set(k, v)
        if len(cols) == 0:
            self.ws.remove(cols)
        self.changed = True

    # ---- applying a style change over selections
    def apply(self, ranges, fn, positional=False):
        """fn(xf, r, c, rg) -> new xf. Whole columns and rows are handled the way the
        original does: column and row styles change, existing cells change, and only the
        intersections that would otherwise show a different style get cells."""
        created = 0
        for rg in ranges:
            r1, c1, r2, c2 = rg
            r2, c2 = min(r2, MAX_ROWS - 1), min(c2, MAX_COLS - 1)
            rg = (r1, c1, r2, c2)
            if whole_cols(rg) and whole_rows(rg):
                # the whole sheet: every column style, every styled row, every existing cell
                self.map_cols(0, MAX_COLS - 1, lambda c, s: fn(s, None, c, rg))
                for r in list(self.row_keys):
                    row = self.row(r)
                    if row.get("customFormat") in ("1", "true"):
                        row.set("s", str(fn(int(row.get("s", 0)), r, None, rg)))
                    for c, el in list(self.cells(r).items()):
                        self.set_cell_xf(r, c, fn(int(el.get("s", 0)), r, c, rg))
            elif whole_cols(rg):
                self.map_cols(c1, c2, lambda c, s: fn(s, None, c, rg))
                for r in list(self.row_keys):
                    if self.row_styled(r):
                        cols = range(c1, c2 + 1)
                    else:
                        cols = [c for c in self.raw_cols(r) if c1 <= c <= c2]
                    for c in cols:
                        self.set_cell_xf(r, c, fn(self.cell_xf(r, c), r, c, rg))
                        created += 1
                if positional:  # top and bottom edges live in the first and last row
                    for r in (r1, r2):
                        for c in range(c1, c2 + 1):
                            if c not in self.raw_cols(r):
                                self._set_if_different(r, c, fn(self.cell_xf(r, c), r, c, rg))
            elif whole_rows(rg):
                for r in range(r1, r2 + 1):
                    row = self.row(r)
                    base = int(row.get("s", 0)) if row.get("customFormat") in ("1", "true") else 0
                    row.set("s", str(fn(base, r, None, rg)))
                    row.set("customFormat", "1")
                    for c, el in list(self.cells(r).items()):
                        self.set_cell_xf(r, c, fn(int(el.get("s", 0)), r, c, rg))
                    if positional:
                        for c in (c1, c2):
                            if c not in self.cells(r):
                                self._set_if_different(r, c, fn(self.cell_xf(r, c), r, c, rg))
                    created += 1
                    if created > CELL_LIMIT:
                        raise ValueError("too many rows")
            else:
                n = (r2 - r1 + 1) * (c2 - c1 + 1)
                created += n
                if created > CELL_LIMIT:
                    raise ValueError("The selection is too large to format cell by cell.")
                for r in range(r1, r2 + 1):
                    for c in range(c1, c2 + 1):
                        self.set_cell_xf(r, c, fn(self.cell_xf(r, c), r, c, rg))
        self.changed = True

    def _set_if_different(self, r, c, xi):
        """A cell only where the style it would inherit is not already the right one."""
        if xi != self.empty_xf(r, c):
            self.set_cell_xf(r, c, xi)

    def xf_grid(self, rg):
        """Style indices of a block (for Format Painter), row by row."""
        r1, c1, r2, c2 = rg
        return [[self.cell_xf(r, c) for c in range(c1, c2 + 1)] for r in range(r1, r2 + 1)]

    # ---- contents
    def clear_cell_contents(self, r, c):
        el = self.cells(r).get(c)
        if el is None:
            return
        for ch in list(el):
            el.remove(ch)
        for k in ("t", "cm", "vm", "ph"):
            el.attrib.pop(k, None)
        self.changed = True

    # ---- merges
    def merges(self):
        mc = self.ws.find(M + "mergeCells")
        if mc is None:
            return []
        return [parse_range(m.get("ref")) for m in mc]

    def set_merges(self, rgs):
        mc = _child(self.ws, "mergeCells", SHEET_ORDER)
        for el in list(mc):
            mc.remove(el)
        for rg in rgs:
            ET.SubElement(mc, M + "mergeCell", ref=range_ref(rg))
        mc.set("count", str(len(rgs)))
        if not rgs:
            self.ws.remove(mc)
        self.changed = True


def intersects(a, b):
    return not (a[2] < b[0] or b[2] < a[0] or a[3] < b[1] or b[3] < a[1])


# --------------------------------------------------------------------------- the package

class Package:
    def __init__(self, data):
        self.zin = zipfile.ZipFile(io.BytesIO(data))
        self.names = self.zin.namelist()
        self._parts = {}
        wb = ET.fromstring(self.zin.read("xl/workbook.xml"))
        rels = ET.fromstring(self.zin.read("xl/_rels/workbook.xml.rels"))
        targets = {}
        styles_target = None
        for rel in rels:
            t = rel.get("Target")
            p = t.lstrip("/") if t.startswith("/") else "xl/" + t
            targets[rel.get("Id")] = p
            if rel.get("Type", "").endswith("/styles"):
                styles_target = p
        self.sheet_parts = []
        sheets = wb.find(M + "sheets")
        for sh in (sheets if sheets is not None else []):
            self.sheet_parts.append(targets.get(sh.get("{%s}id" % REL)))
        self.styles_name = styles_target or "xl/styles.xml"
        self.styles = Styles(self._part(self.styles_name, b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
                                        b'<styleSheet xmlns="%s"/>' % MAIN.encode()))
        self._sheets = {}

    def _part(self, name, default=None, lazy=None):
        if name not in self._parts:
            data = self.zin.read(name) if name in self.names else default
            self._parts[name] = Part(data, lazy)
        return self._parts[name]

    def sheet(self, i):
        if i not in self._sheets:
            name = self.sheet_parts[i]
            part = self._part(name, lazy="sheetData")
            if part.root.tag != M + "worksheet":
                raise ValueError("Formatting works on worksheets only.")
            self._sheets[i] = Sheet(part, self.styles)
        return self._sheets[i]

    def to_bytes(self):
        out = io.BytesIO()
        changed = {self.styles_name} if self.styles.changed else set()
        changed |= {self.sheet_parts[i] for i, s in self._sheets.items() if s.changed}
        with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=1) as z:
            for info in self.zin.infolist():
                if info.filename in changed:
                    z.writestr(info.filename, self._parts[info.filename].to_bytes())
                else:
                    z.writestr(info, self.zin.read(info.filename))
            if self.styles_name in changed and self.styles_name not in self.names:
                z.writestr(self.styles_name, self._parts[self.styles_name].to_bytes())
        return out.getvalue()
