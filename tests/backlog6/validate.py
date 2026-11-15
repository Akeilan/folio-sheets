import sys, zipfile, re, xml.etree.ElementTree as E, subprocess, os
for p in sys.argv[1:]:
    z = zipfile.ZipFile(p); bad = 0; names = z.namelist()
    print(p, len(names), "parts")
    for n in names:
        if n.endswith((".xml", ".rels", ".vml")):
            try: E.fromstring(z.read(n))
            except Exception as e: bad += 1; print("  XML  ", n, e)
    # every relationship target exists; every part has a content type
    ct = z.read("[Content_Types].xml").decode(); defaults = set(re.findall(r'<Default Extension="([^"]+)"', ct)); overrides = set(re.findall(r'<Override PartName="/([^"]+)"', ct))
    for n in names:
        if n.endswith("/") or n == "[Content_Types].xml": continue
        if n not in overrides and n.rsplit(".", 1)[-1].lower() not in {d.lower() for d in defaults}: bad += 1; print("  TYPE  no content type for", n)
    for o in overrides:
        if o not in names: bad += 1; print("  TYPE  override for a missing part", o)
    for n in names:
        if n.endswith(".rels"):
            base = os.path.dirname(os.path.dirname(n))
            for m in re.finditer(r'<Relationship\b([^>]*)>', z.read(n).decode()):
                a = m.group(1)
                if 'TargetMode="External"' in a: continue
                t = re.search(r'Target="([^"]+)"', a).group(1)
                full = t[1:] if t.startswith("/") else os.path.normpath(os.path.join(base, t))
                if full not in names: bad += 1; print("  REL  ", n, "->", t, "missing")
    seen = set()
    for n in names:
        if n in seen: bad += 1; print("  ZIP   duplicate entry", n)
        seen.add(n)
    try:
        import openpyxl, warnings
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always"); wb = openpyxl.load_workbook(p); print("  openpyxl loads:", len(wb.worksheets), "sheets;", len(w), "warnings", sorted({str(x.message)[:70] for x in w})[:6])
    except Exception as e: bad += 1; print("  OPENPYXL", type(e).__name__, str(e)[:200])
    out = "/tmp/b6/lo_" + os.path.basename(p)[:-5]; os.makedirs(out, exist_ok=True)
    r = subprocess.run(["soffice", "--headless", "--convert-to", "pdf", "--outdir", out, p], capture_output=True, text=True, timeout=300, env=dict(os.environ, HOME="/tmp/lohome"))
    okf = os.path.exists(os.path.join(out, os.path.basename(p)[:-5] + ".pdf")); print("  LibreOffice opens and prints it:", okf, "" if okf else (r.stdout + r.stderr)[-200:]); bad += not okf
    print("  problems:", bad)
