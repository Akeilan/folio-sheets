"""Theme effect schemes (S3-11): the fill, line and effect styles shapes and SmartArt drawn in the
theme's styles take (Page Layout > Effects). Written here from their descriptions: each scheme is
three fills (subtle, moderate, intense), three lines, three effects and three background fills,
as a theme's fmtScheme holds them."""

PH = '<a:schemeClr val="phClr">%s</a:schemeClr>'


def _clr(mods=""):
    return PH % mods


def solid(mods=""):
    return "<a:solidFill>%s</a:solidFill>" % _clr(mods)


def grad(stops, ang=5400000, path=None):
    gs = "".join('<a:gs pos="%d">%s</a:gs>' % (p, _clr(m)) for p, m in stops)
    if path:
        shade = '<a:path path="%s"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path>' % path
    else:
        shade = '<a:lin ang="%d" scaled="0"/>' % ang
    return '<a:gradFill rotWithShape="1"><a:gsLst>%s</a:gsLst>%s</a:gradFill>' % (gs, shade)


def tint(v):
    return '<a:tint val="%d"/>' % v


def shade(v):
    return '<a:shade val="%d"/>' % v


def sat(v):
    return '<a:satMod val="%d"/>' % v


def lum(v):
    return '<a:lumMod val="%d"/>' % v


def line(w, fill=None, dash="solid"):
    return ('<a:ln w="%d" cap="flat" cmpd="sng" algn="ctr">%s<a:prstDash val="%s"/><a:miter lim="800000"/></a:ln>'
            % (w, fill or solid(), dash))


def shadow(blur, dist, alpha, ang=5400000, outer=True):
    tag = "outerShdw" if outer else "innerShdw"
    extra = ' rotWithShape="0"' if outer else ""
    return ('<a:%s blurRad="%d" dist="%d" dir="%d"%s><a:srgbClr val="000000"><a:alpha val="%d"/></a:srgbClr></a:%s>'
            % (tag, blur, dist, ang, extra, alpha, tag))


def glow(rad, alpha):
    return '<a:glow rad="%d">%s</a:glow>' % (rad, _clr('<a:alpha val="%d"/>' % alpha + sat(175000)))


def reflection(end=35000, dist=0):
    return ('<a:reflection blurRad="6350" stA="50000" endA="300" endPos="%d" dist="%d" dir="5400000" sy="-100000" algn="bl" rotWithShape="0"/>'
            % (end, dist))


def soft_edge(rad):
    return '<a:softEdge rad="%d"/>' % rad


def bevel(kind="circle", w=63500, h=25400, material=None):
    mat = ' prstMaterial="%s"' % material if material else ""
    return ('<a:scene3d><a:camera prst="orthographicFront"><a:rot lat="0" lon="0" rev="0"/></a:camera>'
            '<a:lightRig rig="threePt" dir="t"><a:rot lat="0" lon="0" rev="1200000"/></a:lightRig></a:scene3d>'
            '<a:sp3d%s><a:bevelT w="%d" h="%d" prst="%s"/></a:sp3d>' % (mat, w, h, kind))


def effect(*parts, three_d=""):
    return "<a:effectStyle><a:effectLst>%s</a:effectLst>%s</a:effectStyle>" % ("".join(parts), three_d)


SUBTLE_BG = solid()


def scheme(name, fills, lines, effects, bgs):
    assert len(fills) == len(lines) == len(effects) == len(bgs) == 3
    return ('<a:fmtScheme name="%s"><a:fillStyleLst>%s</a:fillStyleLst><a:lnStyleLst>%s</a:lnStyleLst>'
            '<a:effectStyleLst>%s</a:effectStyleLst><a:bgFillStyleLst>%s</a:bgFillStyleLst></a:fmtScheme>'
            % (name, "".join(fills), "".join(lines), "".join(effects), "".join(bgs)))


