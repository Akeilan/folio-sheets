"""The system region (S6-32): how numbers, dates and currency are typed and shown.

Read from the locale settings (LC_NUMERIC, LC_MONETARY, LC_TIME) and handed to the engine
once at start. Only typing, the display of General and short-date formats, and CSV follow it;
formulas and the files themselves stay region-neutral, so a workbook reads the same anywhere.
FOLIO_SHEET_REGION=default (or C) keeps the built-in US behaviour.
"""
import locale
import os

_CUR = {"decimal": ".", "group": ",", "date_order": "MDY", "short_date": "m/d/yyyy",
        "currency": "$", "currency_before": True, "currency_space": False}


def _short_date(fmt):
    """An Excel short-date code for a C date format: day and month without leading zeros
    and a four-digit year, in the region's order and with its separator."""
    out, order, i = [], "", 0
    while i < len(fmt):
        ch = fmt[i]
        if ch == "%" and i + 1 < len(fmt):
            k = fmt[i + 1]
            if k in "de":
                out.append("d"); order += "D"
            elif k in "mb":
                out.append("m"); order += "M"
            elif k in "yYC":
                out.append("yyyy"); order += "Y"
            elif k == "F":
                out.append("yyyy-mm-dd"); order += "YMD"
            elif k == "D":
                out.append("m/d/yyyy"); order += "MDY"
            i += 2
            continue
        if ch in "/-. ":
            out.append(ch if ch != " " else " ")
        i += 1
    code = "".join(out).strip()
    if sorted(order) != sorted("DMY") or not code:
        return None, None
    return code, order


def detect(env=None):
    """The region's settings, as the dict the engine's set_locale takes."""
    env = os.environ if env is None else env
    region = env.get("FOLIO_SHEET_REGION", "")
    if region.lower() in ("default", "c", "posix"):
        return dict(_CUR)
    out = dict(_CUR)
    try:
        if region:
            for cat in (locale.LC_NUMERIC, locale.LC_MONETARY, locale.LC_TIME):
                locale.setlocale(cat, region)
        else:
            locale.setlocale(locale.LC_ALL, "")
        conv = locale.localeconv()
    except (locale.Error, ValueError):
        return out
    dec = conv.get("decimal_point") or "."
    grp = conv.get("thousands_sep") or conv.get("mon_thousands_sep") or ("." if dec == "," else ",")
    if grp in (" ", "\xa0"):
        grp = "\xa0"
    out["decimal"], out["group"] = dec, grp
    sym = conv.get("currency_symbol") or ""
    if sym:
        out["currency"] = sym
        out["currency_before"] = bool(conv.get("p_cs_precedes", 1))
        out["currency_space"] = conv.get("p_sep_by_space", 0) == 1
    try:
        code, order = _short_date(locale.nl_langinfo(locale.D_FMT))
    except (AttributeError, ValueError):
        code, order = None, None
    if code:
        out["short_date"], out["date_order"] = code, order
    # Python formats floats the same whatever the region; keep LC_NUMERIC neutral for libraries
    try:
        locale.setlocale(locale.LC_NUMERIC, "C")
    except locale.Error:
        pass
    return out


def apply(settings=None):
    global _CUR
    s = detect() if settings is None else settings
    from .core import _c
    _c.set_locale(s)
    _CUR = dict(s)
    refresh_presets()


def current():
    return dict(_CUR)


def list_separator():
    """The region's list separator: a semicolon where the decimal mark is a comma."""
    return ";" if _CUR.get("decimal") == "," else ","


def _q(sym):
    return '"%s"' % sym.replace('"', '')


def currency_format():
    """The ribbon's Currency format in the region's currency and position."""
    sym, before, sp = _CUR["currency"], _CUR["currency_before"], _CUR["currency_space"]
    if sym == "$" and before and not sp:
        return '"$"#,##0.00'
    gap = "\\ " if sp else ""
    if before and not sp:
        return '%s#,##0.00;\\-%s#,##0.00' % (_q(sym), _q(sym))
    if before:
        return '%s%s#,##0.00;%s%s-#,##0.00' % (_q(sym), gap, _q(sym), gap)
    return '#,##0.00%s%s;-#,##0.00%s%s' % (gap, _q(sym), gap, _q(sym))


def accounting_format():
    """The Accounting Number Format button's code in the region's currency."""
    sym, before = _CUR["currency"], _CUR["currency_before"]
    if sym == "$" and before:
        return '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)'
    s = _q(sym)
    if before and not _CUR["currency_space"]:
        return '_-%s* #,##0.00_-;\\-%s* #,##0.00_-;_-%s* "-"??_-;_-@_-' % (s, s, s)
    if before:
        return '_ %s * #,##0.00_ ;_ %s * -#,##0.00_ ;_ %s * "-"??_ ;_ @_ ' % (s, s, s)
    return '_-* #,##0.00\\ %s_-;\\-* #,##0.00\\ %s_-;_-* "-"??\\ %s_-;_-@_-' % (s, s, s)


def refresh_presets():
    """The Number Format list's Currency and Accounting entries follow the region."""
    from . import ribbon_defs
    for i, (name, _code) in enumerate(ribbon_defs.NUMBER_FORMATS):
        if name == "Currency":
            ribbon_defs.NUMBER_FORMATS[i] = (name, currency_format())
        elif name == "Accounting":
            ribbon_defs.NUMBER_FORMATS[i] = (name, accounting_format())
