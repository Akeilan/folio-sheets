exec(open("/tmp/b6/uiharn.py").read())
focus(); S = g.sheet
def fresh():
    global S
    if g.editing(): g.cancel_edit()
    key("Escape"); n = b.sheet_count; b.insert_sheet(n, "T%d" % n); win.tabs.rebuild(); g.set_sheet(n); win.sheet_changed(); S = n; g.select_cell(0, 0); pump(0.1)
def enter(r, c, text, how="Return"):
    at(r, c); typ(text); key(how)
def R1(cmd, *a):
    MSG.clear(); DLG.clear()
    try: win.run(cmd, *a)
    except Exception as e: print("FAIL %s raised %s" % (cmd, e), flush=True); N[1] += 1
    pump(0.15)
# --- paste variants
fresh()
for r in range(3): b.set_input(S, r, 0, str(r + 1)); b.set_input(S, r, 1, "=A%d*10" % (r + 1))
g.refresh(); at(0, 0); key("ctrl+b"); g.select_range((0, 0, 2, 1), (0, 0)); key("ctrl+c")
at(5, 0); R1("paste-values"); ck("Paste Values turns formulas into their values", (I(5, 1), V(7, 1)), ("10", 30.0)); ck("...without the format", b.cell_style(S, 5, 0).get("bold"), False)
at(10, 0); R1("paste-transpose"); ck("Paste Transpose turns the block", (V(10, 2), V(11, 0)), (3.0, 10.0)); info("transposed formula", I(11, 0))
at(15, 0); R1("paste-formatting"); ck("Paste Formatting brings only the format", (V(15, 0), b.cell_style(S, 15, 0).get("bold")), (None, True))
at(20, 0); R1("paste-link"); ck("Paste Link writes references", I(20, 0), "=A1" if I(20, 0) == "=A1" else "=$A$1"); info("paste link", [I(20, 0), I(20, 1)])
key("Escape")
# across sheets
P = S; b.insert_sheet(b.sheet_count, "Two"); win.tabs.rebuild(); Q = b.sheet_count - 1
at(0, 1); key("ctrl+c"); g.set_sheet(Q); win.sheet_changed(); at(0, 1); key("ctrl+v"); ck("a formula copied to another sheet points at that sheet's cells", I(0, 1, Q), "=A1*10"); key("Escape")
g.set_sheet(P); win.sheet_changed(); b.set_input(P, 0, 4, "=A1+1"); at(0, 0); key("ctrl+x"); g.set_sheet(Q); win.sheet_changed(); at(4, 4); key("ctrl+v"); pump(0.2)
ck("a cell cut to another sheet: formulas that used it follow", I(0, 4, P), "=Two!E5+1"); info("the moved cell on Two", V(4, 4, Q)); g.set_sheet(P); win.sheet_changed(); key("ctrl+z"); pump(0.1); info("after undoing the cross-sheet move: source A1, Two!E5", (V(0, 0, P), V(4, 4, Q)))
# --- typing into a text-formatted cell
fresh(); b.set_number_format(S, [(0, 0, 3, 0)], "@"); enter(0, 0, "00123"); ck("a Text cell keeps leading zeros", V(0, 0), "00123"); enter(1, 0, "=1+1"); ck("a formula typed into a Text cell stays text", V(1, 0), "=1+1"); enter(2, 0, "1/2"); ck("1/2 typed into a Text cell stays 1/2", V(2, 0), "1/2")
# --- array formula entry
fresh(); 
for r in range(3): b.set_input(S, r, 0, str(r + 1))
g.refresh(); g.select_range((0, 2, 2, 2), (0, 2)); pump(0.05); typ("=A1:A3*2"); key("ctrl+shift+Return"); pump(0.2); ck("Ctrl+Shift+Enter enters an array formula over the selection", [V(r, 2) for r in range(3)], [2.0, 4.0, 6.0]); info("array range / input", (b.array_range(S, 1, 2), I(1, 2)))
at(1, 2); typ("9"); MSG.clear(); key("Return"); pump(0.2); ck("part of an array can't be changed", (V(1, 2), bool(MSG) or bool(g.editing())), (4.0, True)); info("message", MSG[:1])
if g.editing(): key("Escape")
# --- decimals, comma, percent buttons
fresh(); b.set_input(S, 0, 0, "1234.5"); at(0, 0); R1("decimal-increase"); ck("Increase Decimal", D(0, 0), "1234.50"); R1("decimal-decrease"); R1("decimal-decrease"); ck("Decrease Decimal twice", D(0, 0), "1235"); R1("comma"); ck("Comma Style", D(0, 0).strip(), "1,234.50"); R1("percent"); ck("Percent Style", D(0, 0), "123450%")
# --- limits
fresh(); MSG.clear(); 
try: b.set_col_width(S, 0, 0, 300); info("column width 300 accepted ->", b.col_width(S, 0))
except Exception as e: info("column width 300", str(e)[:60])
try: b.set_row_height(S, 0, 0, 500); info("row height 500 accepted ->", b.row_height(S, 0))
except Exception as e: info("row height 500", str(e)[:60])
b.set_col_width(S, 0, 0, 8.43); b.set_row_height(S, 0, 0, 15)
# --- sheet names
for bad in ("a/b", "a:b", "a*b", "a?b", "a[b]", "x" * 32, "Two", "", "'lead", "History"):
    try: err = b.check_sheet_name(bad, 0)
    except Exception as e: err = "EXC " + str(e)[:40]
    print(("PASS " if err else "FAIL ") + "sheet name %r is refused%s" % (bad[:12], "" if err else "   accepted"), flush=True); N[0] += 1; N[1] += not err