def _schemes():
    out = []
    # Office 2007 - 2010: soft gradients, a drop shadow, a bevel on the intense effect
    out.append(scheme("Office 2007 - 2010",
                      [solid(), grad([(0, tint(50000) + sat(300000)), (35000, tint(37000) + sat(300000)), (100000, tint(15000) + sat(350000))], 16200000),
                       grad([(0, shade(51000) + sat(130000)), (80000, shade(93000) + sat(130000)), (100000, shade(94000) + sat(135000))], 16200000)],
                      [line(9525, solid(shade(95000) + sat(105000))), line(25400), line(38100)],
                      [effect(shadow(40000, 20000, 38000)), effect(shadow(40000, 23000, 35000)),
                       effect(shadow(40000, 23000, 35000), three_d=bevel("circle", 63500, 25400))],
                      [solid(), grad([(0, tint(40000) + sat(350000)), (40000, tint(45000) + shade(99000) + sat(350000)), (100000, shade(20000) + sat(255000))], path="circle"),
                       grad([(0, tint(80000) + sat(300000)), (100000, shade(30000) + sat(200000))], path="circle")]))
    # Subtle Solids: flat fills, thin lines, no effects
    out.append(scheme("Subtle Solids",
                      [solid(), solid(tint(85000)), solid(shade(90000))],
                      [line(6350), line(12700), line(19050)],
                      [effect(), effect(), effect(shadow(25400, 12700, 25000))],
                      [solid(), solid(tint(95000) + sat(170000)), solid(shade(85000))]))
    # Banded Edge: solid fills, a wide band of the darker shade round the edge
    out.append(scheme("Banded Edge",
                      [solid(), solid(tint(90000)), grad([(0, tint(85000)), (60000, ""), (100000, shade(70000))], 5400000)],
                      [line(12700, solid(shade(75000))), line(38100, solid(shade(70000))), line(57150, solid(shade(60000)))],
                      [effect(), effect(three_d=bevel("angle", 38100, 38100)), effect(shadow(50800, 25400, 40000), three_d=bevel("angle", 57150, 38100))],
                      [solid(), solid(tint(90000)), grad([(0, tint(80000)), (100000, shade(70000))], 5400000)]))
    # Smokey Glass: translucent, softly lit fills with a gentle shadow
    out.append(scheme("Smokey Glass",
                      [solid(), solid('<a:alpha val="80000"/>'), grad([(0, tint(60000) + '<a:alpha val="70000"/>'), (100000, shade(80000) + '<a:alpha val="85000"/>')], 5400000)],
                      [line(6350, solid(tint(60000))), line(12700, solid(tint(50000))), line(19050, solid(shade(80000)))],
                      [effect(), effect(shadow(38100, 19050, 30000)), effect(shadow(63500, 25400, 45000), three_d=bevel("softRound", 50800, 25400, "translucentPowder"))],
                      [solid(), solid('<a:alpha val="85000"/>'), grad([(0, tint(70000)), (100000, shade(60000))], path="circle")]))
    # Glow Edge: a glow of the shape's own colour
    out.append(scheme("Glow Edge",
                      [solid(), solid(tint(80000)), grad([(0, tint(75000)), (100000, shade(85000))], 5400000)],
                      [line(9525), line(19050), line(28575)],
                      [effect(), effect(glow(63500, 40000)), effect(glow(101600, 55000))],
                      [solid(), solid(tint(90000)), grad([(0, tint(85000)), (100000, shade(75000))], path="circle")]))
    # Top Shadow: the shadow falls from the top edge inside the shape
    out.append(scheme("Top Shadow",
                      [solid(), grad([(0, tint(88000)), (100000, "")], 5400000), grad([(0, tint(70000)), (100000, shade(90000))], 5400000)],
                      [line(9525), line(19050, solid(shade(85000))), line(25400, solid(shade(75000)))],
                      [effect(), effect(shadow(38100, 12700, 35000, outer=False)), effect(shadow(63500, 25400, 45000, outer=False), shadow(38100, 12700, 30000))],
                      [solid(), solid(tint(92000)), grad([(0, shade(80000)), (100000, tint(90000))], 5400000)]))
    # Frosted: pale, misty fills with a soft edge
    out.append(scheme("Frosted",
                      [solid(tint(65000)), grad([(0, tint(40000)), (100000, tint(80000))], 5400000), grad([(0, tint(30000)), (100000, "")], 5400000)],
                      [line(6350, solid(tint(50000))), line(12700, solid(tint(40000))), line(19050, solid(tint(30000)))],
                      [effect(), effect(soft_edge(12700)), effect(soft_edge(25400), three_d=bevel("relaxedInset", 38100, 12700, "matte"))],
                      [solid(tint(70000)), grad([(0, tint(50000)), (100000, tint(85000))], 5400000), grad([(0, tint(35000)), (100000, shade(90000))], path="circle")]))
    # Inset: pressed-in bevels
    out.append(scheme("Inset",
                      [solid(), solid(tint(92000)), grad([(0, shade(85000)), (100000, tint(85000))], 5400000)],
                      [line(9525), line(12700, solid(shade(80000))), line(19050, solid(shade(70000)))],
                      [effect(), effect(shadow(25400, 12700, 40000, outer=False)), effect(shadow(50800, 19050, 50000, outer=False), three_d=bevel("relaxedInset", 76200, 25400))],
                      [solid(), solid(tint(90000)), grad([(0, shade(75000)), (100000, tint(85000))], 5400000)]))
    # Milk Glass: bright, satin fills and a soft round bevel
    out.append(scheme("Milk Glass",
                      [solid(), grad([(0, tint(85000) + sat(110000)), (100000, shade(95000))], 5400000),
                       grad([(0, tint(70000) + sat(120000)), (50000, ""), (100000, shade(80000))], 5400000)],
                      [line(6350, solid(tint(85000))), line(12700, solid(tint(75000))), line(19050)],
                      [effect(), effect(shadow(38100, 15875, 30000)), effect(shadow(57150, 22225, 35000), three_d=bevel("softRound", 63500, 50800, "softEdge"))],
                      [solid(), solid(tint(90000)), grad([(0, tint(85000)), (100000, shade(80000))], path="circle")]))
    # Riblet: fine ribbed edges
    out.append(scheme("Riblet",
                      [solid(), solid(tint(88000)), grad([(0, ""), (50000, tint(80000)), (100000, "")], 5400000)],
                      [line(12700), line(19050), line(25400, solid(shade(80000)))],
                      [effect(), effect(three_d=bevel("riblet", 25400, 12700)), effect(shadow(38100, 19050, 35000), three_d=bevel("riblet", 50800, 25400))],
                      [solid(), solid(tint(90000)), grad([(0, ""), (50000, tint(75000)), (100000, shade(85000))], 5400000)]))
    # Reflection: a mirror image under the shape
    out.append(scheme("Reflection",
                      [solid(), grad([(0, tint(80000)), (100000, "")], 5400000), grad([(0, tint(65000)), (100000, shade(85000))], 5400000)],
                      [line(9525), line(19050), line(25400)],
                      [effect(), effect(reflection(30000, 0)), effect(reflection(45000, 38100), shadow(38100, 19050, 30000))],
                      [solid(), solid(tint(92000)), grad([(0, tint(80000)), (100000, shade(80000))], 5400000)]))
    # Extreme Shadow: long, dark shadows
    out.append(scheme("Extreme Shadow",
                      [solid(), solid(tint(90000)), grad([(0, tint(80000)), (100000, shade(80000))], 5400000)],
                      [line(9525), line(19050), line(28575)],
                      [effect(shadow(50800, 38100, 45000, 2700000)), effect(shadow(76200, 63500, 55000, 2700000)), effect(shadow(101600, 88900, 65000, 2700000))],
                      [solid(), solid(tint(90000)), grad([(0, tint(85000)), (100000, shade(70000))], 5400000)]))
    # Glossy: shiny highlights on the top half and a strong bevel
    out.append(scheme("Glossy",
                      [solid(), grad([(0, tint(70000) + sat(130000)), (45000, tint(90000)), (50000, ""), (100000, shade(85000) + sat(120000))], 5400000),
                       grad([(0, tint(55000) + sat(140000)), (48000, tint(80000)), (52000, shade(95000)), (100000, shade(70000) + sat(130000))], 5400000)],
                      [line(6350, solid(shade(80000))), line(12700, solid(shade(75000))), line(19050, solid(shade(65000)))],
                      [effect(shadow(25400, 12700, 30000)), effect(shadow(38100, 19050, 35000), three_d=bevel("circle", 38100, 25400, "plastic")),
                       effect(shadow(50800, 25400, 40000), three_d=bevel("circle", 63500, 38100, "plastic"))],
                      [solid(), grad([(0, tint(80000)), (50000, ""), (100000, shade(80000))], 5400000),
                       grad([(0, tint(60000) + sat(140000)), (100000, shade(70000))], path="circle")]))
    # Grunge Texture: rough, darkened edges
    out.append(scheme("Grunge Texture",
                      [solid(), grad([(0, shade(90000)), (100000, shade(70000))], path="rect"), grad([(0, tint(80000)), (100000, shade(55000))], path="rect")],
                      [line(15875, solid(shade(60000)), "sysDash"), line(25400, solid(shade(55000))), line(38100, solid(shade(45000)))],
                      [effect(soft_edge(6350)), effect(shadow(38100, 12700, 50000, outer=False)), effect(shadow(50800, 25400, 55000), shadow(63500, 19050, 50000, outer=False))],
                      [solid(), grad([(0, shade(90000)), (100000, shade(65000))], path="rect"), grad([(0, tint(70000)), (100000, shade(50000))], path="rect")]))
    return out


def schemes():
    """[(name, fmtScheme xml)] in the gallery's order (Office first, from the default theme)."""
    import re
    out = []
    for x in _schemes():
        name = re.search(r'name="([^"]*)"', x).group(1)
        out.append((name, x))
    return out


def theme_with(theme_xml, fmt_xml):
    """A theme's XML with its effects replaced."""
    import re
    m = re.search(r"<a:fmtScheme\b.*?</a:fmtScheme>", theme_xml, re.S)
    if m:
        return theme_xml[:m.start()] + fmt_xml + theme_xml[m.end():]
    return theme_xml
