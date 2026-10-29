"""Analysis ToolPak (S3-09): the statistical and engineering tools of Data > Data Analysis.

Each tool takes the numbers it is given and returns its output table as rows of cells (str for
labels, float/int for numbers, None for empty, "#N/A" for errors), laid out as the desktop
spreadsheet's ToolPak lays them out. The distributions are worked out here (no other library)."""
import cmath
import math
import random

NA = "#N/A"


# ------------------------------------------------------------------ distributions
def _betacf(a, b, x):
    """Continued fraction for the incomplete beta function (modified Lentz)."""
    tiny = 1e-300
    qab, qap, qam = a + b, a + 1.0, a - 1.0
    c, d = 1.0, 1.0 - qab * x / qap
    if abs(d) < tiny:
        d = tiny
    d = 1.0 / d
    h = d
    for m in range(1, 400):
        m2 = 2 * m
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        d = tiny if abs(d) < tiny else d
        c = 1.0 + aa / c
        c = tiny if abs(c) < tiny else c
        d = 1.0 / d
        h *= d * c
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        d = tiny if abs(d) < tiny else d
        c = 1.0 + aa / c
        c = tiny if abs(c) < tiny else c
        d = 1.0 / d
        de = d * c
        h *= de
        if abs(de - 1.0) < 3e-16:
            break
    return h


def betainc(a, b, x):
    """The regularized incomplete beta I_x(a, b)."""
    if x <= 0:
        return 0.0
    if x >= 1:
        return 1.0
    lbt = math.lgamma(a + b) - math.lgamma(a) - math.lgamma(b) + a * math.log(x) + b * math.log(1 - x)
    bt = math.exp(lbt)
    if x < (a + 1) / (a + b + 2):
        return bt * _betacf(a, b, x) / a
    return 1.0 - bt * _betacf(b, a, 1 - x) / b


def t_cdf(t, df):
    x = df / (df + t * t)
    p = 0.5 * betainc(df / 2.0, 0.5, x)
    return 1 - p if t > 0 else p


def t_tail2(t, df):
    """P(|T| >= |t|)."""
    return betainc(df / 2.0, 0.5, df / (df + t * t))


def f_cdf(f, d1, d2):
    if f <= 0:
        return 0.0
    return betainc(d1 / 2.0, d2 / 2.0, d1 * f / (d1 * f + d2))


def norm_cdf(z):
    return 0.5 * math.erfc(-z / math.sqrt(2))


def _inverse(cdf, p, lo, hi):
    """x with cdf(x) = p, by widening then bisecting."""
    while cdf(lo) > p:
        lo = lo * 2 if lo < 0 else lo - 1
    while cdf(hi) < p:
        hi = hi * 2 if hi > 0 else hi + 1
    for _ in range(200):
        mid = (lo + hi) / 2
        if cdf(mid) < p:
            lo = mid
        else:
            hi = mid
        if hi - lo < 1e-14 * max(1.0, abs(mid)):
            break
    return (lo + hi) / 2


def t_inv(p, df):
    return _inverse(lambda x: t_cdf(x, df), p, -10.0, 10.0)


def t_inv2(alpha, df):
    """The two-tailed critical value (T.INV.2T)."""
    return t_inv(1 - alpha / 2, df)


def f_inv_rt(alpha, d1, d2):
    return _inverse(lambda x: f_cdf(x, d1, d2), 1 - alpha, 0.0, 10.0)


def norm_inv(p):
    return _inverse(norm_cdf, p, -10.0, 10.0)


# ------------------------------------------------------------------ basic statistics
def mean(xs):
    return sum(xs) / len(xs)


def var_s(xs):
    m = mean(xs)
    return sum((x - m) ** 2 for x in xs) / (len(xs) - 1)


