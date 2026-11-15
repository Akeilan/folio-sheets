# Hand-made workbook with the things Excel itself writes: rich text, shared formulas, inline strings,
# 1904 dates, theme colours with tint, unknown extensions, custom parts. Open, check, save, look again.
import sys, zipfile, io, re
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
src = zipfile.ZipFile(sys.argv[2])
parts = {n: src.read(n) for n in src.namelist()}
ss = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="4"><si><t>plain</t></si><si><r><t xml:space="preserve">part </t></r><r><rPr><b/><sz val="11"/><color rgb="FFFF0000"/><rFont val="Calibri"/></rPr><t>bold red</t></r><r><rPr><i/><sz val="11"/><rFont val="Calibri"/></rPr><t xml:space="preserve"> italic</t></r></si><si><t xml:space="preserve">  two leading spaces</t></si><si><t>phon</t><rPh sb="0" eb="1"><t>ph</t></rPh><phoneticPr fontId="1"/></si></sst>'''
sheet = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x14ac" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"><sheetPr codeName="Sheet1x"><tabColor theme="4" tint="0.39997558519241921"/></sheetPr><dimension ref="A1:D9"/><sheetViews><sheetView tabSelected="1" workbookViewId="0"><pane xSplit="2000" ySplit="1500" topLeftCell="C5" activePane="bottomRight"/><selection pane="bottomRight" activeCell="C5" sqref="C5"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="15" x14ac:dyDescent="0.25"/><sheetData>
<row r="1" spans="1:4" x14ac:dyDescent="0.25"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c><c r="D1" t="s"><v>3</v></c></row>
<row r="2"><c r="A2"><v>1</v></c><c r="B2"><f t="shared" ref="B2:B5" si="0">A2*2</f><v>2</v></c><c r="C2" t="inlineStr"><is><t>inline</t></is></c><c r="D2" t="b"><v>1</v></c></row>
<row r="3"><c r="A3"><v>2</v></c><c r="B3"><f t="shared" si="0"/><v>4</v></c><c r="C3" t="e"><v>#N/A</v></c><c r="D3" t="str"><f>"str"&amp;A3</f><v>str2</v></c></row>
<row r="4"><c r="A4"><v>3</v></c><c r="B4"><f t="shared" si="0"/><v>6</v></c><c r="C4" t="inlineStr"><is><r><t>rich </t></r><r><rPr><b/></rPr><t>inline</t></r></is></c><c r="D4" t="d"><v>2024-03-15T00:00:00Z</v></c></row>
<row r="5"><c r="A5"><v>4</v></c><c r="B5"><f t="shared" si="0"/><v>8</v></c><c r="C5"><f t="array" ref="C5:C6">A2:A3*10</f><v>10</v></c><c r="D5"><f>SUM(B2:B5)</f><v>20</v></c></row>
<row r="6"><c r="C6"><v>20</v></c><c r="D6"><f ca="1">NOW()</f><v>1</v></c></row>
<row r="7"><c r="A7"><f>_xlfn.XLOOKUP(2,A2:A5,B2:B5)</f><v>4</v></c><c r="B7"><f>_xlfn.CONCAT(A1,"x")</f><v>plainx</v></c><c r="C7"><f>_xlfn._xlws.FILTER(A2:A5,A2:A5&gt;2)</f><v>3</v></c><c r="D7"><f>_xlfn.STDEV.S(A2:A5)</f><v>1.29</v></c></row>
<row r="9"><c r="A9"><v>100</v></c></row>
</sheetData><extLst><ext uri="{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:dataValidations count="0" xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"/></ext><ext uri="{11111111-2222-3333-4444-555555555555}"><unknownThing xmlns="urn:keep-me" val="42"/></ext></extLst></worksheet>'''
wbx = parts["xl/workbook.xml"].decode()
wbx = re.sub(r"<workbookPr[^>]*/>", '<workbookPr date1904="1" codeName="ThisWorkbookX"/>', wbx) if "<workbookPr" in wbx else wbx.replace("<sheets>", '<workbookPr date1904="1"/><sheets>')
parts["xl/workbook.xml"] = wbx.encode(); parts["xl/sharedStrings.xml"] = ss.encode()
first = sorted(n for n in parts if n.startswith("xl/worksheets/sheet"))[0]; parts[first] = sheet.encode()
parts["customXml/item1.xml"] = b'<?xml version="1.0"?><keep xmlns="urn:custom">me</keep>'; parts["docProps/custom.xml"] = b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Project"><vt:lpwstr>Folio</vt:lpwstr></property></Properties>'
ct = parts["[Content_Types].xml"].decode()
if "sharedStrings" not in ct: ct = ct.replace("</Types>", '<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>')
ct = ct.replace("</Types>", '<Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/><Default Extension="xml" ContentType="application/xml"/></Types>') if 'Extension="xml"' not in ct else ct.replace("</Types>", '<Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/></Types>')
parts["[Content_Types].xml"] = ct.encode()
rels = parts["xl/_rels/workbook.xml.rels"].decode()
if "sharedStrings" not in rels: rels = rels.replace("</Relationships>", '<Relationship Id="rId901" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>')
rels = rels.replace("</Relationships>", '<Relationship Id="rId902" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="../customXml/item1.xml"/></Relationships>'); parts["xl/_rels/workbook.xml.rels"] = rels.encode()
rr = parts["_rels/.rels"].decode().replace("</Relationships>", '<Relationship Id="rId903" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/></Relationships>'); parts["_rels/.rels"] = rr.encode()
z = zipfile.ZipFile("/tmp/b6/hand.xlsx", "w", zipfile.ZIP_DEFLATED)
for n, d in parts.items(): z.writestr(n, d)
z.close()
b = core.Book.open("/tmp/b6/hand.xlsx")
def v(r, c):
    x = b.value(0, r, c); return tuple(x) if isinstance(x, list) else x
