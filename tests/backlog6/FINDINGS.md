# Backlog 6 — Folio Sheet 0.22.0 (master 48e3248) — findings so far
Probes in /home/claude/b6 (copied to /srv/jammy/tmp/b6). Engine built to /srv/jammy/tmp/fs.

## Confirmed
1. CRITICAL save: file from openpyxl (prefix r declared on the element, not on <worksheet>) with a table / chart / picture -> Sheet saves r:id with no xmlns:r on the root: not well-formed XML. openpyxl refuses it ("unbound prefix"); Excel will call it corrupt. Sheet and LibreOffice (lenient) still read it. (rt.py, sheets 3 and 4). NOTE: jammy chroot had no LibreOffice Calc; installed libreoffice-calc-nogui from the archive for checks.
2. CRITICAL: number format starting with a decimal point (".00", ".0#") panics numfmt.rs:884 (slice 1..0) -- TEXT() and a cell's display both. (fmt.py, p3.py)
3. Typing +1+1 stays text; Excel makes it the formula =+1+1 (also -A1, +A1?). (fmt.py)
4. DATE(1900,2,29) gives 61 (Excel 60); typing 2/29/1900 stays text (Excel: serial 60).
5. XLOOKUP with an array of lookup values returns only the first (SUM(XLOOKUP({1,2},A1:A5,B1:B5)) = 10, Excel 30).
6. INDEX(A1:B5,{1,2},2) returns one value, Excel an array.
7. ISOMITTED of an omitted LAMBDA parameter -> #VALUE! (Excel TRUE).
8. LINEST(B1:B5,A1:A5) slope 9.99999999999999 (Excel exactly 10).
9. Missing functions: GETPIVOTDATA, ENCODEURL, FILTERXML, AMORDEGRC, AMORLINC, ODDFPRICE, ODDFYIELD, ODDLPRICE, ODDLYIELD, REGEXTEST, REGEXEXTRACT, REGEXREPLACE, GROUPBY, PIVOTBY, PERCENTOF, TRIMRANGE, EUROCONVERT, IMAGE (cloud/locale ones left out on purpose: CUBE*, RTD, WEBSERVICE, STOCKHISTORY, TRANSLATE, DETECTLANGUAGE, ASC, DBCS, JIS, PHONETIC, BAHTTEXT).
10. Typing 1,23 / 1,2,3 / 12,34.5 stays text (Excel: 123 / 123 / 1234.5). low.

## Baseline (source, jammy): two failures to look into
- backlog4_audit: "S3-17: Create New offers the kinds this computer can make" []
- chrome_audit: "ribbon body is 96 px (+-1)" got 108

## To verify / low confidence
- TEXT(0.5,"# ?/?") two leading spaces (Excel one?)
11. Copying =A1+B1 from C1 to A1 gives =XFC1+XFD1 (wraps round the sheet); Excel gives =#REF!+#REF!.
12. Fill handle: Q1 continues Q5 (Excel wraps to Q1); 1st -> 2st, 3st (Excel 2nd, 3rd); a time 10:00 just copies (Excel adds an hour); "1","a" selected together repeats 1 (Excel 1,a,2,a); month ends 1/31,2/29 -> 3/29 (Excel 3/31) [last one medium confidence].
13. Engine set_merges keeps the values of the covered cells (SUM still counts them) -- check what the Merge command does in the UI.
Engine probes done: formulas.py (711, 15 diffs -> items 3-9), fmt.py, rt.py (round trip clean once the prefix is repaired), struct.py, p4.py (3-D refs fine).