def median(xs):
    s = sorted(xs)
    n = len(s)
    return s[n // 2] if n % 2 else (s[n // 2 - 1] + s[n // 2]) / 2


def mode(xs):
    counts = {}
    first = {}
    for i, x in enumerate(xs):
        counts[x] = counts.get(x, 0) + 1
        first.setdefault(x, i)
    best = max(counts.values())
    if best < 2:
        return NA
    return min((x for x in counts if counts[x] == best), key=lambda x: first[x])


def skew(xs):
    n = len(xs)
    if n < 3:
        return "#DIV/0!"
    m, sd = mean(xs), math.sqrt(var_s(xs))
    if sd == 0:
        return "#DIV/0!"
    return n / ((n - 1) * (n - 2)) * sum(((x - m) / sd) ** 3 for x in xs)


def kurt(xs):
    n = len(xs)
    if n < 4:
        return "#DIV/0!"
    m, sd = mean(xs), math.sqrt(var_s(xs))
    if sd == 0:
        return "#DIV/0!"
    s4 = sum(((x - m) / sd) ** 4 for x in xs)
    return n * (n + 1) / ((n - 1) * (n - 2) * (n - 3)) * s4 - 3 * (n - 1) ** 2 / ((n - 2) * (n - 3))


def _div(a, b):
    return a / b if b else "#DIV/0!"


# ------------------------------------------------------------------ tools
def descriptive(cols, labels, conf=None, kth_large=None, kth_small=None, summary=True):
    """Descriptive Statistics: one two-column block per input column."""
    blocks = []
    for name, xs in zip(labels, cols):
        n = len(xs)
        rows = [[name, None]]
        if summary:
            sd = math.sqrt(var_s(xs)) if n > 1 else "#DIV/0!"
            rows += [["Mean", mean(xs)],
                     ["Standard Error", sd / math.sqrt(n) if n > 1 else "#DIV/0!"],
                     ["Median", median(xs)],
                     ["Mode", mode(xs)],
                     ["Standard Deviation", sd],
                     ["Sample Variance", var_s(xs) if n > 1 else "#DIV/0!"],
                     ["Kurtosis", kurt(xs)],
                     ["Skewness", skew(xs)],
                     ["Range", max(xs) - min(xs)],
                     ["Minimum", min(xs)],
                     ["Maximum", max(xs)],
                     ["Sum", sum(xs)],
                     ["Count", n]]
        if kth_large:
            s = sorted(xs, reverse=True)
            rows.append(["Largest(%d)" % kth_large, s[kth_large - 1] if kth_large <= n else "#NUM!"])
        if kth_small:
            s = sorted(xs)
            rows.append(["Smallest(%d)" % kth_small, s[kth_small - 1] if kth_small <= n else "#NUM!"])
        if conf:
            lvl = conf * 100
            v = t_inv2(1 - conf, n - 1) * math.sqrt(var_s(xs)) / math.sqrt(n) if n > 1 else "#DIV/0!"
            rows.append(["Confidence Level(%s%%)" % ("%.1f" % lvl), v])
        blocks.append(rows)
    height = max(len(b) for b in blocks)
    out = []
    for r in range(height):
        row = []
        for b in blocks:
            row += b[r] if r < len(b) else [None, None]
        out.append(row)
    return out


def histogram(xs, bins=None, pareto=False, cumulative=False, bin_label="Bin"):
    """Histogram: frequency per bin (x <= bin, above the previous), then More."""
    if not bins:
        lo, hi = min(xs), max(xs)
        k = max(1, int(math.sqrt(len(xs))))
        step = (hi - lo) / k if hi > lo else 1
        bins = [lo + step * i for i in range(k + 1)]
    bins = sorted(bins)
    counts = [0] * (len(bins) + 1)
    for x in xs:
        for i, b in enumerate(bins):
            if x <= b:
                counts[i] += 1
                break
        else:
            counts[-1] += 1
    names = list(bins) + ["More"]
    rows = list(zip(names, counts))
    n = len(xs)

    def table(rs, head):
        out = [[head, "Frequency"] + (["Cumulative %"] if cumulative else [])]
        run = 0
        for name, c in rs:
            run += c
            out.append([name, c] + ([run / n if n else 0] if cumulative else []))
        return out
    out = table(rows, bin_label)
    if pareto:
        srt = sorted(rows, key=lambda x: -x[1])
        p = table(srt, bin_label)
        out = [a + [None] + b for a, b in zip(out, p)]
    return out, bins, counts


def _solve(a, b):
    """Gauss-Jordan: a x = b (a square); None when singular."""
    n = len(a)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(m[r][c]))
        if abs(m[p][c]) < 1e-12:
            return None
        m[c], m[p] = m[p], m[c]
        for r in range(n):
            if r != c:
                f = m[r][c] / m[c][c]
                for k in range(c, n + 1):
                    m[r][k] -= f * m[c][k]
    return [m[i][n] / m[i][i] for i in range(n)]


