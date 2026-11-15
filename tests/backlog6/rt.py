# Round trip: a workbook openpyxl writes -> Folio Sheet opens and saves -> openpyxl reads both and compares.
import sys, datetime
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
import openpyxl
from openpyxl.styles import Font, PatternFill, Border, Side, Alignment, Protection, GradientFill
from openpyxl.comments import Comment
from openpyxl.worksheet.datavalidation import DataValidation
from openpyxl.formatting.rule import CellIsRule, ColorScaleRule, DataBarRule, IconSetRule, FormulaRule
from openpyxl.worksheet.table import Table, TableStyleInfo
from openpyxl.workbook.defined_name import DefinedName
from openpyxl.chart import BarChart, LineChart, PieChart, ScatterChart, Reference, Series
wb = openpyxl.Workbook(); ws = wb.active; ws.title = "Data"
ws.append(["Name", "Qty", "Price", "When", "Flag", "Total"])
rows = [("apple", 12, 1.25, datetime.date(2024, 1, 5), True), ("pear", 7, 2.5, datetime.date(2024, 2, 14), False), ("plum", 30, 0.4, datetime.date(2023, 3, 1), True), ("fig", 3, 4.75, datetime.datetime(2023, 12, 31, 13, 30), False)]
for i, r in enumerate(rows, 2):
    ws.append(list(r) + ["=B%d*C%d" % (i, i)])
ws["A7"] = "Sum"; ws["F7"] = "=SUM(F2:F5)"; ws["G1"] = "long text that overflows into the next cells"; ws["H2"] = datetime.time(6, 30); ws["H3"] = 1e-7; ws["H4"] = 1234567890123; ws["H5"] = "'quoted"; ws["H6"] = "=NA()"; ws["H7"] = "line1\nline2"
ws["A1"].font = Font(name="Arial", size=14, bold=True, italic=True, underline="double", strike=True, color="FF0000")
ws["B1"].fill = PatternFill("solid", fgColor="FFFF00"); ws["C1"].fill = PatternFill("darkGrid", fgColor="00FF00", bgColor="0000FF")
ws["D1"].fill = GradientFill(stop=("FF0000", "0000FF"), degree=45)
thin = Side(style="thin", color="000000"); ws["B2"].border = Border(left=thin, right=Side(style="double", color="FF0000"), top=Side(style="dashed"), bottom=Side(style="thick"), diagonal=Side(style="hair"), diagonalUp=True)
ws["C2"].alignment = Alignment(horizontal="center", vertical="top", wrap_text=True, text_rotation=45, indent=0, shrink_to_fit=False)
ws["A3"].alignment = Alignment(horizontal="right", indent=2); ws["A4"].alignment = Alignment(text_rotation=255); ws["A5"].alignment = Alignment(horizontal="distributed", vertical="justify", shrink_to_fit=True)
for c, f in (("C2", "#,##0.00"), ("C3", "0.00%"), ("C4", '_($* #,##0.00_);_($* (#,##0.00);_($* "-"??_);_(@_)'), ("C5", "0.00E+00"), ("D2", "yyyy-mm-dd"), ("D3", "[$-409]d-mmm-yy;@"), ("D5", "m/d/yy h:mm"), ("B3", "# ?/?"), ("B4", '[Red][<10]0;[Blue]0')):
    ws[c].number_format = f
