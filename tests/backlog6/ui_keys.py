exec(open("/tmp/b6/uiharn.py").read())
focus()
# --- typing and where Enter / Tab go
at(0, 0); typ("hello"); key("Return"); ck("typing then Enter enters the text", V(0, 0), "hello"); ck("Enter moves down", A(), (1, 0))
typ("12"); key("Tab"); ck("Tab enters and moves right", (V(1, 0), A()), (12.0, (1, 1)))
typ("x"); key("Tab"); typ("y"); key("Return"); ck("Enter after Tabs returns to the column the row started in", A(), (2, 0))
at(4, 4); typ("a"); key("shift+Return"); ck("Shift+Enter moves up", A(), (3, 4)); typ("b"); key("shift+Tab"); ck("Shift+Tab moves left", A(), (3, 3))
at(0, 0); typ("zzz"); key("Escape"); ck("Esc cancels the entry", V(0, 0), "hello")
at(0, 0); key("F2"); typ(" there"); key("Return"); ck("F2 edits at the end of the cell", V(0, 0), "hello there")
at(0, 0); typ("new"); key("Return"); ck("typing over a cell replaces it", V(0, 0), "new")
at(0, 0); key("Delete"); ck("Delete clears", V(0, 0), None)
at(1, 0); key("BackSpace"); typ("7"); key("Return"); ck("Backspace clears and starts an entry", V(1, 0), 7.0)
at(1, 0); key("ctrl+z"); ck("Ctrl+Z undoes the entry", V(1, 0), 12.0); key("ctrl+y"); ck("Ctrl+Y redoes it", V(1, 0), 7.0)
at(6, 0); typ("=1+"); key("Return"); info("after typing =1+ and Enter: dialogs/messages, editing", (DLG[-2:], MSG[-2:], bool(g.editing()))); 
if g.editing(): key("Escape")
at(6, 0); typ("=SUM(1,2"); key("Return"); ck("a missing closing bracket is added", (I(6, 0), V(6, 0)), ("=SUM(1,2)", 3.0))
at(6, 1); typ("=sum(a2:a2)"); key("Return"); ck("function and reference names are upper-cased", I(6, 1), "=SUM(A2:A2)")
at(6, 2); typ("=a2*2"); key("Return"); ck("formula value", V(6, 2), 14.0)
at(7, 0); typ("line1"); key("alt+Return"); typ("line2"); key("Return"); ck("Alt+Enter puts a line break in the cell", V(7, 0), "line1\nline2")
# --- pointing while entering a formula
clear_sheet(); 
for r in range(5): b.set_input(g.sheet, r, 0, str(r + 1)); b.set_input(g.sheet, r, 1, str((r + 1) * 10))
g.refresh(); at(0, 3); typ("="); key("Left", "Left"); typ("+"); key("Left", "Left", "Left"); key("Return"); ck("arrow keys point at cells while a formula is typed", I(0, 3), "=B1+A1")
at(1, 3); typ("=SUM("); key("Left", "Left", "Left"); key("shift+Down", "shift+Down"); key("Return"); ck("Shift+arrows point at a range", I(1, 3), "=SUM(A2:A4)")
at(2, 3); typ("=A1"); key("F4"); key("Return"); ck("F4 makes the reference absolute", I(2, 3), "=$A$1")
at(3, 3); typ("=A1"); key("F4", "F4"); key("Return"); ck("F4 twice: row absolute", I(3, 3), "=A$1")
at(4, 3); typ("=A1"); key("F4", "F4", "F4"); key("Return"); ck("F4 three times: column absolute", I(4, 3), "=$A1")
at(5, 3); typ("="); click(2, 1); typ("*2"); key("Return"); ck("clicking a cell while typing a formula points at it", I(5, 3), "=B3*2")
at(6, 3); typ("=su"); pump(0.3); key("Tab"); typ("A1:A5"); key("Return"); ck("Tab takes the function from the list", I(6, 3), "=SUM(A1:A5)")
at(5, 0); key("alt+equal"); key("Return"); ck("Alt+= writes a SUM of the numbers above", I(5, 0), "=SUM(A1:A5)")
# --- moving about
at(0, 0); key("ctrl+Down"); ck("Ctrl+Down goes to the last cell of the block", A(), (5, 0)); key("ctrl+Down"); ck("Ctrl+Down again goes to the bottom of the sheet", A(), (1048575, 0))
key("ctrl+Home"); ck("Ctrl+Home", A(), (0, 0)); key("ctrl+End"); ck("Ctrl+End goes to the last used cell", A(), (6, 3))
at(2, 2); key("Home"); ck("Home goes to column A", A(), (2, 0)); key("ctrl+Right"); ck("Ctrl+Right", A(), (2, 1))
at(0, 0); key("Next"); pg = A()[0]; ck("Page Down moves a screenful", pg > 10, True); key("Prior"); ck("Page Up comes back", A(), (0, 0))
key("alt+Next"); ck("Alt+Page Down moves a screen right", A()[1] > 5, True); key("alt+Prior"); ck("Alt+Page Up comes back", A(), (0, 0))
at(0, 0); key("shift+Right", "shift+Down"); ck("Shift+arrows extend the selection", R(), [(0, 0, 1, 1)])
at(0, 0); key("ctrl+shift+Down"); ck("Ctrl+Shift+Down selects to the end of the block", R(), [(0, 0, 5, 0)])
at(1, 1); key("ctrl+a"); ck("Ctrl+A inside data selects the block", R(), [(0, 0, 6, 3)]); key("ctrl+a"); ck("Ctrl+A again selects the sheet", R(), [(0, 0, 1048575, 16383)])
at(2, 1); key("ctrl+space"); ck("Ctrl+Space selects the column", R(), [(0, 1, 1048575, 1)]); at(2, 1); key("shift+space"); ck("Shift+Space selects the row", R(), [(2, 0, 2, 16383)])
at(0, 0); key("shift+ctrl+End"); ck("Ctrl+Shift+End selects to the last used cell", R(), [(0, 0, 6, 3)])
at(1, 1); key("ctrl+shift+asterisk"); ck("Ctrl+Shift+* selects the current region", R(), [(0, 0, 6, 3)])
g.select_range((1, 0, 3, 1), (1, 0)); pump(0.05); key("Return", "Return", "Return"); ck("Enter walks inside a selection and wraps to the next column", A(), (1, 1)); ck("...and the selection stays", R(), [(1, 0, 3, 1)])
key("Tab"); ck("Tab inside a selection moves across then down", A(), (2, 0))
g.select_range((1, 0, 3, 1), (1, 0)); pump(0.05); typ("9"); key("ctrl+Return"); ck("Ctrl+Enter fills the whole selection", [V(r, c) for r in (1, 2, 3) for c in (0, 1)], [9.0] * 6); key("ctrl+z")
ck("...and one Ctrl+Z takes it all back", [V(1, 0), V(3, 1)], [2.0, 40.0])
# --- clipboard
at(0, 0); key("ctrl+c"); at(0, 5); key("ctrl+v"); ck("Ctrl+C / Ctrl+V", V(0, 5), 1.0); at(1, 5); key("ctrl+v"); ck("the copy can be pasted again", V(1, 5), 1.0)
key("Escape"); at(2, 5); key("ctrl+v"); pump(0.2); info("paste after Esc (Excel: nothing to paste)", V(2, 5))
at(0, 1); key("ctrl+x"); at(0, 6); key("ctrl+v"); ck("Ctrl+X / Ctrl+V moves the cell", (V(0, 1), V(0, 6)), (None, 10.0)); key("ctrl+z"); ck("undo of the move puts it back", (V(0, 1), V(0, 6)), (10.0, None))
at(1, 0); key("ctrl+c"); at(3, 5); key("Return"); ck("Enter pastes the copy", V(3, 5), 2.0); at(4, 5); key("Return"); ck("...once only", (V(4, 5), A()), (None, (5, 5)))
g.select_range((0, 0, 1, 1), (0, 0)); pump(0.05); key("ctrl+c"); g.select_range((8, 0, 11, 3), (8, 0)); pump(0.05); key("ctrl+v"); ck("a 2x2 copy tiles a 4x4 selection", [V(8, 0), V(11, 3), V(10, 2)], [1.0, 20.0, 1.0]); key("Escape")
b.clear_contents(g.sheet, [(8, 0, 11, 6)]); b.clear_contents(g.sheet, [(0, 5, 6, 6)]); g.refresh()
# --- fill and insert shortcuts
g.select_range((0, 1, 4, 1), (0, 1)); pump(0.05); key("ctrl+d"); ck("Ctrl+D fills down", [V(r, 1) for r in range(5)], [10.0] * 5); key("ctrl+z")
g.select_range((0, 0, 0, 2), (0, 0)); pump(0.05); key("ctrl+r"); ck("Ctrl+R fills right", [V(0, c) for c in range(3)], [1.0, 1.0, 1.0]); key("ctrl+z")
at(9, 0); key("ctrl+semicolon"); key("Return"); import datetime; ck("Ctrl+; enters today's date", V(9, 0), float((datetime.date.today() - datetime.date(1899, 12, 30)).days))
at(9, 1); key("ctrl+shift+colon"); key("Return"); ck("Ctrl+Shift+: enters the time", isinstance(V(9, 1), float) and 0 <= V(9, 1) < 1, True)
at(1, 3); key("ctrl+apostrophe"); key("Return"); ck("Ctrl+' copies the formula above without moving its references", I(1, 3), I(0, 3))
at(9, 3); key("ctrl+quotedbl"); key("Return"); info("Ctrl+Shift+\" copies the value above", V(9, 3))
# --- format shortcuts
at(0, 0); key("ctrl+b"); ck("Ctrl+B", b.cell_style(g.sheet, 0, 0).get("bold"), True); key("ctrl+i"); ck("Ctrl+I", b.cell_style(g.sheet, 0, 0).get("italic"), True); key("ctrl+u"); info("cell_style after B/I/U", b.cell_style(g.sheet, 0, 0))
key("ctrl+b", "ctrl+i", "ctrl+u")
for combo, want in (("ctrl+shift+dollar", "$"), ("ctrl+shift+percent", "0%"), ("ctrl+shift+numbersign", "d-mmm-yy"), ("ctrl+shift+at", "h:mm AM/PM"), ("ctrl+shift+exclam", "#,##0.00"), ("ctrl+shift+asciicircum", "0.00E+00"), ("ctrl+shift+asciitilde", "General")):
    at(1, 0); key(combo); nf = b.number_format(g.sheet, 1, 0); ck("%s gives a format with %s" % (combo, want), want in nf, True) if want not in nf else ck(combo, True)
    if want not in nf: info("   format was", nf)