def _invert(a):
    n = len(a)
    cols = []
    for j in range(n):
        e = [1.0 if i == j else 0.0 for i in range(n)]
        x = _solve(a, e)
        if x is None:
            return None
        cols.append(x)
    return [[cols[j][i] for j in range(n)] for i in range(n)]


def regression(y, xcols, xlabels, const_zero=False, conf=0.95, residuals=False, std_residuals=False, ylabel="Y"):
    n = len(y)
    k = len(xcols)
    X = [([] if const_zero else [1.0]) + [xc[i] for xc in xcols] for i in range(n)]
    p = len(X[0])
    xtx = [[sum(X[r][i] * X[r][j] for r in range(n)) for j in range(p)] for i in range(p)]
    xty = [sum(X[r][i] * y[r] for r in range(n)) for i in range(p)]
    inv = _invert(xtx)
    if inv is None:
        raise ValueError("The X values are linearly dependent: regression can't be worked out.")
    beta = [sum(inv[i][j] * xty[j] for j in range(p)) for i in range(p)]
    fit = [sum(beta[i] * X[r][i] for i in range(p)) for r in range(n)]
    res = [y[r] - fit[r] for r in range(n)]
    ym = mean(y)
    sse = sum(e * e for e in res)
    sst = sum((v - ym) ** 2 for v in y) if not const_zero else sum(v * v for v in y)
    ssr = sst - sse
    df_r = k
    df_e = n - k - (0 if const_zero else 1)
    df_t = n - (0 if const_zero else 1)
    mse = sse / df_e if df_e > 0 else "#NUM!"
    r2 = ssr / sst if sst else 1.0
    adj = 1 - (1 - r2) * df_t / df_e if df_e > 0 else "#NUM!"
    se = math.sqrt(mse) if isinstance(mse, float) else "#NUM!"
    F = (ssr / df_r) / mse if isinstance(mse, float) and mse and df_r else "#NUM!"
    sigF = 1 - f_cdf(F, df_r, df_e) if isinstance(F, float) else "#NUM!"
    out = [["SUMMARY OUTPUT"], [], ["Regression Statistics", None],
           ["Multiple R", math.sqrt(max(r2, 0))], ["R Square", r2], ["Adjusted R Square", adj],
           ["Standard Error", se], ["Observations", n], [],
           ["ANOVA"], [None, "df", "SS", "MS", "F", "Significance F"],
           ["Regression", df_r, ssr, ssr / df_r if df_r else "#NUM!", F, sigF],
           ["Residual", df_e, sse, mse],
           ["Total", df_t, sst], []]
    head = [None, "Coefficients", "Standard Error", "t Stat", "P-value", "Lower 95%", "Upper 95%"]
    if abs(conf - 0.95) > 1e-9:
        head += ["Lower %.1f%%" % (conf * 100), "Upper %.1f%%" % (conf * 100)]
    out.append(head)
    names = ([] if const_zero else ["Intercept"]) + list(xlabels)
    tc95 = t_inv2(0.05, df_e) if df_e > 0 else None
    tcc = t_inv2(1 - conf, df_e) if df_e > 0 else None
    for i in range(p):
        sei = math.sqrt(inv[i][i] * mse) if isinstance(mse, float) else "#NUM!"
        if isinstance(sei, float) and sei > 0:
            t = beta[i] / sei
            pv = t_tail2(t, df_e)
            row = [names[i], beta[i], sei, t, pv, beta[i] - tc95 * sei, beta[i] + tc95 * sei]
            if len(head) > 7:
                row += [beta[i] - tcc * sei, beta[i] + tcc * sei]
        else:
            row = [names[i], beta[i], sei, "#NUM!", "#NUM!", "#NUM!", "#NUM!"]
        out.append(row)
    if residuals or std_residuals:
        out += [[], [], ["RESIDUAL OUTPUT"], []]
        h = ["Observation", "Predicted " + ylabel] + (["Residuals"] if residuals else []) + (["Standard Residuals"] if std_residuals else [])
        out.append(h)
        sdr = math.sqrt(sse / (n - 1)) if n > 1 else 0
        for i in range(n):
            row = [i + 1, fit[i]]
            if residuals:
                row.append(res[i])
            if std_residuals:
                row.append(res[i] / sdr if sdr else "#DIV/0!")
            out.append(row)
    return out


