import sys, zipfile, re
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open("/tmp/b6/rt_in.xlsx")
for i, o in enumerate(b.objects(3)):
    sp = b.chart_spec(3, i)
    print(i, o.get("kind"), {k: sp.get(k) for k in ("kind", "title", "legend", "legend_pos", "x_title", "y_title")} if sp else None)
    if i == 0: print("   spec keys:", sorted(sp.keys()))
z = zipfile.ZipFile("/tmp/b6/rt_in.xlsx")
x = z.read("xl/charts/chart1.xml").decode(); print(re.findall(r"<c:legend>.*?</c:legend>|<legend>.*?</legend>", x)[:2], x[:300])
print("C2 style:", {k: v for k, v in b.style_spec(0, 1, 2).items() if k in ("rotation", "halign", "valign", "wrap", "indent")} if isinstance(b.style_spec(0, 1, 2), dict) else b.style_spec(0, 1, 2))
