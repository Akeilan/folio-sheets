exec(open("/tmp/b6/uiharn.py").read())
focus(); S = g.sheet
def enter(r, c, text, how="Return"):
    at(r, c); typ(text); key(how)
rows = [("Item", "Qty", "Price"), ("apple", "2", "1.5"), ("pear", "3", "2"), ("fig", "10", "0.5")]
for r, row in enumerate(rows):
    for c, v in enumerate(row): b.set_input(S, r, c, v)
g.refresh(); at(1, 1); ANSWER[0] = Gtk.ResponseType.OK; DLG.clear(); key("ctrl+t"); pump(0.4)
tb = b.tables(S); info("after Ctrl+T, OK", (DLG, tb)); ck("Ctrl+T makes a table of the block", len(tb), 1)
def tref(): 
    t = b.tables(S); return tuple(t[0]["ref"]) if t and isinstance(t[0], dict) and "ref" in t[0] else (t[0] if t else None)
info("table record", b.tables(S)[0] if b.tables(S) else None)
r0 = tref()
enter(4, 0, "kiwi"); pump(0.2); ck("typing under the table makes it one row longer", tref(), (0, 0, 4, 2) if r0 == (0, 0, 3, 2) else "?")
enter(0, 3, "Total"); pump(0.2); ck("typing beside the header adds a column", tref(), (0, 0, 4, 3))
at(1, 3); typ("="); click(1, 1); typ("*"); click(1, 2); pump(0.1); txt = g.editor.text() if g.editing() else None; key("Return"); pump(0.3)
info("formula made by clicking inside the table (Excel: =[@Qty]*[@Price])", txt); ck("a formula typed in a table column fills the column", [V(r, 3) for r in (1, 2, 3)], [3.0, 6.0, 5.0])
enter(4, 1, "4", "Tab"); typ("0.25"); key("Tab"); pump(0.2); ck("the new row's calculated column fills in", V(4, 3), 1.0)
at(4, 3); key("Tab"); pump(0.3); ck("Tab in the last cell adds a row", tref(), (0, 0, 5, 3)); key("ctrl+z")
enter(8, 0, "=SUM(Table1[Qty])"); ck("structured reference outside the table", V(8, 0), 19.0); enter(9, 0, "=SUM(Table1[[#All],[Qty]])"); ck("[#All] counts the header as text", V(9, 0), 19.0)
enter(10, 0, "=Table1[[#Headers],[Qty]]"); ck("[#Headers]", V(10, 0), "Qty"); enter(11, 0, "=COUNTA(Table1[#Data])"); ck("[#Data]", V(11, 0), 16.0); enter(12, 0, "=ROWS(Table1)"); ck("ROWS(Table1)", V(12, 0), 4.0)
at(1, 1); key("ctrl+shift+t"); pump(0.3); info("Ctrl+Shift+T total row: table", b.tables(S)[0] if b.tables(S) else None); info("total row cells", [I(5, c) for c in range(4)])
b.rename_table(S, 0, "Sales") if hasattr(b, "rename_table") and False else None
# header rename flows into formulas
enter(0, 1, "Count"); pump(0.2); ck("renaming a header updates the formulas that name it", I(8, 0), "=SUM(Table1[Count])")
# delete a table row via the cells
# --- validation
b.clear_contents(S, [(0, 6, 30, 9)])
b.dv_set(S, [(0, 6, 5, 6)], {"type": "list", "formula1": '"red,green,blue"', "allow_blank": True, "show_error": True, "error_style": "stop"}) if False else None
info("dv_set doc", (type(b).dv_set.__doc__ if hasattr(type(b), "dv_set") else None, b.dv_at(S, 0, 6)))
done()