def _tail_rows(stat, df, alpha, name="t"):
    one = 1 - t_cdf(abs(stat), df)
    return [["%s Stat" % name, stat], ["P(T<=t) one-tail", one], ["t Critical one-tail", t_inv(1 - alpha, df)],
            ["P(T<=t) two-tail", 2 * one], ["t Critical two-tail", t_inv2(alpha, df)]]


def t_paired(a, b, la, lb, hyp=0.0, alpha=0.05):
    if len(a) != len(b):
        raise ValueError("Input ranges must have the same number of values.")
    n = len(a)
    d = [x - y for x, y in zip(a, b)]
    sd = math.sqrt(var_s(d))
    t = (mean(d) - hyp) / (sd / math.sqrt(n))
    ma, mb = mean(a), mean(b)
    r = sum((x - ma) * (y - mb) for x, y in zip(a, b)) / math.sqrt(sum((x - ma) ** 2 for x in a) * sum((y - mb) ** 2 for y in b))
    return [["t-Test: Paired Two Sample for Means"], [], [None, la, lb], ["Mean", ma, mb], ["Variance", var_s(a), var_s(b)],
            ["Observations", n, n], ["Pearson Correlation", r], ["Hypothesized Mean Difference", hyp], ["df", n - 1]] + _tail_rows(t, n - 1, alpha)


def t_equal(a, b, la, lb, hyp=0.0, alpha=0.05):
    na, nb = len(a), len(b)
    va, vb = var_s(a), var_s(b)
    pooled = ((na - 1) * va + (nb - 1) * vb) / (na + nb - 2)
    t = (mean(a) - mean(b) - hyp) / math.sqrt(pooled * (1 / na + 1 / nb))
    df = na + nb - 2
    return [["t-Test: Two-Sample Assuming Equal Variances"], [], [None, la, lb], ["Mean", mean(a), mean(b)], ["Variance", va, vb],
            ["Observations", na, nb], ["Pooled Variance", pooled], ["Hypothesized Mean Difference", hyp], ["df", df]] + _tail_rows(t, df, alpha)


def t_unequal(a, b, la, lb, hyp=0.0, alpha=0.05):
    na, nb = len(a), len(b)
    va, vb = var_s(a), var_s(b)
    sa, sb = va / na, vb / nb
    t = (mean(a) - mean(b) - hyp) / math.sqrt(sa + sb)
    df = round((sa + sb) ** 2 / (sa * sa / (na - 1) + sb * sb / (nb - 1)))
    return [["t-Test: Two-Sample Assuming Unequal Variances"], [], [None, la, lb], ["Mean", mean(a), mean(b)], ["Variance", va, vb],
            ["Observations", na, nb], ["Hypothesized Mean Difference", hyp], ["df", df]] + _tail_rows(t, df, alpha)


def z_test(a, b, la, lb, var_a, var_b, hyp=0.0, alpha=0.05):
    na, nb = len(a), len(b)
    z = (mean(a) - mean(b) - hyp) / math.sqrt(var_a / na + var_b / nb)
    one = 1 - norm_cdf(abs(z))
    return [["z-Test: Two Sample for Means"], [], [None, la, lb], ["Mean", mean(a), mean(b)], ["Known Variance", var_a, var_b],
            ["Observations", na, nb], ["Hypothesized Mean Difference", hyp], ["z", z], ["P(Z<=z) one-tail", one],
            ["z Critical one-tail", norm_inv(1 - alpha)], ["P(Z<=z) two-tail", 2 * one], ["z Critical two-tail", norm_inv(1 - alpha / 2)]]


def f_test(a, b, la, lb, alpha=0.05):
    va, vb = var_s(a), var_s(b)
    d1, d2 = len(a) - 1, len(b) - 1
    F = va / vb
    if F < 1:
        p = f_cdf(F, d1, d2)
        crit = _inverse(lambda x: f_cdf(x, d1, d2), alpha, 0.0, 10.0)
    else:
        p = 1 - f_cdf(F, d1, d2)
        crit = f_inv_rt(alpha, d1, d2)
    return [["F-Test Two-Sample for Variances"], [], [None, la, lb], ["Mean", mean(a), mean(b)], ["Variance", va, vb],
            ["Observations", len(a), len(b)], ["df", d1, d2], ["F", F], ["P(F<=f) one-tail", p], ["F Critical one-tail", crit]]


