import sys, zipfile, xml.etree.ElementTree as E
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2])
vals = ["a\x00b", "bell\x07", "vt\x0bx", "esc\x1b[0m", "ok\ttab", "nl\nx", "cr\rx", "_x0041_ literal", "fffe￾", "ffff￿", "lone \ud800" if False else "emoji 😀", "<&>\"'", "]]>", "  lead and trail  ", "a b", "\x7f del", "\x85 nel"]
for i, v in enumerate(vals):
    try: b.set_input(0, i, 0, v)
    except BaseException as e: print("set_input refused", repr(v), str(e)[:60])
b.set_input(0, 0, 2, '="x"&CHAR(1)&"y"'); b.set_input(0, 1, 2, '=CHAR(11)'); b.define_name("Nmx", '="a"&CHAR(7)')
b.rename_sheet(0, "Sh<&>'t"); b.set_note(0, 0, 4, "Me\x01", "note\x02text") if True else None
b.save("/tmp/b6/ctl.xlsx")
z = zipfile.ZipFile("/tmp/b6/ctl.xlsx")
for n in z.namelist():
    if n.endswith((".xml", ".rels", ".vml")):
        try: E.fromstring(z.read(n))
        except Exception as e: print("NOT WELL-FORMED", n, e)
b2 = core.Book.open("/tmp/b6/ctl.xlsx"); bad = 0
for i, v in enumerate(vals):
    got = b2.value(0, i, 0)
    if got != b.value(0, i, 0): bad += 1; print("ROUND TRIP", repr(b.value(0, i, 0)), "->", repr(got))
print("formula results:", repr(b2.value(0, 0, 2)), repr(b2.value(0, 1, 2)), "| control-character strings changed by save/open:", bad)
for f in ['LEN(REPT("a",20000)&REPT("b",20000))', 'LEN(CONCAT(REPT("a",20000),REPT("b",20000)))', 'LEN(TEXTJOIN("",TRUE,REPT("a",20000),REPT("b",20000)))', 'LEN(SUBSTITUTE(REPT("a",20000),"a","bb"))', 'LEN(REPLACE(REPT("a",32767),1,1,"bb"))', 'LEN(CONCATENATE(REPT("a",20000),REPT("b",20000)))',
          'LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,100))', 'LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,250))', 'LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,60))', "SUM(" + "+".join(["A1"] * 1000) + ")", "IF(1," * 64 + "1" + ",0)" * 64, "IF(1," * 65 + "1" + ",0)" * 65, "LEN(\"" + "a" * 255 + "\")", "LEN(\"" + "a" * 256 + "\")", "A1" + "+A1" * 2700]:
    b.set_input(0, 20, 5, "=" + f); x = b.value(0, 20, 5); print((f[:60] + ("…" if len(f) > 60 else "")).ljust(62), "len %d ->" % (len(f) + 1), tuple(x) if isinstance(x, list) else x)
