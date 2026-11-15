exec(open("/tmp/b6/uiharn.py").read())
import subprocess
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill, Border, Side, Alignment, GradientFill
wb = Workbook(); ws = wb.active
def put(ref, v, **kw):
    c = ws[ref]; c.value = v
    for k, x in kw.items(): setattr(c, k, x)
ws.row_dimensions[1].height = 60; ws.row_dimensions[2].height = 60; ws.row_dimensions[3].height = 45
for col in "ABCDEFGH": ws.column_dimensions[col].width = 14
put("A1", "rot 45", alignment=Alignment(text_rotation=45)); put("B1", "rot 90", alignment=Alignment(text_rotation=90)); put("C1", "rot -90", alignment=Alignment(text_rotation=180))
put("D1", "stack", alignment=Alignment(text_rotation=255)); put("E1", "rot45 wrap", alignment=Alignment(text_rotation=45, wrap_text=True)); put("F1", 1.25, alignment=Alignment(text_rotation=45, horizontal="center", vertical="top"))
put("G1", "rot 30 border", alignment=Alignment(text_rotation=30), border=Border(left=Side(style="thin"), right=Side(style="thin"), top=Side(style="thin"), bottom=Side(style="thin")))
put("A2", "top", alignment=Alignment(vertical="top")); put("B2", "middle", alignment=Alignment(vertical="center")); put("C2", "bottom", alignment=Alignment(vertical="bottom")); put("D2", "wrap this longer text please now", alignment=Alignment(wrap_text=True, vertical="top"))
put("E2", "shrink this long text to fit", alignment=Alignment(shrink_to_fit=True)); put("F2", "indent 2", alignment=Alignment(horizontal="left", indent=2)); put("G2", "r indent 1", alignment=Alignment(horizontal="right", indent=1)); put("H2", "justify these few words across", alignment=Alignment(horizontal="justify"))
put("A3", "center across", alignment=Alignment(horizontal="centerContinuous")); put("B3", None, alignment=Alignment(horizontal="centerContinuous")); put("C3", None, alignment=Alignment(horizontal="centerContinuous"))
put("D3", "fill", alignment=Alignment(horizontal="fill")); put("E3", "dist ri bu ted", alignment=Alignment(horizontal="distributed")); put("F3", "a\nb\nc", alignment=Alignment(wrap_text=True, vertical="center"))
pats = ["solid", "darkGray", "mediumGray", "lightGray", "gray125", "gray0625", "darkHorizontal", "darkVertical", "darkDown", "darkUp", "darkGrid", "darkTrellis", "lightHorizontal", "lightVertical", "lightDown", "lightUp", "lightGrid", "lightTrellis"]
for i, p in enumerate(pats):
    c = ws.cell(row=5 + i // 8, column=1 + i % 8); c.value = p; c.fill = PatternFill(p, fgColor="C00000", bgColor="FFFF00")
put("C7", "grad lin", fill=GradientFill(stop=("FF0000", "0000FF"), degree=0)); put("D7", "grad 90", fill=GradientFill(stop=("FFFFFF", "00B050"), degree=90)); put("E7", "grad path", fill=GradientFill(type="path", stop=("FFFF00", "7030A0"), left=0.5, right=0.5, top=0.5, bottom=0.5))
styles = ["thin", "medium", "thick", "double", "dotted", "dashed", "hair", "mediumDashed", "dashDot", "mediumDashDot", "dashDotDot", "mediumDashDotDot", "slantDashDot"]
for i, s in enumerate(styles):
    c = ws.cell(row=9 + (i // 7) * 2, column=1 + i % 7); c.value = s; sd = Side(style=s, color="0000FF"); c.border = Border(left=sd, right=sd, top=sd, bottom=sd)
put("A13", "diag up", border=Border(diagonal=Side(style="thin", color="FF0000"), diagonalUp=True)); put("B13", "diag both", border=Border(diagonal=Side(style="medium"), diagonalUp=True, diagonalDown=True))
put("A15", "single u", font=Font(underline="single")); put("B15", "double u", font=Font(underline="double")); put("C15", "acct u", font=Font(underline="singleAccounting")); put("D15", "strike", font=Font(strike=True)); put("E15", "super", font=Font(vertAlign="superscript")); put("F15", "sub", font=Font(vertAlign="subscript")); put("G15", "outline?", font=Font(outline=True, shadow=True)); put("H15", "Courier 9", font=Font(name="Courier New", size=9))
put("A17", "text overflows into the next empty cells to the right"); put("D17", "x"); put("E18", "right-aligned text overflows to the left", alignment=Alignment(horizontal="right")); put("C19", "centred text overflows both ways equally", alignment=Alignment(horizontal="center"))
put("A20", 1234567.891); put("B20", -1234.5, number_format='#,##0.00_);[Red](#,##0.00)'); put("C20", 1234.5, number_format='_($* #,##0.00_);_($* (#,##0.00);_($* "-"??_);_(@_)'); put("D20", 0, number_format='_($* #,##0.00_);_($* (#,##0.00);_($* "-"??_);_(@_)'); put("E20", 0.5, number_format="# ?/?"); put("F20", True); put("G20", "=1/0"); put("H20", 12, number_format='0" units"')
ws.column_dimensions["I"].width = 4; put("I20", 123456789); put("I21", 0.123456789); put("I22", "2024-03-15"); ws["I23"] = __import__("datetime").date(2024, 3, 15); put("A22", -5, number_format='0.00;[Blue]-0.00'); put("B22", 5, number_format='"x"*-0'); put("C22", 5, number_format='0*.')
ws.merge_cells("A24:C25"); put("A24", "merged centre", alignment=Alignment(horizontal="center", vertical="center")); put("E24", "a rather long text in a cell that is formatted to wrap but whose row has no height", alignment=Alignment(wrap_text=True)); put("F24", "tall font", font=Font(size=24))
wb.save("/tmp/b6/gallery.xlsx")
win.load("/tmp/b6/gallery.xlsx"); pump(0.5); focus(); g = win.grid
st = g.state(); st.zoom = 1.5; g.select_cell(27, 9); g.select_cell(0, 9); g.refresh(); pump(0.8); subprocess.run(["import", "-window", "root", "/tmp/b6/gal1.png"])
g.select_cell(30, 9); g.refresh(); pump(0.8); subprocess.run(["import", "-window", "root", "/tmp/b6/gal2.png"])
info("row heights 24, 25 (no height stored; wrap + 24pt font)", (win.book.row_height(0, 23), win.book.row_height(0, 24)))
done()