# --- find
fresh(); b.set_input(S, 0, 0, "10"); b.set_input(S, 1, 0, "=5+5"); b.set_input(S, 2, 0, "110"); b.set_input(S, 3, 0, "=A1&\"x\"")
ck("Find 10 in values finds the formula's result too", sorted(b.find_all([S], "10", False, False, False)), [(S, 0, 0), (S, 1, 0), (S, 2, 0), (S, 3, 0)]); ck("Find 10, whole cell, in values", sorted(b.find_all([S], "10", False, True, False)), [(S, 0, 0), (S, 1, 0)])
ck("Find 5+5 in formulas", b.find_all([S], "5+5", False, False, True), [(S, 1, 0)])
# --- go to special blanks then Ctrl+Enter
fresh(); b.set_input(S, 0, 0, "a"); b.set_input(S, 2, 0, "b"); b.set_input(S, 4, 0, "c"); g.refresh(); g.select_range((0, 0, 4, 0), (0, 0)); sp = b.special_cells(S, (0, 0, 4, 0), "blanks"); info("special_cells blanks", sp)
# --- text to columns with the defaults answered OK
fresh(); b.set_input(S, 0, 0, "a,b,c"); b.set_input(S, 1, 0, "d,e,f"); g.refresh(); g.select_range((0, 0, 1, 0), (0, 0)); ANSWER[0] = Gtk.ResponseType.OK; R1("text-to-columns"); info("text to columns (dialog answered OK): row 1", ([V(0, c) for c in range(3)], DLG, MSG[:1]))
# --- flash fill
fresh(); 
for r, n in enumerate(["John Smith", "Mary Jones", "Ann Lee"]): b.set_input(S, r, 0, n)
b.set_input(S, 0, 1, "John"); g.refresh(); at(1, 1); R1("flash-fill"); ck("Flash Fill takes the first names", (V(1, 1), V(2, 1)), ("Mary", "Ann"))
# --- freeze panes
fresh(); at(1, 1); R1("freeze-panes"); st = g.state(); ck("Freeze Panes at B2", (st.freeze_rows, st.freeze_cols), (1, 1)); at(9, 9); key("ctrl+Home"); ck("Ctrl+Home goes to the first unfrozen cell", A(), (1, 1)); R1("unfreeze")
# --- close with changes
fresh(); b.set_input(S, 0, 0, "dirty"); ck("the workbook knows it has changes", b.modified, True); DLG.clear(); MSG.clear(); ANSWER[0] = Gtk.ResponseType.CANCEL
try: r = win.act_close() if hasattr(win, "act_close") else win.run("close")
except Exception as e: r = "EXC " + str(e)
pump(0.3); info("Close with unsaved changes, answered Cancel: dialogs, still open", (DLG, MSG[:1], win.get_visible()))
done()