def _anova_row(name, ss, df, ms=None, F=None, alpha=None, df2=None):
    row = [name, ss, df]
    if ms is not None:
        row.append(ms)
    if F is not None:
        row += [F, 1 - f_cdf(F, df, df2), f_inv_rt(alpha, df, df2)]
    return row


def anova_single(groups, labels, alpha=0.05):
    allx = [x for g in groups for x in g]
    gm = mean(allx)
    ssb = sum(len(g) * (mean(g) - gm) ** 2 for g in groups)
    ssw = sum(sum((x - mean(g)) ** 2 for x in g) for g in groups)
    dfb, dfw = len(groups) - 1, len(allx) - len(groups)
    msb, msw = ssb / dfb, ssw / dfw
    out = [["Anova: Single Factor"], [], ["SUMMARY"], ["Groups", "Count", "Sum", "Average", "Variance"]]
    for name, g in zip(labels, groups):
        out.append([name, len(g), sum(g), mean(g), var_s(g) if len(g) > 1 else "#DIV/0!"])
    out += [[], [], ["ANOVA"], ["Source of Variation", "SS", "df", "MS", "F", "P-value", "F crit"],
            _anova_row("Between Groups", ssb, dfb, msb, msb / msw, alpha, dfw),
            _anova_row("Within Groups", ssw, dfw, msw), [], ["Total", ssb + ssw, len(allx) - 1]]
    return out


def anova_two_no_rep(grid, row_labels, col_labels, alpha=0.05):
    r, c = len(grid), len(grid[0])
    allx = [x for row in grid for x in row]
    gm = mean(allx)
    ssr = c * sum((mean(row) - gm) ** 2 for row in grid)
    cols = [[grid[i][j] for i in range(r)] for j in range(c)]
    ssc = r * sum((mean(col) - gm) ** 2 for col in cols)
    sst = sum((x - gm) ** 2 for x in allx)
    sse = sst - ssr - ssc
    dfr, dfc, dfe = r - 1, c - 1, (r - 1) * (c - 1)
    mse = sse / dfe
    out = [["Anova: Two-Factor Without Replication"], [], ["SUMMARY", "Count", "Sum", "Average", "Variance"]]
    for name, row in zip(row_labels, grid):
        out.append([name, c, sum(row), mean(row), var_s(row)])
    out.append([])
    for name, col in zip(col_labels, cols):
        out.append([name, r, sum(col), mean(col), var_s(col)])
    out += [[], [], ["ANOVA"], ["Source of Variation", "SS", "df", "MS", "F", "P-value", "F crit"],
            _anova_row("Rows", ssr, dfr, ssr / dfr, (ssr / dfr) / mse, alpha, dfe),
            _anova_row("Columns", ssc, dfc, ssc / dfc, (ssc / dfc) / mse, alpha, dfe),
            _anova_row("Error", sse, dfe, mse), [], ["Total", sst, r * c - 1]]
    return out


