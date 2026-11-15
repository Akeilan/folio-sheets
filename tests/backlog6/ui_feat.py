exec(open("/tmp/b6/uiharn.py").read())
focus()
S = g.sheet
def fresh():
    if g.editing(): g.cancel_edit()
    win.new_book() if False else None
    b.clear_contents(S, [(0, 0, 300, 30)])
    try: b.set_merges(S, [])
    except Exception: pass
    b.set_number_format(S, [(0, 0, 300, 30)], "General"); g.refresh(); pump(0.05)
def enter(r, c, text, how="Return"):
    at(r, c); typ(text); key(how)
# --- what typing does to the format
enter(0, 0, "3/15/2024"); ck("typing a date shows as a date", D(0, 0), "3/15/2024")
enter(0, 0, "5"); ck("a number typed over a date cell keeps the date format (Excel shows 1/5/1900)", D(0, 0), "1/5/1900")
enter(1, 0, "50%"); enter(1, 0, "7"); ck("typing 7 into a percent cell means 7%", (V(1, 0), D(1, 0)), (0.07, "7%"))
enter(2, 0, "$5.00"); ck("typing $5.00 gives currency", D(2, 0), "$5.00"); enter(3, 0, "1:30"); ck("typing 1:30 shows 1:30", D(3, 0), "1:30"); enter(4, 0, "1/2"); ck("typing 1/2 shows 2-Jan", D(4, 0), "2-Jan")
enter(5, 0, "1 1/2"); ck("typing 1 1/2 shows 1 1/2", D(5, 0), "1 1/2"); enter(6, 0, "1e5"); ck("typing 1e5 shows 1.00E+05", D(6, 0), "1.00E+05")
enter(7, 0, "123456789012345678"); ck("more than 15 digits are cut to 15", V(7, 0), 123456789012345000.0); enter(8, 0, "1,234.5"); ck("typing 1,234.5 shows with the separator", D(8, 0), "1,234.5")
enter(9, 0, "3/15/2024 2:30 PM"); ck("typing date and time", D(9, 0), "3/15/2024 14:30"); enter(10, 0, "12:30:45"); ck("typing h:mm:ss", D(10, 0), "12:30:45"); enter(11, 0, "1:30 PM"); ck("typing 1:30 PM", D(11, 0), "1:30 PM")
enter(12, 0, "March 2024"); ck("typing March 2024 shows Mar-24", D(12, 0), "Mar-24"); enter(13, 0, "15 Mar"); ck("typing 15 Mar shows 15-Mar", D(13, 0), "15-Mar")
enter(14, 0, "-5%"); ck("typing -5%", D(14, 0), "-5%"); enter(15, 0, "5.5%"); ck("typing 5.5% shows two decimals", D(15, 0), "5.50%")
# --- formats formulas pick up
enter(0, 2, "=A1+1"); info("=A1+1 where A1 shows as a date displays", D(0, 2)); enter(1, 2, "=A2*2"); info("=A2*2 where A2 is a percent displays", D(1, 2)); enter(2, 2, "=A3+A3"); info("=A3+A3 where A3 is currency displays", D(2, 2))
enter(3, 2, "=TODAY()"); ck("=TODAY() displays as a date", "/" in D(3, 2), True); enter(4, 2, "=NOW()"); ck("=NOW() displays date and time", ":" in D(4, 2) and "/" in D(4, 2), True); info("NOW shows", D(4, 2))
enter(5, 2, "=DATE(2024,3,15)"); ck("=DATE() displays as a date", D(5, 2), "3/15/2024"); enter(6, 2, "=TIME(6,30,0)"); ck("=TIME() displays as a time", D(6, 2), "6:30 AM")
enter(7, 2, "=SUM(A3,A3)"); info("=SUM over currency displays", D(7, 2)); enter(8, 2, "=EDATE(A10,1)"); info("=EDATE displays", D(8, 2)); enter(9, 2, "=A4*24"); info("=A4*24 (time x 24) displays", D(9, 2))
# --- narrow columns
fresh(); b.set_input(S, 0, 0, "123456789"); b.set_input(S, 1, 0, "3/15/2024"); b.set_input(S, 2, 0, "0.123456789"); b.set_input(S, 3, 0, "a long piece of text"); b.set_input(S, 4, 0, "1234.5678"); b.set_number_format(S, [(4, 0, 4, 0)], "#,##0.00")
b.set_col_width(S, 0, 0, 4); g.refresh(); pump(0.1); win.present()
import cairo
def shown(r, c):
    for nm in ("cell_text", "shown_text", "display_fit"):
        if hasattr(b, nm): return getattr(b, nm)(S, r, c)
    return None