## UI (ui_keys.py, ui_k2.py; harness uiharn.py + runui.sh, real X input via xdotool)
14. HIGH: typing =SUM(1,2 and Enter is refused ("There's a problem with this formula"); Excel adds the closing bracket itself. Same for =SUM(A2:A4 after pointing.
15. Shortcuts that do nothing (not in window.py on_key): Ctrl+; (date), Ctrl+Shift+: (time), Ctrl+' (formula above), Ctrl+Shift+" (value above), Ctrl+9 / Ctrl+0 (hide row/column), Ctrl+Shift+( / ) (unhide), Ctrl+Shift+* (current region), F12 (Save As), Enter to paste after a copy. [second batch to verify: F4 repeat, F11 chart sheet, Alt+;, Ctrl+[ ], Ctrl+F1, F1, F6, Ctrl+8, Ctrl+6]
16. Backspace on a cell makes its own undo step: Backspace, type 7, Enter, Ctrl+Z leaves the cell empty (Excel: back to the old value).
17. low: after Esc cancels the marching ants, Ctrl+V still pastes (Excel: nothing).
18. low: console floods with Gtk-WARNING "A tag named 'ref0' is already in the tag table" while a formula is typed (formula_edit tags created again and again).
Not bugs (my expectation wrong): Ctrl+A region stops at an empty column; =su Tab takes SUBSTITUTE (first in list); Ctrl+P opens backstage Print.
19. Mouse: double-clicking the fill handle does nothing (Excel fills down as far as the neighbouring column); dragging the selection's border does not move the cells (no hit kind for it in grid.py).
20. More shortcuts that do nothing: F4 / Ctrl+Y repeat last action, F11 (chart sheet), Alt+; (visible cells only), Ctrl+[ and Ctrl+] (precedents / dependents), Ctrl+F1 (collapse ribbon), F1 (help), Ctrl+8, Ctrl+2/3/4, Ctrl+Shift+O, Ctrl+F2 (print preview), Ctrl+Tab (next workbook).
21. CSV in Windows-1252 (what Excel's "CSV (Comma delimited)" writes) opens with accented letters as U+FFFD (window.py _import_text: utf-8-sig, errors=replace). UTF-16 "Unicode Text" files don't open as text at all.
22. Save As offers only xlsx/xlsm/xltx/xltm/csv -- no .txt (tab), .ods, .xls, PDF in the list (PDF is under Export). [decide]
Performance fine: 100k x 5 csv opens 2.0 s, sort 0.32 s, fill 100k formulas 0.66 s, save 1.3 s. Damaged files give a clean message.
23. HIGH rows never grow: Wrap Text, a bigger font (28 pt) and Alt+Enter all leave the row at 15 pt -- wrapped text shows one line, big text is clipped (rows.png). Alt+Enter also doesn't turn Wrap Text on (Excel does). Check files whose rows have no stored height.
24. HIGH filter: SUBTOTAL(9,..)/(3,..) still count rows the filter hides (Excel skips them); SUBTOTAL(109)/AGGREGATE only catch up after a full recalculation (filtering doesn't recalculate them).
25. HIGH filter: hidden (filtered) rows are not skipped by copy/paste (all rows pasted), Delete (clears hidden cells), Ctrl+Enter fill (fills hidden cells), Delete Rows (deletes the hidden rows too). Excel acts on visible rows only.
26. Typing 7 into a cell formatted as percent gives 700% (Excel: 7%).
27. No AutoComplete of an entry from the column above (type b under apple/banana -> Excel offers banana).
28. Typing a web or e-mail address does not make a link (Excel does).
29. Name Box: typing a new name with a range selected does not define it.
30. low: >15 significant digits typed are kept as typed (Excel cuts to 15: 123456789012345000); date+time typed shows "3/15/2024 2:30 PM" (Excel "3/15/2024 14:30"); pasting text "\"q,r\"" keeps the quotes.
HARNESS NOTE: xdotool type needs "--" before text starting with "-" (fixed in uiharn.py). Typing -5, -5%, -A2 works.
31. Drawing (gallery.xlsx, gal1.png/gal2.png): rotated text is not rotated when Wrap Text is also on; Shrink to Fit does not shrink (text clipped); pattern fills (stripes, grids, trellis) are drawn as a flat blend; gradient fills are drawn as their first colour; Distributed alignment is drawn left-aligned. All low except shrink/rotation (medium).
32. Item 23 also covers opened files: a row with no stored height that holds wrapped text or a 24 pt font stays 15 pt (files from openpyxl / pandas never store heights).
33. low: scatter chart from openpyxl (both axes axPos="l") loses its X axis title (chart_spec x_title '').
34. Tables: =ROWS(Table1) / a bare table name as a reference gives #NAME? (Excel: the data range). Tab in the last cell of a table does not add a row. Clicking table cells while typing a formula writes =B2*C2 (Excel =[@Qty]*[@Price]) [low]. Ctrl+Shift+T (total row) does nothing.
35. No AutoRecover / autosave: nothing in the app writes recovery copies; a crash loses everything since the last save.
36. Cutting a cell to another sheet (Ctrl+X, other sheet, Ctrl+V): formulas that used the cell keep pointing at the old, now empty, cell (Excel: =Two!E5+1). Same-sheet cut is fine.
37. Ctrl+Shift+Enter over a selection writes the formula separately into every cell (#SPILL!, #SPILL!, 2) instead of one array formula; and part of what should be an array can be overwritten. Engine has set_array_formula.
38. low: engine accepts column width 300 and row height 500 (Excel caps at 255 and 409) -- check the dialogs.
39. env: chrome_audit "ribbon body is 96 px" fails (108) where none of "Segoe UI","Noto Sans","Open Sans","Ubuntu" is installed (falls to DejaVu Sans) -- the bare jammy root; backlog4 S3-17 fails where no other Folio app is installed. Both will fail Sheet's own gate in this container.
40. low: TEXT(-1,"h:mm") and TEXT(2958466,"m/d/yyyy") give "" (Excel #VALUE!); BESSELY(2.5,1) is right to 8 digits only. formulas2.py: 508 checked against Excel's documented examples, nothing else wrong.
41. CSV is written as UTF-8 with no BOM: Excel opens it as ANSI and shows accented letters wrong; Excel's own "CSV UTF-8" writes a BOM. [low/decide]
42. Regional settings are not used anywhere: dates are always m/d/yyyy, currency $, decimal point "." (Excel follows the system region). [decide -- Product Owner is in NL]
Sheet-written files: 41 workbooks (the end state of every audit) all well-formed, relationships and content types right, openpyxl and LibreOffice open them. Hand-made Excel-style workbook (rt3.py) round trips; only phonetic runs are dropped. .ods/.xls open through the converter in ~1.5 s.
43. CRASH (segfault, stack overflow): a formula nested thousands deep kills the process -- "=" + 8000 "(" + 1 + 8000 ")" ; IF or SUM nested 6000 deep; 50000 unary minus. check_formula() alone crashes too. Reachable by pasting such text into a cell or opening a file that holds it. (p9.py)
44. A formula with more than 400 operands joined by + & * gives #NUM! (=A1+A1+... 401 terms, 1203 characters; Excel's limit is 8192 characters). Recursive LAMBDA stops at depth ~100-250 (Excel ~340 for two parameters) [low].
45. low: text holding a literal _x0041_ comes back as "A" after save and open (the underscore escape is not itself escaped on write). SUBSTITUTE/REPLACE may return more than 32,767 characters (Excel #VALUE!). BINOM.INV(1e9,0.5,0.5) takes 19 s, COMBINA(1e9,1e9) 5.5 s (linear loops; UI frozen meanwhile).
46. CRITICAL consequence of any engine panic (ui_panic.py, panic.png): (a) a cell with format ".00" makes Book.render panic on every draw -- the sheet area goes blank, the title bar is drawn twice, the tab row disappears; saving keeps the format, so the file stays undrawable. ".00" is an ordinary Excel custom format. (b) a formula that panics (=IMSUM(9.99e307,9.99e307), =MUNIT(1e15), ...) is stored, and after that every edit anywhere raises during recalculation: typing 5 in another cell leaves it empty. Panics found by fuzz2.py are listed below.
47. fuzz2.py (every function x 66 odd argument lists, one process each). PANICS (-> item 46b): IMSUM, IMDIV, IMPRODUCT, IMPOWER with huge numbers (unwrap on None); MUNIT(1e15) capacity overflow, MUNIT(1e308) index out of bounds; TEXTSPLIT with six cell arguments. PROCESS KILLED (app would vanish): DOLLAR(1e9,1e9) (out of memory), EXPAND(1e9,1e9), WRAPCOLS(1e9,1e9), WRAPROWS(1e9,1e9), MUNIT(45351) (allocation abort). HANGS > 25 s: COMBINA(1e308,1e308), PERMUT(1e308,1e308), FACTDOUBLE(1e308), NETWORKDAYS / NETWORKDAYS.INTL(1e308,1e308). 6208 number formats and 72,100 formula strings otherwise survived (only the leading-point format panics).
48. No support for workbooks encrypted with a password to open (Excel's File > Info > Encrypt): they are refused as "not an .xlsx package"; and Sheet cannot save one. [medium/decide]
49. An .xltx / .xltm template opens as itself (window.py NATIVE): Save writes over the template. Excel opens a new workbook based on it.
50. Charts: no bubble chart, no 3-D types, no pie-of-pie / bar-of-pie. Scatter of a block whose first column is text uses the first number column as X (Excel: 1,2,3...). [low]