ws["E2"].protection = Protection(locked=False, hidden=True)
ws.merge_cells("A9:C10"); ws["A9"] = "merged"
ws.column_dimensions["A"].width = 22.5; ws.column_dimensions["G"].hidden = True; ws.row_dimensions[2].height = 30; ws.row_dimensions[6].hidden = True
ws.column_dimensions.group("J", "K", outline_level=1, hidden=False); ws.row_dimensions.group(12, 14, outline_level=2)
ws.freeze_panes = "B2"; ws.sheet_properties.tabColor = "00B050"; ws.sheet_view.zoomScale = 85; ws.sheet_view.showGridLines = False
ws["A12"].hyperlink = "https://example.com/x?y=1&z=2"; ws["A12"] = "link"; ws["A13"].hyperlink = "#Second!B2"; ws["A13"] = "internal"
ws["B12"].comment = Comment("a note\nsecond line", "Tester")
dv = DataValidation(type="list", formula1='"red,green,blue"', allow_blank=True, showErrorMessage=True, errorTitle="Bad", error="Pick one", promptTitle="Colour", prompt="Choose"); ws.add_data_validation(dv); dv.add("J2:J5")
dv2 = DataValidation(type="whole", operator="between", formula1="1", formula2="10", errorStyle="warning"); ws.add_data_validation(dv2); dv2.add("K2")
dv3 = DataValidation(type="custom", formula1="=LEN(L2)<5"); ws.add_data_validation(dv3); dv3.add("L2")
ws.conditional_formatting.add("B2:B5", CellIsRule(operator="greaterThan", formula=["10"], fill=PatternFill("solid", bgColor="FFC7CE"), font=Font(color="9C0006")))
ws.conditional_formatting.add("C2:C5", ColorScaleRule(start_type="min", start_color="F8696B", mid_type="percentile", mid_value=50, mid_color="FFEB84", end_type="max", end_color="63BE7B"))
ws.conditional_formatting.add("F2:F5", DataBarRule(start_type="min", end_type="max", color="638EC6"))
ws.conditional_formatting.add("B2:B5", IconSetRule("3TrafficLights1", "percent", [0, 33, 67]))
ws.conditional_formatting.add("A2:A5", FormulaRule(formula=['LEFT($A2,1)="p"'], font=Font(bold=True), stopIfTrue=True))
ws.auto_filter.ref = "A1:F5"; ws.auto_filter.add_filter_column(0, ["apple", "pear"])
ws.print_area = "A1:F7"; ws.print_title_rows = "1:1"; ws.page_setup.orientation = "landscape"; ws.page_setup.paperSize = 9; ws.page_setup.fitToWidth = 1; ws.page_setup.fitToHeight = 0; ws.sheet_properties.pageSetUpPr.fitToPage = True
ws.page_margins.left = 0.5; ws.page_margins.top = 1.25; ws.oddHeader.center.text = "Header &P of &N"; ws.oddFooter.left.text = "&D &T"; ws.print_options.gridLines = True; ws.print_options.horizontalCentered = True
from openpyxl.worksheet.pagebreak import Break
ws.row_breaks.append(Break(id=20)); ws.col_breaks.append(Break(id=4))
ws2 = wb.create_sheet("Second"); ws2["A1"] = "=Data!F7*2"; ws2["B2"] = "=SUM(Data!B2:B5)"; ws2["C3"] = "=TaxRate*100"; ws2.sheet_state = "hidden"
ws3 = wb.create_sheet("Tbl & 'Q'"); ws3.append(["K", "V"]);
for i in range(1, 6): ws3.append(["k%d" % i, i * 1.5])
t = Table(displayName="MyTable", ref="A1:B6"); t.tableStyleInfo = TableStyleInfo(name="TableStyleMedium9", showRowStripes=True, showColumnStripes=False); ws3.add_table(t)
ws3["D1"] = "=SUM(MyTable[V])"; ws3["D2"] = "='Tbl & ''Q'''!B2+1"
ws3.protection.sheet = True; ws3.protection.password = "secret"; ws3.protection.formatCells = False
ws4 = wb.create_sheet("Charts")
for r in [("x", "a", "b"), (1, 5, 2), (2, 7, 3), (3, 6, 8), (4, 9, 4)]: ws4.append(r)
for cls, anchor, title in ((BarChart, "E1", "Bars"), (LineChart, "E16", "Lines"), (PieChart, "M1", "Pie"), (ScatterChart, "M16", "XY")):
    ch = cls(); ch.title = title
    if cls is ScatterChart:
        ch.series.append(Series(Reference(ws4, min_col=2, min_row=1, max_row=5), Reference(ws4, min_col=1, min_row=2, max_row=5), title_from_data=True)); ch.x_axis.title = "X axis"; ch.y_axis.title = "Y axis"
    else:
        ch.add_data(Reference(ws4, min_col=2, max_col=2 if cls is PieChart else 3, min_row=1, max_row=5), titles_from_data=True); ch.set_categories(Reference(ws4, min_col=1, min_row=2, max_row=5))
    ws4.add_chart(ch, anchor)
wb.defined_names.append(DefinedName("TaxRate", attr_text="0.21")); wb.defined_names.append(DefinedName("Qtys", attr_text="Data!$B$2:$B$5")); wb.defined_names.append(DefinedName("LocalN", attr_text="Data!$A$1", localSheetId=0))
wb.properties.title = "RT title"; wb.properties.creator = "Tester"; wb.properties.keywords = "k1, k2"
wb.save("/tmp/b6/rt_in.xlsx")

