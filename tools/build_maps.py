"""Build engine/data/maps.txt, the boundaries the Map charts draw (S3-13).

Source: Natural Earth (public domain), 1:50m countries and the 1:50m states and provinces of the
United States, Canada, Brazil and Australia, as Debian's r-cran-rnaturalearthdata (CC0) packages
them: `apt-get download r-cran-rnaturalearthdata`, `dpkg-deb -x` it, and pass the package's
data directory. The shapes are simplified (Douglas-Peucker) and kept to 0.01 degree.

Lines:   C|iso_a2|iso_a3|name|other names;...|continent|subregion|label x,label y|rings
         S|country iso_a3|code|postal|name|other names;...|label x,label y|rings
rings:   ring;ring;...   ring: x,y x,y ...  (degrees * 100, integers; holes as rings too)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rdata  # noqa: E402

TOL = 0.04       # degrees
MIN_AREA = 0.02  # square degrees: smaller islands go (unless the region has nothing bigger)


def simplify(pts, tol):
    if len(pts) < 4:
        return pts
    keep = [False] * len(pts)
    keep[0] = keep[-1] = True
    stack = [(0, len(pts) - 1)]
    while stack:
        a, b = stack.pop()
        ax, ay = pts[a]
        bx, by = pts[b]
        dx, dy = bx - ax, by - ay
        L = (dx * dx + dy * dy) ** 0.5
        best, bi = 0.0, -1
        for i in range(a + 1, b):
            px, py = pts[i]
            if L == 0:
                d = ((px - ax) ** 2 + (py - ay) ** 2) ** 0.5
            else:
                d = abs(dy * px - dx * py + bx * ay - by * ax) / L
            if d > best:
                best, bi = d, i
        if best > tol and bi > 0:
            keep[bi] = True
            stack.append((a, bi))
            stack.append((bi, b))
    return [p for p, k in zip(pts, keep) if k]


def area(pts):
    s = 0.0
    for (x1, y1), (x2, y2) in zip(pts, pts[1:] + pts[:1]):
        s += x1 * y2 - x2 * y1
    return abs(s) / 2


def rings_of(poly):
    """The rings of one sp Polygons object."""
    out = []
    for p in poly.attr["Polygons"].value:
        c = p.attr["coords"]
        n = c.attr["dim"].value[0]
        xs, ys = c.value[:n], c.value[n:]
        out.append((list(zip(xs, ys)), bool(p.attr["hole"].value[0])))
    return out


def encode(rings, tol):
    big = [(r, h) for r, h in rings if area(r) >= MIN_AREA] or sorted(rings, key=lambda x: -area(x[0]))[:1]
    parts = []
    for r, _hole in big:
        s = simplify(r, tol)
        q = []
        for x, y in s:
            p = (int(round(x * 100)), int(round(y * 100)))
            if not q or q[-1] != p:
                q.append(p)
        if len(q) >= 4:
            parts.append(" ".join("%d,%d" % p for p in q))
    return ";".join(parts)


def col(df, name):
    return df.value[df.names.index(name)].value


def clean(t):
    return (t or "").replace("|", "/").replace(";", ",").strip()


def main(datadir, out):
    o = rdata.lazyload(os.path.join(datadir, "Rdata.rdx"), os.path.join(datadir, "Rdata.rdb"))
    lines = []
    c = o["countries50"]
    df = c.attr["data"]
    polys = c.attr["polygons"].value
    for i in range(len(polys)):
        iso2, iso3 = col(df, "iso_a2")[i], col(df, "iso_a3")[i]
        if iso3 in (None, "-99"):
            iso3 = col(df, "adm0_a3")[i]
        if iso2 in (None, "-99"):
            iso2 = ""
        name = col(df, "name")[i]
        alts = []
        for k in ("name_long", "admin", "formal_en", "name_alt", "sovereignt", "abbrev", "brk_name", "geounit", "subunit", "name_sort"):
            v = col(df, k)[i]
            if v and v not in alts and v != name and v != "NA":
                alts.append(v)
        lab = polys[i].attr["labpt"].value
        lines.append("C|%s|%s|%s|%s|%s|%s|%.2f,%.2f|%s" % (iso2, iso3, clean(name), ";".join(clean(a) for a in alts),
                                                        clean(col(df, "continent")[i]), clean(col(df, "subregion")[i]),
                                                        lab[0], lab[1], encode(rings_of(polys[i]), TOL)))
    st = o["states50"]
    df = st.attr["data"]
    polys = st.attr["polygons"].value
    adm = {"United States of America": "USA", "Brazil": "BRA", "Canada": "CAN", "Australia": "AUS"}
    for i in range(len(polys)):
        country = adm.get(col(df, "admin")[i], col(df, "sr_adm0_a3")[i])
        name = col(df, "name")[i]
        alts = [v for v in (col(df, "name_alt")[i], col(df, "name_local")[i], col(df, "abbrev")[i]) if v and v != name and v != "NA"]
        code = col(df, "iso_3166_2")[i] or ""
        postal = col(df, "postal")[i] or ""
        lab = polys[i].attr["labpt"].value
        lines.append("S|%s|%s|%s|%s|%s|%.2f,%.2f|%s" % (country, clean(code), clean(postal), clean(name),
                                                     ";".join(clean(a) for a in alts if a), lab[0], lab[1],
                                                     encode(rings_of(polys[i]), TOL / 2)))
    head = ("# Map chart boundaries: Natural Earth 1:50m (public domain) via r-cran-rnaturalearthdata (CC0),\n"
            "# simplified by tools/build_maps.py. Countries (C) and states/provinces (S).\n")
    with open(out, "w") as f:
        f.write(head + "\n".join(lines) + "\n")
    print("wrote", out, os.path.getsize(out), "bytes,", len(lines), "regions")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