at(1, 0); key("ctrl+5"); info("Ctrl+5 strikethrough", b.cell_style(g.sheet, 1, 0)); key("ctrl+5")
at(3, 0); key("ctrl+9"); ck("Ctrl+9 hides the row", b.row_hidden(g.sheet, 3), True); g.select_range((2, 0, 4, 0), (2, 0)); pump(0.05); key("ctrl+shift+parenleft"); ck("Ctrl+Shift+( unhides it", b.row_hidden(g.sheet, 3), False)
at(0, 2); key("ctrl+0"); hs = b.hidden_spans(g.sheet); info("Ctrl+0 hides the column: hidden_spans", hs); b.set_cols_hidden(g.sheet, 2, 2, False)
DLG.clear(); at(0, 0); key("ctrl+1"); ck("Ctrl+1 opens Format Cells", any("Format Cells" in d for d in DLG), True) if not any("Format Cells" in d for d in DLG) else ck("Ctrl+1 opens Format Cells", True)
for combo, title in (("ctrl+f", "Find"), ("ctrl+h", "Replace"), ("F5", "Go To"), ("ctrl+g", "Go To"), ("ctrl+k", "Hyperlink"), ("ctrl+t", "Table"), ("ctrl+l", "Table"), ("F12", "Save"), ("ctrl+o", "Open"), ("ctrl+p", "Print"), ("alt+F8", "Macro"), ("ctrl+F3", "Name Manager"), ("shift+F3", "Insert Function"), ("F7", "Spelling"), ("ctrl+alt+v", "Paste Special"), ("ctrl+shift+f", "Format Cells"), ("ctrl+q", "Quick Analysis")):
    DLG.clear(); MSG.clear(); at(1, 1); n0 = len(Gtk.Window.list_toplevels()); key(combo); pump(0.3)
    tops = [w.get_title() or "" for w in Gtk.Window.list_toplevels() if w.get_visible() and w is not win]
    hit = any(title.lower() in (d or "").lower() for d in DLG + tops + MSG)
    print(("PASS " if hit else "FAIL ") + "%s opens %s%s" % (combo, title, "" if hit else "   dialogs %r windows %r msgs %r" % (DLG, tops, MSG[:1])), flush=True); N[0] += 1; N[1] += not hit
    for w in Gtk.Window.list_toplevels():
        if w is not win and w.get_visible() and not isinstance(w, type(win)):
            try: w.close() if hasattr(w, "close") else w.hide()
            except Exception: pass
    key("Escape"); pump(0.1); focus()
# --- sheets
n0 = b.sheet_count; key("shift+F11"); ck("Shift+F11 inserts a sheet", b.sheet_count, n0 + 1); s1 = g.sheet; key("ctrl+Next"); ck("Ctrl+Page Down goes to the next sheet", g.sheet, (s1 + 1) % b.sheet_count if s1 + 1 < b.sheet_count else s1); key("ctrl+Prior"); ck("Ctrl+Page Up goes back", g.sheet, s1)
key("ctrl+grave"); info("Ctrl+` show formulas state", getattr(g, "show_formulas", None) if hasattr(g, "show_formulas") else g.params().get("show_formulas")); key("ctrl+grave")
done()
