exec(open("/tmp/b6/uiharn.py").read())
import tempfile, time, zipfile, io
T = tempfile.mkdtemp()
def w(name, data, mode="wb"):
    p = os.path.join(T, name); open(p, mode).write(data); return p
def load(p):
    MSG.clear(); win.load(p); pump(0.2); return win.book
# CSV basics
p = w("a.csv", 'name,qty,when,code,pct,note\r\napple,12,2024-03-15,00123,50%,"said ""hi"", twice"\r\npear,7,3/1/2024,1E5,5.5,"line1\nline2"\r\n=1+1,"1,234",,-5,(3),\'x\r\n'.encode())
bk = load(p); g = win.grid
ck("csv: text", bk.value(0, 1, 0), "apple"); ck("csv: number", bk.value(0, 1, 1), 12.0); ck("csv: ISO date becomes a date", bk.value(0, 1, 2), 45366.0)
ck("csv: quoted quotes", bk.value(0, 1, 5), 'said "hi", twice'); ck("csv: line break inside quotes", bk.value(0, 2, 5), "line1\nline2")
ck("csv: a formula in the file is evaluated as in Excel", bk.value(0, 3, 0), 2.0); info("csv: =1+1 cell input", bk.input(0, 3, 0))
ck("csv: quoted 1,234 is a number", bk.value(0, 3, 1), 1234.0); info("csv: 00123 ->", (bk.value(0, 1, 3), bk.display(0, 1, 3))); info("csv: 'x ->", bk.value(0, 3, 5))
ck("csv: title shows the file name", "a" in win.get_title(), True); info("title", win.get_title()); ck("csv: opened unmodified", bk.modified, False)
# encodings
bk = load(w("latin1.csv", "naïve,café,Ærø\r\n1,2,3\r\n".encode("latin-1"))); ck("csv in Windows-1252 keeps accented letters", bk.value(0, 0, 1), "café")
bk = load(w("utf16.csv", "a\tb\r\n1\t2\r\n".encode("utf-16"))); ck("UTF-16 text file (Excel's Unicode Text) opens", (bk.value(0, 0, 0), bk.value(0, 1, 1)), ("a", 2.0))
bk = load(w("bom.csv", b"\xef\xbb\xbfa,b\r\n1,2\r\n")); ck("UTF-8 BOM is not part of the first cell", bk.value(0, 0, 0), "a")
bk = load(w("semi.csv", b"a;b;c\r\n1,5;2;3\r\n")); ck("semicolon-separated file splits into columns", (bk.value(0, 0, 1), bk.value(0, 1, 2)), ("b", 3.0))
bk = load(w("one.csv", b"just one column\r\nsecond, with a comma\r\n")); info("one-column-ish csv", [bk.value(0, r, c) for r in (0, 1) for c in (0, 1)])
bk = load(w("t.tsv", b"a\tb,c\r\n1\t2\r\n")); ck("tsv splits on tabs only", (bk.value(0, 0, 1), bk.value(0, 1, 1)), ("b,c", 2.0))
bk = load(w("empty.csv", b"")); ck("an empty csv opens as an empty sheet", (len(MSG), bk.used_extent(0)), (0, None)); info("empty csv msgs", MSG)
bk = load(w("wide.csv", (",".join(str(i) for i in range(300)) + "\r\n").encode())); ck("300 columns", bk.value(0, 0, 299), 299.0)
bk = load(w("ragged.csv", b"a,b,c\r\n1\r\n1,2,3,4,5\r\n")); ck("ragged rows", bk.value(0, 2, 4), 5.0)
bk = load(w("lf.csv", b"a,b\n1,2\n")); ck("LF line ends", bk.value(0, 1, 1), 2.0); bk = load(w("cr.csv", b"a,b\r1,2\r")); ck("CR-only line ends (old Mac)", bk.value(0, 1, 1), 2.0)
# big CSV: time to open
rows = 100000
big = io.StringIO(); 
for i in range(rows): big.write("%d,name%d,%0.2f,2024-01-%02d,%s\r\n" % (i, i, i * 1.5, i % 28 + 1, "TRUE" if i % 2 else "FALSE"))
p = w("big.csv", big.getvalue().encode()); t0 = time.time(); bk = load(p); dt = time.time() - t0
info("100,000 x 5 csv opens in seconds", round(dt, 1)); ck("big csv last row", bk.value(0, rows - 1, 0), float(rows - 1))
t0 = time.time(); bk.sort_range(0, (0, 0, rows - 1, 4), [(2, False)], False); info("sort 100k rows seconds", round(time.time() - t0, 2))
t0 = time.time(); bk.set_input(0, 0, 6, "=SUM(C:C)"); bk.set_input(0, 1, 6, "=COUNTIF(B:B,\"name5*\")"); bk.set_input(0, 2, 6, "=VLOOKUP(\"name99999\",B:C,2,FALSE)"); info("three whole-column formulas seconds", (round(time.time() - t0, 2), bk.value(0, 0, 6), bk.value(0, 1, 6), bk.value(0, 2, 6)))
t0 = time.time(); bk.autofill(0, (0, 7, 0, 7), (0, 7, 0, 7)) if False else None
bk.set_input(0, 0, 7, "=C1*2+A1"); t0 = time.time(); bk.autofill(0, (0, 7, 0, 7), (0, 7, rows - 1, 7)); info("fill a formula down 100k rows seconds", round(time.time() - t0, 2))
t0 = time.time(); bk.set_input(0, 0, 0, "5"); info("recalc after changing one input seconds", round(time.time() - t0, 3))
px = os.path.join(T, "big.xlsx"); t0 = time.time(); bk.save(px); t1 = time.time(); b2 = type(bk).open(px) if hasattr(type(bk), "open") else None; info("save / reopen 100k x 8 seconds, MB", (round(t1 - t0, 1), round(time.time() - t1, 1), round(os.path.getsize(px) / 1e6, 1)))
t0 = time.time(); g.set_book(win.book) if False else None; g.refresh(); pump(0.3); t0 = time.time()
for _ in range(20): g.scroll_by(drows=40); pump(0.01)
info("20 page scrolls seconds", round(time.time() - t0, 2))
# save csv
bk = load(w("s.csv", b"a,b\r\n1.50,\"x,y\"\r\n2024-03-15,50%\r\n")); out = os.path.join(T, "out.csv"); win.save_csv(out); info("csv written", open(out, "rb").read())
# odd workbooks
import folio_sheets_core as core
good = open(px.replace("big.xlsx", "x.xlsx"), "wb") if False else None
bk = core.Book.open(win.app.blank_template()); bk.set_input(0, 0, 0, "hello"); pg = os.path.join(T, "good.xlsx"); bk.save(pg); raw = open(pg, "rb").read()
cases = {"truncated.xlsx": raw[: len(raw) // 2], "garbage.xlsx": b"this is not a zip file at all", "empty.xlsx": b"", "notxlsx.xlsx": open("/tmp/b6/hang.png", "rb").read() if os.path.exists("/tmp/b6/hang.png") else b"\x89PNG", "docx-as-xlsx.xlsx": None}
zb = io.BytesIO(); z = zipfile.ZipFile(zb, "w"); z.writestr("word/document.xml", "<w:document/>"); z.writestr("[Content_Types].xml", "<Types/>"); z.close(); cases["docx-as-xlsx.xlsx"] = zb.getvalue()
zb = io.BytesIO(); zi = zipfile.ZipFile(io.BytesIO(raw)); z = zipfile.ZipFile(zb, "w")
for n in zi.namelist(): z.writestr(n, zi.read(n) if "sheet1" not in n else b"<worksheet><sheetData><row r='1'><c r='A1' t='s'><v>999</v></c><c r='ZZZZ1'><v>1</v></c><c r='B1' t='n'><v>abc</v></c><c><v>5</v></c></row><row r='0'/><row r='9999999'><c r='A9999999'><v>1</v></c></row></sheetData>")
z.close(); cases["badsheet.xlsx"] = zb.getvalue()
for name, data in cases.items():
    MSG.clear()
    try: load(w(name, data)); info("open " + name, MSG[:1] or "opened without a message; A1=%r" % (win.book.value(0, 0, 0),))
    except BaseException as e: print("FAIL open %s raised %s: %s" % (name, type(e).__name__, str(e)[:150]), flush=True); N[1] += 1
MSG.clear(); load(os.path.join(T, "does-not-exist.xlsx")); info("missing file", MSG[:1])
done()
