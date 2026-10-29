"""Read R's serialization format (XDR, versions 2 and 3) and lazy-load databases (.rdb/.rdx),
enough for data packages: vectors, lists, attributes, S4 objects, environments and the common
ALTREP compact sequences. Written for tools/build_maps.py (the map charts' boundaries come from
Natural Earth, public domain, as Debian's r-cran-rnaturalearthdata packages them)."""
import bz2
import lzma
import struct
import zlib


class RObj:
    """An R value: .type (SEXP type), .value, .attr (dict name -> RObj)."""

    def __init__(self, type_, value=None, attr=None):
        self.type, self.value, self.attr = type_, value, attr or {}

    def __repr__(self):
        return "RObj(%d, %r%s)" % (self.type, self.value if not isinstance(self.value, list) or len(self.value) < 6 else "[%d items]" % len(self.value),
                                    ", attr=%s" % list(self.attr) if self.attr else "")

    def get(self, name):
        return self.attr.get(name)

    @property
    def names(self):
        n = self.attr.get("names")
        return n.value if n is not None else None

    def __getitem__(self, k):
        if isinstance(k, str):
            return self.value[self.names.index(k)]
        return self.value[k]


NA_INT = -2147483648


class Reader:
    def __init__(self, data):
        self.d = data
        self.i = 0
        self.refs = []

    def int(self):
        v = struct.unpack_from(">i", self.d, self.i)[0]
        self.i += 4
        return v

    def ints(self, n):
        v = struct.unpack_from(">%di" % n, self.d, self.i)
        self.i += 4 * n
        return list(v)

    def doubles(self, n):
        v = struct.unpack_from(">%dd" % n, self.d, self.i)
        self.i += 8 * n
        return list(v)

    def raw(self, n):
        v = self.d[self.i:self.i + n]
        self.i += n
        return v

    def length(self):
        n = self.int()
        if n == -1:
            hi, lo = self.int(), self.int()
            n = (hi << 32) + lo
        return n

    def header(self):
        fmt = self.raw(2)
        if fmt != b"X\n":
            raise ValueError("only XDR serialization is read (got %r)" % fmt)
        version = self.int()
        self.int()
        self.int()
        if version == 3:
            n = self.int()
            self.raw(n)

    def strvec(self):
        self.int()
        n = self.int()
        return [self.item() for _ in range(n)]

    def item(self):
        flags = self.int()
        t = flags & 0xFF
        has_attr = bool(flags & (1 << 9))
        has_tag = bool(flags & (1 << 10))
        if t == 255:                     # REFSXP
            k = flags >> 8
            if k == 0:
                k = self.int()
            return self.refs[k - 1]
        if t in (254, 253, 252, 251, 250, 242, 241):
            return RObj(t)
        if t in (249, 248, 247):         # namespace, package, persistent
            o = RObj(t, self.strvec())
            self.refs.append(o)
            return o
        if t == 1:                       # SYMSXP
            o = RObj(1)
            self.refs.append(o)
            o.value = self.item().value
            return o
        if t == 4:                       # ENVSXP
            o = RObj(4, {})
            self.refs.append(o)
            self.int()
            enclos, frame, hashtab, attr = self.item(), self.item(), self.item(), self.item()
            o.value = {"frame": frame, "hashtab": hashtab, "enclos": enclos}
            o.attr = _attr_dict(attr)
            return o
        if t in (2, 6, 3, 5, 17, 240, 239):  # pairlists and the like
            items = []
            attr = {}
            first = True
            while True:
                a = self.item() if has_attr else None
                tag = self.item() if has_tag else None
                car = self.item()
                if first and a is not None:
                    attr = _attr_dict(a)
                first = False
                items.append((tag.value if tag is not None and tag.type == 1 else None, car))
                nxt = self.int()
                nt = nxt & 0xFF
                if nt not in (2, 6, 17, 240, 239) or t in (3, 5):
                    # the CDR is something else: read it whole
                    self.i -= 4
                    cdr = self.item()
                    if cdr.type != 254:
                        items.append((None, cdr))
                    break
                has_attr = bool(nxt & (1 << 9))
                has_tag = bool(nxt & (1 << 10))
            return RObj(2 if t in (239,) else t, items, attr)
        if t == 9:                       # CHARSXP
            n = self.int()
            if n == -1:
                return RObj(9, None)
            return RObj(9, self.raw(n).decode("utf-8", "replace"))
        if t in (10, 13):
            n = self.length()
            v = self.ints(n)
            if t == 10:
                v = [None if x == NA_INT else bool(x) for x in v]
            else:
                v = [None if x == NA_INT else x for x in v]
            o = RObj(t, v)
        elif t == 14:
            o = RObj(14, self.doubles(self.length()))
        elif t == 15:
            n = self.length()
            d = self.doubles(2 * n)
            o = RObj(15, [complex(d[2 * k], d[2 * k + 1]) for k in range(n)])
        elif t == 16:
            n = self.length()
            o = RObj(16, [self.item().value for _ in range(n)])
        elif t in (19, 20):
            n = self.length()
            o = RObj(t, [self.item() for _ in range(n)])
        elif t == 24:
            o = RObj(24, self.raw(self.length()))
        elif t == 25:                    # S4SXP
            o = RObj(25)
        elif t in (7, 8):
            n = self.int()
            o = RObj(t, self.raw(n).decode())
        elif t == 22:                    # EXTPTRSXP
            o = RObj(22)
            self.refs.append(o)
            self.item()
            self.item()
        elif t == 23:
            o = RObj(23)
            self.refs.append(o)
        elif t == 238:                   # ALTREP
            info, state, attr = self.item(), self.item(), self.item()
            o = _altrep(info, state)
            if attr.type != 254:
                o.attr = _attr_dict(attr)
            return o
        else:
            raise ValueError("can't read SEXP type %d at %d" % (t, self.i))
        if has_attr:
            o.attr = _attr_dict(self.item())
        return o