def anova_two_rep(grid, rows_per, row_labels, col_labels, alpha=0.05):
    """grid: all data rows (rows_per rows for each sample), columns = factor B."""
    nb = len(grid) // rows_per
    c = len(grid[0])
    cells = [[[grid[k * rows_per + i][j] for i in range(rows_per)] for j in range(c)] for k in range(nb)]
    allx = [x for row in grid for x in row]
    gm = mean(allx)
    n = rows_per
    ss_s = c * n * sum((mean([x for cell in cells[k] for x in cell]) - gm) ** 2 for k in range(nb))
    ss_c = nb * n * sum((mean([x for k in range(nb) for x in cells[k][j]]) - gm) ** 2 for j in range(c))
    ss_w = sum(sum((x - mean(cells[k][j])) ** 2 for x in cells[k][j]) for k in range(nb) for j in range(c))
    sst = sum((x - gm) ** 2 for x in allx)
    ss_i = sst - ss_s - ss_c - ss_w
    df_s, df_c, df_i, df_w = nb - 1, c - 1, (nb - 1) * (c - 1), nb * c * (n - 1)
    msw = ss_w / df_w
    out = [["Anova: Two-Factor With Replication"], [], ["SUMMARY"] + list(col_labels) + ["Total"]]
    for k in range(nb):
        out.append([row_labels[k] if k < len(row_labels) else "Sample %d" % (k + 1)])
        allk = [x for cell in cells[k] for x in cell]
        out.append(["Count"] + [n] * c + [len(allk)])
        out.append(["Sum"] + [sum(cl) for cl in cells[k]] + [sum(allk)])
        out.append(["Average"] + [mean(cl) for cl in cells[k]] + [mean(allk)])
        out.append(["Variance"] + [var_s(cl) for cl in cells[k]] + [var_s(allk)])
        out.append([])
    out.append(["Total"])
    colall = [[x for k in range(nb) for x in cells[k][j]] for j in range(c)]
    out.append(["Count"] + [len(x) for x in colall])
    out.append(["Sum"] + [sum(x) for x in colall])
    out.append(["Average"] + [mean(x) for x in colall])
    out.append(["Variance"] + [var_s(x) for x in colall])
    out += [[], [], ["ANOVA"], ["Source of Variation", "SS", "df", "MS", "F", "P-value", "F crit"],
            _anova_row("Sample", ss_s, df_s, ss_s / df_s, (ss_s / df_s) / msw, alpha, df_w),
            _anova_row("Columns", ss_c, df_c, ss_c / df_c, (ss_c / df_c) / msw, alpha, df_w),
            _anova_row("Interaction", ss_i, df_i, ss_i / df_i, (ss_i / df_i) / msw, alpha, df_w),
            _anova_row("Within", ss_w, df_w, msw), [], ["Total", sst, len(allx) - 1]]
    return out


def correlation(cols, labels, covariance=False):
    k = len(cols)
    out = [[None] + list(labels)]
    for i in range(k):
        row = [labels[i]]
        for j in range(k):
            if j > i:
                row.append(None)
                continue
            a, b = cols[i], cols[j]
            if i == j and not covariance:
                row.append(1)
                continue
            ma, mb = mean(a), mean(b)
            cov = sum((x - ma) * (y - mb) for x, y in zip(a, b)) / len(a)
            if covariance:
                row.append(cov)
            else:
                sa = math.sqrt(sum((x - ma) ** 2 for x in a) / len(a))
                sb = math.sqrt(sum((y - mb) ** 2 for y in b) / len(b))
                row.append(cov / (sa * sb) if sa and sb else "#DIV/0!")
        out.append(row)
    return out


def moving_average(xs, interval=3, std_errors=False):
    out = []
    for i in range(len(xs)):
        if i < interval - 1:
            row = [NA]
        else:
            row = [mean(xs[i - interval + 1:i + 1])]
        if std_errors:
            if i < 2 * interval - 2:
                row.append(NA)
            else:
                # the root mean square error over the last `interval` forecasts
                errs = [(xs[j] - mean(xs[j - interval + 1:j + 1])) ** 2 for j in range(i - interval + 1, i + 1)]
                row.append(math.sqrt(sum(errs) / interval))
        out.append(row)
    return out


def exp_smoothing(xs, damping=0.3, std_errors=False):
    a = 1 - damping
    out = [[NA] + ([NA] if std_errors else [])]
    f = xs[0]
    fc = [None, f]
    for i in range(1, len(xs)):
        row = [f]
        if std_errors:
            if i < 4:
                row.append(NA)
            else:
                errs = [(xs[j] - fc[j]) ** 2 for j in range(i - 2, i + 1)]
                row.append(math.sqrt(sum(errs) / 3))
        out.append(row)
        f = a * xs[i] + (1 - a) * f
        fc.append(f)
    return out


def rank_percentile(cols, labels):
    blocks = []
    for name, xs in zip(labels, cols):
        order = sorted(range(len(xs)), key=lambda i: -xs[i])
        rows = [["Point", name, "Rank", "Percent"]]
        n = len(xs)
        for pos, i in enumerate(order):
            rank = 1 + sum(1 for v in xs if v > xs[i])
            below = sum(1 for v in xs if v < xs[i])
            pct = below / (n - 1) if n > 1 else 1.0
            rows.append([i + 1, xs[i], rank, math.floor(pct * 1000) / 1000])
            _ = pos
        blocks.append(rows)
    out = []
    for r in range(max(len(b) for b in blocks)):
        row = []
        for b in blocks:
            row += b[r] if r < len(b) else [None] * 4
        out.append(row)
    return out