exp = {(0, 0): "plain", (0, 1): "part bold red italic", (0, 2): "  two leading spaces", (0, 3): "phon", (1, 1): 2.0, (2, 1): 4.0, (4, 1): 8.0, (1, 2): "inline", (1, 3): True, (2, 2): ("error", "#N/A"), (2, 3): "str2", (3, 2): "rich inline", (4, 2): 10.0, (5, 2): 20.0, (4, 3): 20.0, (6, 0): 4.0, (6, 1): "plainx", (6, 2): 3.0}
bad = 0
for k, w in exp.items():
    if v(*k) != w: bad += 1; print("OPEN  %s%d want %r got %r" % ("ABCD"[k[1]], k[0] + 1, w, v(*k)))
print("inputs:", [b.input(0, r, 1) for r in (1, 2, 4)], b.input(0, 4, 2), b.input(0, 6, 0), b.input(0, 6, 2), "| date1904:", b.date1904, "| D4 (t=d):", v(3, 3), b.display(0, 3, 3), "| array range C5:", b.array_range(0, 4, 2), "| view:", {k: b.sheet_info(0).get(k) for k in ("freeze_rows", "freeze_cols", "top_row", "left_col", "active")})
b.save("/tmp/b6/hand_out.xlsx")
o = zipfile.ZipFile("/tmp/b6/hand_out.xlsx"); on = o.namelist()
sso = o.read("xl/sharedStrings.xml").decode() if "xl/sharedStrings.xml" in on else ""; sh = o.read(first).decode(); wbo = o.read("xl/workbook.xml").decode()
def keep(name, cond):
    global bad
    if not cond: bad += 1; print("LOST  " + name)
keep("rich text runs in a shared string (bold red part)", "<b/>" in sso and "FFFF0000" in sso and "bold red" in sso)
keep("leading spaces (xml:space=preserve)", 'xml:space="preserve">  two leading spaces' in sso or ">  two leading spaces<" in sso and "preserve" in sso)
keep("phonetic run", "<rPh" in sso)
keep("rich inline string's bold run", ("rich " in sso + sh) and ("<b/>" in sh or sso.count("<b/>") >= 2))
keep("unknown extension in the sheet (urn:keep-me)", "urn:keep-me" in sh)
keep("x14 extension", "CCE6A557" in sh)
keep("sheet codeName", 'codeName="Sheet1x"' in sh); keep("workbook codeName", "ThisWorkbookX" in wbo); keep("date1904", 'date1904="1"' in wbo)
keep("theme tab colour with tint", 'theme="4"' in sh and "tint=" in sh)
keep("split panes (not frozen)", "xSplit" in sh and "frozen" not in sh)
keep("custom XML part", "customXml/item1.xml" in on); keep("custom document properties", "docProps/custom.xml" in on)
keep("array formula C5:C6", 't="array"' in sh and 'ref="C5:C6"' in sh)
keep("_xlfn. prefix on XLOOKUP", "_xlfn.XLOOKUP" in sh); keep("_xlfn._xlws. prefix on FILTER", "_xlfn._xlws.FILTER" in sh); keep("_xlfn. prefix on STDEV.S", "_xlfn.STDEV.S" in sh); keep("volatile flag or NOW formula", "NOW()" in sh)
keep("x14ac:dyDescent / mc:Ignorable", "mc:Ignorable" in sh)
print("hand-made workbook:", bad, "problems")
import xml.etree.ElementTree as E
for n in on:
    if n.endswith((".xml", ".rels")):
        try: E.fromstring(o.read(n))
        except Exception as e: print("XML  ", n, e)