def _attr_dict(p):
    if p is None or p.type == 254:
        return {}
    return {k: v for k, v in p.value} if isinstance(p.value, list) else {}


def _altrep(info, state):
    cls = info.value[0][1].value if info.type == 2 and info.value else None
    if cls == "compact_intseq":
        n, start, inc = state.value
        return RObj(13, [int(start + inc * k) for k in range(int(n))])
    if cls == "compact_realseq":
        n, start, inc = state.value
        return RObj(14, [start + inc * k for k in range(int(n))])
    if cls in ("wrap_integer", "wrap_real", "wrap_string", "wrap_logical", "wrap_list"):
        return state.value[0][1] if state.type == 2 else state.value[0]
    if cls == "deferred_string":
        arg = state.value[0][1] if state.type == 2 else state
        return RObj(16, [str(x) for x in arg.value])
    raise ValueError("can't read ALTREP class %s" % cls)


def unserialize(data):
    r = Reader(data)
    r.header()
    return r.item()


def decompress(blob):
    n = struct.unpack_from(">i", blob, 0)[0]
    body = blob[4:]
    if body[:1] in (b"Z", b"2", b"X", b"0") and len(body) > 1:
        kind = body[:1]
        rest = body[1:]
        if kind == b"Z":
            # R's raw LZMA2 stream (no .xz container)
            try:
                return lzma.decompress(rest, format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "preset": 9}])
            except lzma.LZMAError:
                return lzma.decompress(rest)
        if kind == b"2":
            return bz2.decompress(rest)
        if kind == b"0":
            return rest
    try:
        out = zlib.decompress(body)
    except zlib.error:
        out = lzma.decompress(body[1:])
    assert len(out) == n or n < 0, (len(out), n)
    return out


def lazyload(rdx_path, rdb_path):
    """{name: RObj} of a lazy-load database."""
    raw = open(rdx_path, "rb").read()
    idx = unserialize(zlib.decompress(raw, 31) if raw[:2] == b"\x1f\x8b" else decompress(raw))
    rdb = open(rdb_path, "rb").read()
    variables = idx["variables"]
    out = {}
    for name, pos in zip(variables.names, variables.value):
        off, ln = int(pos.value[0]), int(pos.value[1])
        blob = rdb[off:off + ln]
        r = Reader(decompress(blob))
        r.header()
        try:
            out[name] = r.item()
        except ValueError as e:
            out[name] = e
    return out
