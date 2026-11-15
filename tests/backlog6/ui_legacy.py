exec(open("/tmp/b6/uiharn.py").read())
import time
for f in ("gallery.ods", "gallery.xls", "rt_fixed.ods", "rt_fixed.xls", "gallery_lo.xlsx"):
    MSG.clear(); t0 = time.time(); win.load("/tmp/b6/leg/" + f); pump(0.2); bk = win.book
    info("open " + f, dict(seconds=round(time.time() - t0, 1), msg=MSG[:1], sheets=bk.sheet_count, title=win.get_title(), A20=bk.display(0, 19, 0), objects=[len(bk.objects(s)) for s in range(bk.sheet_count)], path=win.path, modified=bk.modified))
    if f == "gallery_lo.xlsx": bk.save("/tmp/b6/leg/gallery_lo_out.xlsx")
    if f == "rt_fixed.xls":
        info("  xls: formulas / names / table / cf", (bk.input(0, 6, 5), [n[0] for n in bk.names()], [len(bk.tables(s)) for s in range(bk.sheet_count)], len(bk.cf_rules(0)) if hasattr(bk, "cf_rules") else None))
        DLG.clear(); MSG.clear(); bk.set_input(0, 0, 8, "changed"); 
        try: r = win.act_save()
        except Exception as e: r = "EXC " + str(e)[:80]
        pump(0.2); info("  Save on an imported .xls goes to (dialogs, messages)", (DLG, MSG[:1], r))
done()