info("narrow column API", [m for m in dir(b._book if hasattr(b, "_book") else b) if "fit" in m or "shown" in m][:8])
# --- merge
fresh(); 
for (r, c), v in {(0, 0): "1", (0, 1): "2", (1, 0): "3", (1, 1): "4"}.items(): b.set_input(S, r, c, v)
b.set_input(S, 5, 0, "=SUM(A1:B2)"); g.select_range((0, 0, 1, 1), (0, 0)); ANSWER[0] = Gtk.ResponseType.OK; MSG.clear(); DLG.clear(); win.run("merge-center"); pump(0.2)
info("merge of four filled cells: warning shown", (MSG[:1], DLG[:1])); ck("merging keeps only the upper-left value", (V(0, 1), V(1, 0), V(1, 1), V(5, 0)), (None, None, None, 1.0))
key("ctrl+z"); ck("undo of the merge brings the values back", (V(0, 1), V(1, 1), b.merges(S)), (2.0, 4.0, []))
# --- sort from the ribbon with a header row
fresh(); rows = [("Name", "Score"), ("pear", "7"), ("apple", "12"), ("fig", "3")]
for r, row in enumerate(rows):
    for c, v in enumerate(row): b.set_input(S, r, c, v)
g.refresh(); at(2, 1); win.run("sort-desc" if "sort-desc" in __import__("folio_sheets.commands", fromlist=["x"]).LABELS else "sort-za"); pump(0.2)
ck("Sort Z-A on a column sorts the block by it and leaves the header", [D(r, 0) for r in range(4)], ["Name", "apple", "pear", "fig"])
at(1, 0); win.run("sort-asc" if "sort-asc" in __import__("folio_sheets.commands", fromlist=["x"]).LABELS else "sort-az"); pump(0.2); ck("Sort A-Z on the name column", [D(r, 0) for r in range(4)], ["Name", "apple", "fig", "pear"])
# --- filter and what works on visible rows only
at(0, 0); win.run("filter"); pump(0.2); info("autofilter", b.autofilter(S)); b.filter_column(S, 0, {"values": ["apple", "pear"], "blank": False}); g.refresh(); pump(0.1)
ck("filtered row is hidden", b.row_hidden(S, 2), True); b.set_input(S, 6, 1, "=SUBTOTAL(9,B2:B4)"); b.set_input(S, 7, 1, "=SUM(B2:B4)"); ck("SUBTOTAL skips filtered rows, SUM does not", (V(6, 1), V(7, 1)), (19.0, 22.0))
g.select_range((1, 0, 3, 1), (1, 0)); pump(0.05); key("ctrl+c"); at(10, 3); key("ctrl+v"); pump(0.2); ck("copying a filtered range pastes only the visible rows", [D(10, 3), D(11, 3), D(12, 3)], ["apple", "pear", ""]); key("Escape")
g.select_range((1, 1, 3, 1), (1, 1)); pump(0.05); key("Delete"); ck("Delete on a filtered range clears only visible cells", V(2, 1), 3.0); key("ctrl+z")
g.select_range((1, 2, 3, 2), (1, 2)); pump(0.05); typ("x"); key("ctrl+Return"); ck("Ctrl+Enter on a filtered range fills only visible cells", (V(1, 2), V(2, 2), V(3, 2)), ("x", None, "x")); key("ctrl+z")
g.select_range((1, 0, 3, 16383), (1, 0)); win.run("delete-rows"); pump(0.2); info("after deleting the visible filtered rows, column A", [D(r, 0) for r in range(4)]); key("ctrl+z")
win.run("filter"); pump(0.1)
# --- autocomplete of entries
fresh(); 
for r, v in enumerate(["apple", "banana", "apricot"]): b.set_input(S, r, 0, v)
g.refresh(); at(3, 0); typ("b"); pump(0.3); info("typing b under apple/banana/apricot: editor shows", g.editor.text() if g.editing() else None); key("Return"); ck("AutoComplete finishes the entry from the column", V(3, 0), "banana")
at(4, 0); typ("ap"); pump(0.3); key("Return"); ck("an ambiguous start is not completed", V(4, 0), "ap")
# --- typing a link
fresh(); enter(0, 0, "https://example.com/page"); ck("typing a web address makes a link", b.link_at(S, 0, 0) is not None, True); enter(1, 0, "someone@example.com"); ck("typing an e-mail address makes a link", b.link_at(S, 1, 0) is not None, True)
# --- inserted rows take the format above
fresh(); b.set_input(S, 0, 0, "x"); at(0, 0); key("ctrl+b"); g.select_range((1, 0, 1, 16383), (1, 0)); win.run("insert-rows"); pump(0.1); ck("an inserted row takes the format of the row above", b.cell_style(S, 1, 0).get("bold"), True); key("ctrl+z"); at(0, 0); key("ctrl+b")
# --- wrap text grows the row; bigger font grows the row
fresh(); h0 = b.row_height(S, 0); b.set_input(S, 0, 0, "one two three four five six seven eight nine ten eleven twelve"); at(0, 0); win.run("wrap-text"); pump(0.2); ck("Wrap Text makes the row taller", b.row_height(S, 0) > h0 * 1.9, True); info("row heights", (h0, b.row_height(S, 0)))
at(2, 0); b.set_input(S, 2, 0, "big"); win.run("font-size:28"); pump(0.2); ck("a bigger font makes the row taller", b.row_height(S, 2) > h0 * 1.9, True)
# --- AutoSum choices
fresh()
for r, v in enumerate(["1", "2", "", "4", "5"]): b.set_input(S, r, 1, v)
for c, v in enumerate(["10", "20", "30"]): b.set_input(S, 8, c, v)
g.refresh(); at(5, 1); win.run("autosum"); pump(0.2); info("AutoSum under 1,2,blank,4,5 proposes", g.editor.text() if g.editing() else I(5, 1)); key("Escape")
at(8, 3); win.run("autosum"); pump(0.2); ck("AutoSum at the end of a row sums the row", g.editor.text() if g.editing() else I(8, 3), "=SUM(A9:C9)"); key("Escape")
g.select_range((0, 1, 5, 1), (0, 1)); win.run("autosum"); pump(0.2); info("AutoSum with the column and a blank cell selected writes", I(5, 1)); key("ctrl+z")
# --- name box
fresh(); nb = win.formula_bar.namebox; ent = nb.get_child()
g.select_range((0, 0, 2, 1), (0, 0)); ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("MyRange"); key("Return"); pump(0.2); ck("typing a name in the Name Box defines it", [n[0] for n in b.names()], ["MyRange"])
at(5, 5); ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("C7"); key("Return"); pump(0.2); ck("typing a reference in the Name Box goes there", A(), (6, 2))
ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("myrange"); key("Return"); pump(0.2); ck("typing the name selects its range", R(), [(0, 0, 2, 1)])
ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("B2:D4"); key("Return"); pump(0.2); ck("a range typed in the Name Box is selected", R(), [(1, 1, 3, 3)])
ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("A:A"); key("Return"); pump(0.2); info("A:A in the Name Box", R()); ent.grab_focus(); pump(0.1); key("ctrl+a"); typ("zz!!"); MSG.clear(); key("Return"); pump(0.2); info("nonsense in the Name Box", MSG[:1]); key("Escape"); focus()
# --- pasting text from another program
fresh(); cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD); cb.set_text("a\tb\t3\n1,5\t=1+1\t3/15/2024\n\"q,r\"\tx\ty\n", -1); cb.store(); pump(0.3); at(0, 0); key("ctrl+v"); pump(0.3)
ck("tab-separated text pastes into cells", (V(0, 0), V(0, 2), V(1, 2), V(2, 1)), ("a", 3.0, 45366.0, "x")); info("pasted row 2", [I(1, c) for c in range(3)]); info("pasted row 3 first cell", V(2, 0))
done()
