#!/usr/bin/env python3
"""Rebuild engine/data/maps.txt, the boundaries map charts draw (S3-13, S3-14).

The shapes come from Natural Earth 1:50m, which is public domain, through the Debian package
r-cran-rnaturalearthdata (CC0). Nothing is fetched from the internet.

    sudo apt install r-cran-rnaturalearthdata r-cran-sf
    tools/build_maps.py [--states-only]

States and provinces exist for the nine countries Natural Earth covers at this scale: Australia,
Brazil, Canada, China, India, Indonesia, Russia, South Africa and the United States.
"""
import subprocess, sys, tempfile, os, re

R_DUMP = r'''
suppressMessages({library(rnaturalearthdata); library(sf)})
sf::sf_use_s2(FALSE)
dump <- function(d, path, cols) {
  g <- sf::st_simplify(sf::st_geometry(d), dTolerance = TOL, preserveTopology = TRUE)
  con <- file(path, "w")
  for (i in seq_len(nrow(d))) {
    r <- d[i, ]
    vals <- sapply(cols, function(k) { v <- r[[k]]; if (is.null(v) || is.na(v)) "" else as.character(v) })
    cat(paste(c(vals, sf::st_as_text(g[i])), collapse = "\t"), "\n", sep = "", file = con)
  }
  close(con)
}
dump(sf::st_as_sf(states50), "STATES_OUT",
     c("adm0_a3", "iso_3166_2", "postal", "name", "name_alt", "longitude", "latitude"))
'''
# how far a boundary may move, in degrees: about a kilometre, which no map chart shows
TOL = 0.02


def rings_of(wkt, keep=0.0):
    """The rings of a POLYGON or MULTIPOLYGON as lists of (lon, lat)."""
    out = []
    for body in re.findall(r'\(([-0-9eE., ]+)\)', wkt):
        pts = []
        for p in body.split(','):
            p = p.strip()
            if not p:
                continue
            try:
                lon, lat = (float(x) for x in p.split())
            except ValueError:
                continue
            pts.append((lon, lat))
        if len(pts) >= 3:
            out.append(pts)
    return out


def area(ring):
    s = 0.0
    for i in range(len(ring)):
        x1, y1 = ring[i]
        x2, y2 = ring[(i + 1) % len(ring)]
        s += x1 * y2 - x2 * y1
    return abs(s) / 2.0


def encode(rings, biggest_share=0.004):
    """Hundredths of a degree, whole numbers, with the specks dropped."""
    if not rings:
        return ""
    big = max(area(r) for r in rings)
    parts = []
    for r in rings:
        if big > 0 and area(r) < big * biggest_share:
            continue
        pts, last = [], None
        for lon, lat in r:
            q = (int(round(lon * 100)), int(round(lat * 100)))
            if q != last:
                pts.append(q)
                last = q
        if len(pts) >= 3:
            if pts[0] != pts[-1]:
                pts.append(pts[0])
            parts.append(' '.join(f"{x},{y}" for x, y in pts))
    return ';'.join(parts)


def main():
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data = os.path.join(here, "engine/data/maps.txt")
    tmp = tempfile.mkdtemp()
    states_tsv = os.path.join(tmp, "states.tsv")
    script = R_DUMP.replace("STATES_OUT", states_tsv).replace("TOL", str(TOL))
    rs = os.path.join(tmp, "dump.R")
    open(rs, "w").write(script)
    r = subprocess.run(["Rscript", rs], capture_output=True, text=True)
    if not os.path.exists(states_tsv):
        sys.exit("Rscript failed:\n" + r.stderr[-2000:])

    lines = []
    for raw in open(states_tsv):
        f = raw.rstrip("\n").split("\t")
        if len(f) < 8:
            continue
        iso3, code, postal, name, alt, lon, lat, wkt = f[:8]
        # Natural Earth parks a placeholder like "AU-X02~" where it has no real
        # ISO 3166-2 code; an unusable code is worse than none, so drop it
        if "~" in code or re.search(r"-X[0-9]", code):
            code = ""
        rings = encode(rings_of(wkt))
        if not rings:
            continue
        alts = alt if alt and alt != "NA" else ""
        try:
            label = f"{float(lon):.2f},{float(lat):.2f}"
        except ValueError:
            label = "0,0"
        lines.append(f"S|{iso3}|{code}|{postal}|{name}|{alts}|{label}|{rings}")

    old = open(data).read().splitlines()
    head = [l for l in old if l.startswith("#")]
    countries = [l for l in old if l.startswith("C|")]
    if not lines:
        sys.exit("no states came back")
    lines.sort(key=lambda l: (l.split("|")[1], l.split("|")[4]))
    with open(data, "w") as fh:
        fh.write("\n".join(head + countries + lines) + "\n")
    by_country = {}
    for l in lines:
        by_country[l.split("|")[1]] = by_country.get(l.split("|")[1], 0) + 1
    print(f"{len(countries)} countries, {len(lines)} states and provinces")
    for k in sorted(by_country):
        print(f"  {k} {by_country[k]}")


if __name__ == "__main__":
    main()
