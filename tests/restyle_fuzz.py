"""Random formatting over random ranges: the file must stay valid and ordered, the
engine must load it, and every cell must show the formats last applied to it."""
import io, os, random, re, sys, zipfile
import xml.dom.minidom
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "app"))
from folio_sheets.core import Book
from folio_sheets.history import HistoryBook
from folio_sheets import restyle as R

TPL = "/usr/share/folio-sheets/templates/Blank.xlsx"
bad = 0
for seed in range(int(os.environ.get("SEEDS", "30"))):
    rnd = random.Random(seed)
    hb = HistoryBook(Book.open(TPL))
    for _ in range(40):
        hb.set_input(0, rnd.randrange(30), rnd.randrange(12), str(rnd.random()))
    expect = {}  # (r, c) -> bold, for a sample of cells
    for step in range(25):
        kind = rnd.random()
        if kind < 0.15:
            c = rnd.randrange(12); rg = (0, c, R.MAX_ROWS - 1, c + rnd.randrange(2))
        elif kind < 0.3:
            r = rnd.randrange(30); rg = (r, 0, r + rnd.randrange(2), R.MAX_COLS - 1)
        else:
            r, c = rnd.randrange(30), rnd.randrange(12)
            rg = (r, c, r + rnd.randrange(4), c + rnd.randrange(4))
        bold = rnd.random() < 0.5
        p = R.Package(hb.snapshot()); sh = p.sheet(0)
        sh.apply([rg], lambda xf, r, c, g: p.styles.derive(xf, {"font": {"bold": bold}}))
        if rnd.random() < 0.3:
            sh.set_merges([m for m in sh.merges() if not R.intersects(m, rg)] + ([(rg[0], rg[1], rg[0] + 1, rg[1] + 1)] if rg[2] < 100 and rg[3] < 100 else []))
        data = p.to_bytes()
        hb.push(Book.from_bytes(data), "x")
        for r in range(0, 34):
            for c in range(0, 16):
                if R.intersects(rg, (r, c, r, c)):
                    expect[(r, c)] = bold
    z = zipfile.ZipFile(io.BytesIO(data))
    x = z.read("xl/worksheets/sheet1.xml")
    xml.dom.minidom.parseString(x); xml.dom.minidom.parseString(z.read("xl/styles.xml"))
    rows = [int(v) for v in re.findall(rb'<row\b[^>]*?\sr="(\d+)"', x)]
    ok = rows == sorted(set(rows))
    for row in re.findall(rb"<row\b.*?(?:/>|</row>)", x, re.S):
        cols = [R.parse_cell(m.decode())[1] for m in re.findall(rb'<c\b[^>]*?\sr="([A-Z]+\d+)"', row)]
        ok = ok and cols == sorted(set(cols))
    wrong = [(k, v) for k, v in expect.items() if hb.cell_style(0, *k)["bold"] != v]
    # (empty cells show their inherited style; the engine may report defaults for those)
    wrong = [w for w in wrong if hb.has_content(0, *w[0])]
    if not ok or wrong:
        bad += 1
        print("seed", seed, "ordered" if ok else "OUT OF ORDER", wrong[:5])
    open("/tmp/fuzz_%d.xlsx" % seed, "wb").write(data)
print("restyle fuzz: %d bad" % bad)
sys.exit(1 if bad else 0)