def dump(path):
    wb = openpyxl.load_workbook(path); out = {}
    P = lambda k, v: out.__setitem__(k, v)
    P("sheets", [(s.title, s.sheet_state) for s in wb.worksheets])
    P("names", sorted((n.name, n.attr_text, n.localSheetId) for n in wb.defined_names.definedName))
    P("props", (wb.properties.title, wb.properties.creator, wb.properties.keywords))
    for s in wb.worksheets:
        k = s.title + "!"
        for row in s.iter_rows():
            for c in row:
                if c.value is not None: P(k + c.coordinate + ".v", c.value)
                if c.has_style:
                    f = c.font; P(k + c.coordinate + ".font", (f.name, f.sz, f.b, f.i, f.u, f.strike, f.color.rgb if f.color and f.color.type == "rgb" else None))
                    fl = c.fill; P(k + c.coordinate + ".fill", (fl.tagname, getattr(fl, "patternType", None), getattr(getattr(fl, "fgColor", None), "rgb", None), getattr(getattr(fl, "bgColor", None), "rgb", None) if getattr(fl, "patternType", None) not in (None, "solid") else None, getattr(fl, "degree", None)))
                    bd = c.border; P(k + c.coordinate + ".border", tuple((getattr(bd, e).style, getattr(getattr(bd, e).color, "rgb", None) if getattr(bd, e).style else None) for e in ("left", "right", "top", "bottom", "diagonal")) + (bd.diagonalUp, bd.diagonalDown))
                    al = c.alignment; P(k + c.coordinate + ".align", (al.horizontal, al.vertical, al.wrap_text, al.text_rotation, al.indent, al.shrink_to_fit))
                    P(k + c.coordinate + ".nf", c.number_format); P(k + c.coordinate + ".prot", (c.protection.locked, c.protection.hidden))
                if c.hyperlink: P(k + c.coordinate + ".link", (c.hyperlink.target, c.hyperlink.location))
                if c.comment: P(k + c.coordinate + ".note", (c.comment.text, c.comment.author))
        P(k + "merged", sorted(str(m) for m in s.merged_cells.ranges))
        P(k + "colw", sorted((kk, round(d.width or 0, 2), d.hidden, d.outlineLevel) for kk, d in s.column_dimensions.items() if d.width or d.hidden or d.outlineLevel))
        P(k + "rowh", sorted((kk, d.height, d.hidden, d.outlineLevel) for kk, d in s.row_dimensions.items() if d.height or d.hidden or d.outlineLevel))
        P(k + "freeze", s.freeze_panes); P(k + "tab", s.sheet_properties.tabColor.rgb if s.sheet_properties.tabColor else None)
        P(k + "view", (s.sheet_view.zoomScale, s.sheet_view.showGridLines))
        P(k + "dv", sorted((str(d.sqref), d.type, d.operator, d.formula1, d.formula2, d.errorStyle, d.errorTitle, d.error, d.promptTitle, d.prompt, d.allow_blank) for d in s.data_validations.dataValidation))
        cf = []
        for rng, rules in s.conditional_formatting._cf_rules.items():
            for r in rules: cf.append((str(rng.sqref), r.type, r.operator, tuple(r.formula or ()), r.stopIfTrue, r.priority, bool(r.colorScale), bool(r.dataBar), r.iconSet.iconSet if r.iconSet else None, (r.dxf.font.b, r.dxf.font.color.rgb if r.dxf.font and r.dxf.font.color else None, r.dxf.fill.bgColor.rgb if r.dxf.fill else None) if r.dxf and r.dxf.font else (None, None, r.dxf.fill.bgColor.rgb if r.dxf and r.dxf.fill else None)))
        P(k + "cf", sorted(cf, key=str))
        P(k + "filter", (s.auto_filter.ref, [(fc.colId, list(fc.filters.filter) if fc.filters else None) for fc in s.auto_filter.filterColumn]))
        P(k + "print", (s.print_area, s.print_title_rows, s.page_setup.orientation, s.page_setup.paperSize, s.page_setup.fitToWidth, s.page_setup.fitToHeight, s.sheet_properties.pageSetUpPr.fitToPage if s.sheet_properties.pageSetUpPr else None, s.page_margins.left, s.page_margins.top, s.oddHeader.center.text, s.oddFooter.left.text, s.print_options.gridLines, s.print_options.horizontalCentered))
        P(k + "breaks", ([b.id for b in s.row_breaks.brk], [b.id for b in s.col_breaks.brk]))
        P(k + "tables", sorted((t.displayName, t.ref, t.tableStyleInfo.name if t.tableStyleInfo else None, t.tableStyleInfo.showRowStripes if t.tableStyleInfo else None) for t in s.tables.values()))
        P(k + "protect", (s.protection.sheet, bool(s.protection.password or s.protection.hashValue), s.protection.formatCells))
        P(k + "charts", sorted((type(c).__name__, c.anchor._from.col, c.anchor._from.row, len(c.series), "".join(r.t for p in c.title.tx.rich.p for r in (p.r or [])) if c.title and c.title.tx and c.title.tx.rich else None) for c in s._charts))
    return out
a = dump("/tmp/b6/rt_in.xlsx")
bk = core.Book.open("/tmp/b6/rt_in.xlsx")
print("opened: F7 =", bk.value(0, 6, 5), " Second!A1 =", bk.value(1, 0, 0), " Second!C3 =", bk.value(1, 2, 2), " Tbl!D1 =", bk.value(2, 0, 3), " Tbl!D2 =", bk.value(2, 1, 3))
bk.save("/tmp/b6/rt_out.xlsx")
z = dump("/tmp/b6/rt_out.xlsx")
n = 0
for k in sorted(set(a) | set(z)):
    if a.get(k) != z.get(k):
        n += 1; print("RT  %s\n      in : %r\n      out: %r" % (k, a.get(k), z.get(k)))
print("round trip: %d keys, %d differ" % (len(a), n))