def sampling(xs, periodic=None, count=None, seed=None):
    if periodic:
        return [[xs[i]] for i in range(periodic - 1, len(xs), periodic)]
    rng = random.Random(seed)
    return [[rng.choice(xs)] for _ in range(count or 1)]


def random_numbers(nvars, count, dist, params, seed=None):
    rng = random.Random(seed)
    out = [[None] * nvars for _ in range(count)]
    if dist == "Patterned":
        lo, hi, step, rep_num, rep_seq = params
        seq = []
        v = lo
        while v <= hi + 1e-12:
            seq += [v] * rep_num
            v += step
        seq = seq * rep_seq
        for j in range(nvars):
            for i in range(min(count, len(seq))):
                out[i][j] = seq[i]
        return [r for r in out if any(v is not None for v in r)]
    for j in range(nvars):
        for i in range(count):
            if dist == "Uniform":
                v = params[0] + (params[1] - params[0]) * rng.random()
            elif dist == "Normal":
                v = rng.gauss(params[0], params[1])
            elif dist == "Bernoulli":
                v = 1 if rng.random() < params[0] else 0
            elif dist == "Binomial":
                v = sum(1 for _ in range(int(params[1])) if rng.random() < params[0])
            elif dist == "Poisson":
                lam, k, p = params[0], 0, 1.0
                limit = math.exp(-lam)
                while True:
                    p *= rng.random()
                    if p <= limit:
                        break
                    k += 1
                v = k
            elif dist == "Discrete":
                vals, probs = params
                t, acc, v = rng.random() * sum(probs), 0.0, vals[-1]
                for x, pr in zip(vals, probs):
                    acc += pr
                    if t <= acc:
                        v = x
                        break
            else:
                raise ValueError("Unknown distribution %s" % dist)
            out[i][j] = v
    return out


def _fft(a):
    n = len(a)
    if n == 1:
        return a[:]
    even, odd = _fft(a[0::2]), _fft(a[1::2])
    t = [cmath.exp(-2j * math.pi * k / n) * odd[k] for k in range(n // 2)]
    return [even[k] + t[k] for k in range(n // 2)] + [even[k] - t[k] for k in range(n // 2)]


def complex_text(z):
    def num(x):
        if abs(x - round(x)) < 1e-12:
            x = round(x)
        return "%.15g" % x
    re_, im = z.real, z.imag
    if abs(im) < 1e-12:
        return num(re_)
    if abs(re_) < 1e-12:
        return ("" if im != 1 else "") + (num(im) if abs(im) != 1 else ("-" if im < 0 else "")) + "i"
    sign = "+" if im > 0 else "-"
    mag = "" if abs(abs(im) - 1) < 1e-12 else num(abs(im))
    return "%s%s%si" % (num(re_), sign, mag)


def parse_complex(t):
    if isinstance(t, (int, float)):
        return complex(t, 0)
    s = str(t).strip().replace(" ", "")
    if not s:
        return 0j
    s = s.replace("j", "i")
    if s.endswith("i"):
        body = s[:-1]
        cut = max(body.rfind("+"), body.rfind("-"))
        if cut <= 0:
            im = body in ("", "+") and 1 or (body == "-" and -1 or float(body))
            return complex(0, im)
        re_, im = body[:cut], body[cut:]
        im = 1 if im == "+" else (-1 if im == "-" else float(im))
        return complex(float(re_), im)
    return complex(float(s), 0)


def fourier(values, inverse=False):
    n = len(values)
    if n == 0 or n & (n - 1) or n > 4096:
        raise ValueError("Input range must have 2^n values (up to 4096).")
    zs = [parse_complex(v) for v in values]
    if inverse:
        res = [z.conjugate() / n for z in _fft([z.conjugate() for z in zs])]
    else:
        res = _fft(zs)
    return [[complex_text(z)] for z in res]


TOOLS = [
    "Anova: Single Factor", "Anova: Two-Factor With Replication", "Anova: Two-Factor Without Replication",
    "Correlation", "Covariance", "Descriptive Statistics", "Exponential Smoothing",
    "F-Test Two-Sample for Variances", "Fourier Analysis", "Histogram", "Moving Average",
    "Random Number Generation", "Rank and Percentile", "Regression", "Sampling",
    "t-Test: Paired Two Sample for Means", "t-Test: Two-Sample Assuming Equal Variances",
    "t-Test: Two-Sample Assuming Unequal Variances", "z-Test: Two Sample for Means",
]
