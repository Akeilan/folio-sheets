import sys, zipfile, re
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
print(core.Book.render_png.__text_signature__, core.Book.render_png.__doc__)
z = zipfile.ZipFile("/tmp/b6/rt_in.xlsx"); x = z.read("xl/charts/chart4.xml").decode()
print(re.findall(r"<(valAx|catAx)>.*?<axPos val=\"(\w)\"/>", x), re.findall(r"<a:t>(.*?)</a:t>", x))
b = core.Book.open("/tmp/b6/rt_in.xlsx")
sp = b.style_spec(0, 1, 2); print(type(sp), sp if not isinstance(sp, dict) else {k: sp[k] for k in sp if "rot" in k or "align" in k or "wrap" in k})
