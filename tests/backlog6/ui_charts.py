exec(open("/tmp/b6/uiharn.py").read())
import subprocess
focus(); S = g.sheet
rows = [("Month", "North", "South", "East"), ("Jan", "12", "7", "3"), ("Feb", "15", "9", "8"), ("Mar", "9", "14", "5"), ("Apr", "18", "11", "-4"), ("May", "21", "6", "10")]
for r, row in enumerate(rows):
    for c, v in enumerate(row): b.set_input(S, r, c, v)
kinds = ["chart-column:col", "chart-column:col-stacked", "chart-column:col-percent", "chart-column:bar", "chart-column:bar-stacked", "chart-line:line", "chart-line:line-markers", "chart-line:area", "chart-line:area-stacked", "chart-pie:pie", "chart-pie:doughnut", "chart-scatter:scatter", "chart-scatter:scatter-lines", "chart-scatter:scatter-smooth", "chart-column:radar", "chart-column:radar-filled", "chart-combo:combo", "chart-combo:combo-secondary", "chart-column:stock", "chart-column:contour", "chartex:waterfall", "chartex:funnel", "chartex:histogram", "chartex:pareto", "chartex:boxwhisker", "chartex:treemap", "chartex:sunburst"]
made = []
for i, k in enumerate(kinds):
    g.select_range((0, 0, 5, 3), (0, 0)); n0 = len(b.objects(S)); 
    try: win.run(k)
    except Exception as e: print("FAIL %s raised %s" % (k, str(e)[:120]), flush=True); N[1] += 1; continue
    pump(0.1); n1 = len(b.objects(S))
    if n1 != n0 + 1: print("FAIL %s made no chart (messages %r)" % (k, MSG[-1:]), flush=True); N[1] += 1; MSG.clear(); continue
    p = g.params(); col, row = i % 5, i // 5
    b.place_object(S, n1 - 1, 330 + col * 250, 10 + row * 190, 240, 180, p, "Move Chart"); made.append(k); g.deselect_objects()
g.select_cell(0, 0); g.refresh(); pump(0.8)
win.maximize(); pump(0.8); subprocess.run(["import", "-window", "root", "/tmp/b6/charts1.png"])
info("charts made", len(made))
# data change updates the chart; the series follow inserted rows
sp0 = b.chart_spec(S, 0); b.insert_rows(S, 0, 1); sp1 = b.chart_spec(S, 0); info("chart series ref before / after inserting a row above", (str(sp0.get("series"))[:120], str(sp1.get("series"))[:120])); b.undo()
win.save_to("/tmp/b6/charts.xlsx")
done()
