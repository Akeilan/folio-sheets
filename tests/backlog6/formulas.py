# Backlog 6 probe: worksheet formulas against the values Excel gives.
import sys, math
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2])
S = 0
for i, v in enumerate([1, 2, 3, 4, 5]): b.set_input(S, i, 0, str(v))
for i, v in enumerate([10, 20, 30, 40, 50]): b.set_input(S, i, 1, str(v))
for i, v in enumerate(["apple", "Banana", "cherry", "date", "apple"]): b.set_input(S, i, 2, v)
b.set_input(S, 0, 4, "TRUE"); b.set_input(S, 1, 4, "text"); b.set_input(S, 2, 4, "3.5")
b.set_input(S, 0, 5, "2/29/2024")
E = lambda s: ("error", s)
J = lambda f: 'TEXTJOIN(",",TRUE,%s)' % f
T = [
# operators and coercion
("1+2*3", 7), ("2^3^2", 64), ("-2^2", 4), ("10%*200", 20), ('"3"+4', 7), ("1/0", E("#DIV/0!")), ('"a"+1', E("#VALUE!")),
("5&6", "56"), ('"a"="A"', True), ('"a"<"B"', True), ("TRUE>5", True), ('"a">5', True), ('1="1"', False), ('2<"1"', True),
("TRUE=1", False), ("TRUE+TRUE", 2), ('--"5"', 5), ('"2024-01-01"+0', 45292), ('"12:00"+0', 0.5), ('"1e3"+0', 1000),
('" 5 "+0', 5), ('"5%"+0', 0.05), ('"$1,000"+0', 1000), ('"(5)"+0', -5), ('"1 1/2"+0', 1.5), ("5%", 0.05), ("0^0", E("#NUM!")),
("POWER(0,0)", E("#NUM!")), ("1E+308*10", E("#NUM!")), ("0.1+0.2=0.3", True), ('""=0', False), ("D1=0", True), ('D1=""', True),
("LEN(D1)", 0), ("D1+1", 1), ("SUM((A1:A2,B1))", 13), ("SUM(A1:A2 A2:A3)", 2), ("SUM(A:A)", 15), ("SUM(1:1)", 45362),
("2*-3", -6), ("2^-1", 0.5), ("1-(-1)", 2), ("3>2>1", True), ("1<>1", False), ('"abc"&1+2', "abc3"), ("(1+2)*3%", 0.09),
("{1,2,3}", 1), ("SUM({1,2;3,4})", 10), ("1=1=TRUE", True), ("2>=2", True), ("-A1%", -0.01),
# math
("ROUND(2.5,0)", 3), ("ROUND(-2.5,0)", -3), ("ROUND(1234.567,-2)", 1200), ("ROUND(1.005,2)", 1.01), ("ROUNDUP(1.1,0)", 2),
("ROUNDDOWN(-1.9,0)", -1), ("INT(-1.5)", -2), ("TRUNC(-1.5)", -1), ("TRUNC(3.14159,2)", 3.14), ("MOD(-3,2)", 1), ("MOD(3,-2)", -1),
("MOD(5,0)", E("#DIV/0!")), ("MOD(5.5,2)", 1.5), ("CEILING(2.1,1)", 3), ("CEILING(-2.1,1)", -2), ("CEILING(-2.1,-1)", -3),
("FLOOR(-2.1,-1)", -2), ("FLOOR(2.1,-1)", E("#NUM!")), ("FLOOR(2.7,0.5)", 2.5), ("CEILING.MATH(-2.5)", -2), ("CEILING.MATH(-2.5,1,1)", -3),
("FLOOR.MATH(-2.5)", -3), ("FLOOR.MATH(-2.5,1,1)", -2), ("MROUND(10,3)", 9), ("MROUND(-10,3)", E("#NUM!")), ("MROUND(1.3,0.5)", 1.5),
("EVEN(1.5)", 2), ("ODD(2)", 3), ("EVEN(-1)", -2), ("ODD(0)", 1), ("SIGN(-3)", -1), ("ABS(-3)", 3), ("SQRT(-1)", E("#NUM!")),
("POWER(2,10)", 1024), ("EXP(1)", math.e), ("LN(1)", 0), ("LN(0)", E("#NUM!")), ("LOG(100)", 2), ("LOG(8,2)", 3), ("LOG10(1000)", 3),
("FACT(5)", 120), ("FACT(0)", 1), ("FACT(-1)", E("#NUM!")), ("FACT(5.9)", 120), ("FACTDOUBLE(7)", 105), ("COMBIN(5,2)", 10),
("COMBINA(4,3)", 20), ("PERMUT(5,2)", 20), ("PERMUTATIONA(3,2)", 9), ("GCD(12,18)", 6), ("LCM(4,6)", 12), ("QUOTIENT(7,2)", 3),
("QUOTIENT(-7,2)", -3), ("PI()", math.pi), ("RADIANS(180)", math.pi), ("DEGREES(PI())", 180), ("SIN(0)", 0), ("COS(0)", 1),
("ATAN2(1,1)", math.pi / 4), ("ATAN2(0,0)", E("#DIV/0!")), ("SUMSQ(3,4)", 25), ("PRODUCT(A1:A5)", 120), ("SUM(A1:A5)", 15),
("SUM(A1:B5)", 165), ("SUMPRODUCT(A1:A5,B1:B5)", 550), ('ROMAN(1999)', "MCMXCIX"), ('ARABIC("MCMXCIX")', 1999), ("BASE(255,16)", "FF"),
("BASE(5,2,8)", "00000101"), ('DECIMAL("FF",16)', 255), ("MULTINOMIAL(2,3,4)", 1260), ("SERIESSUM(2,0,1,{1,2,3})", 17),
("SQRTPI(1)", math.sqrt(math.pi)), ("MDETERM({1,2;3,4})", -2), ("ISO.CEILING(-2.1)", -2), ("CEILING.PRECISE(-2.1)", -2),
("FLOOR.PRECISE(-2.1)", -3), ("BITAND(12,10)", 8), ("BITOR(12,10)", 14), ("BITXOR(12,10)", 6), ("BITLSHIFT(1,4)", 16),
("BITRSHIFT(16,2)", 4), ("DELTA(1,1)", 1), ("GESTEP(5,4)", 1), ("ACOS(2)", E("#NUM!")), ("COT(0)", E("#DIV/0!")), ("SINH(0)", 0),
("ACOT(1)", math.pi / 4), ("SEC(0)", 1), ("CSC(PI()/2)", 1), ("ASINH(0)", 0), ("ATANH(0.5)", 0.5493061443340549), ("TAN(PI()/4)", 1),
("SUMX2MY2({2,3},{1,1})", 11), ("SUMXMY2({2,3},{1,1})", 5), ("RANDBETWEEN(3,3)", 3), ("AND(RAND()>=0,RAND()<1)", True),
# statistics
("AVERAGE(A1:A5)", 3), ("MEDIAN(A1:A5)", 3), ("MEDIAN(1,2,3,4)", 2.5), ("MODE(1,2,2,3)", 2), ("MODE(1,2,3)", E("#N/A")),
("STDEV(A1:A5)", 1.5811388300841898), ("STDEVP(A1:A5)", 1.4142135623730951), ("VAR(A1:A5)", 2.5), ("VARP(A1:A5)", 2), ("VAR.S(A1:A5)", 2.5),
("STDEV.P(A1:A5)", 1.4142135623730951), ("MAX(A1:B5)", 50), ("MIN(A1:B5)", 1), ("LARGE(B1:B5,2)", 40), ("SMALL(B1:B5,2)", 20),
("LARGE(B1:B5,9)", E("#NUM!")), ("RANK(30,B1:B5)", 3), ("RANK(10,B1:B5,1)", 1), ("RANK.AVG(2,{1,2,2,3})", 2.5), ("RANK(35,B1:B5)", E("#N/A")),
("COUNT(A1:C5)", 10), ("COUNTA(A1:C5)", 15), ("COUNTBLANK(A1:D5)", 5), ('COUNTIF(C1:C5,"apple")', 2), ('COUNTIF(A1:A5,">2")', 3),
('COUNTIF(C1:C5,"*an*")', 1), ('COUNTIF(C1:C5,"?pple")', 2), ('SUMIF(C1:C5,"apple",A1:A5)', 6), ('SUMIF(A1:A5,">2")', 12),
('SUMIFS(B1:B5,A1:A5,">1",A1:A5,"<5")', 90), ('AVERAGEIF(A1:A5,">2")', 4), ('AVERAGEIF(A1:A5,">9")', E("#DIV/0!")),
("CORREL(A1:A5,B1:B5)", 1), ("SLOPE(B1:B5,A1:A5)", 10), ("INTERCEPT(B1:B5,A1:A5)", 0), ("FORECAST(6,B1:B5,A1:A5)", 60),
("FORECAST.LINEAR(6,B1:B5,A1:A5)", 60), ("PERCENTILE(A1:A5,0.5)", 3), ("PERCENTILE(A1:A5,0.1)", 1.4), ("QUARTILE(A1:A5,1)", 2),
("PERCENTILE.EXC(A1:A5,0.25)", 1.5), ("PERCENTILE.EXC(A1:A5,0.1)", E("#NUM!")), ("GEOMEAN(1,2,4)", 2), ("HARMEAN(1,2,4)", 12 / 7),
("AVEDEV(A1:A5)", 1.2), ("DEVSQ(A1:A5)", 10), ("SKEW(A1:A5)", 0), ("KURT(A1:A5)", -1.2), ("TRIMMEAN(A1:A5,0.4)", 3),
("NORM.S.DIST(0,TRUE)", 0.5), ("NORM.S.INV(0.975)", 1.959963984540054), ("NORM.DIST(1,0,1,FALSE)", 0.24197072451914337),
("NORM.INV(0.5,10,2)", 10), ("NORMSDIST(1.96)", 0.9750021048517795), ("BINOM.DIST(2,5,0.5,FALSE)", 0.3125), ("BINOM.DIST(2,5,0.5,TRUE)", 0.5),
("POISSON.DIST(2,3,FALSE)", 0.22404180765538775), ("EXPON.DIST(1,1,TRUE)", 0.6321205588285577), ("T.DIST.2T(2,10)", 0.07338803477074037),
("T.INV.2T(0.05,10)", 2.2281388519649385), ("CHISQ.INV.RT(0.05,1)", 3.841458820694124), ("CHISQ.DIST.RT(3.841458820694124,1)", 0.05),
("GAMMA(5)", 24), ("GAMMALN(5)", 3.1780538303479458), ("FISHER(0.5)", 0.5493061443340549), ("FISHERINV(0.5493061443340549)", 0.5),
("STANDARDIZE(5,3,2)", 1), ("CONFIDENCE.NORM(0.05,1,100)", 0.1959963984540054), ("SUBTOTAL(9,A1:A5)", 15), ("SUBTOTAL(109,A1:A5)", 15),
("SUBTOTAL(1,A1:A5)", 3), ("SUBTOTAL(3,C1:C5)", 5), ("AGGREGATE(9,6,A1:A5)", 15), ("AGGREGATE(14,6,A1:A5,2)", 4),
("HYPGEOM.DIST(1,4,8,20,FALSE)", 0.3632610939112487), ("WEIBULL.DIST(1,1,1,TRUE)", 0.6321205588285577), ("LOGNORM.DIST(1,0,1,TRUE)", 0.5),
("COVARIANCE.P(A1:A5,B1:B5)", 20), ("COVARIANCE.S(A1:A5,B1:B5)", 25), ("COVAR(A1:A5,B1:B5)", 20), ("RSQ(A1:A5,B1:B5)", 1),
("PEARSON(A1:A5,B1:B5)", 1), ("PROB({1,2,3},{0.2,0.5,0.3},2)", 0.5), ("ERF(1)", 0.8427007929497149), ("ERFC(1)", 0.1572992070502851),
("BETA.DIST(0.5,2,2,TRUE)", 0.5), ("GAUSS(0)", 0), ("PHI(0)", 0.3989422804014327), ("PERCENTRANK(A1:A5,3)", 0.5),
("PERCENTRANK.EXC(A1:A5,3)", 0.5), ("F.DIST.RT(1,5,5)", 0.5), ("T.DIST(0,10,TRUE)", 0.5), ("CHISQ.DIST(0,2,TRUE)", 0),
("NEGBINOM.DIST(0,1,0.5,FALSE)", 0.5), ("BINOM.INV(5,0.5,0.5)", 2), ("Z.TEST(A1:A5,3)", 0.5), ("STEYX(B1:B5,A1:A5)", 0),
("MAXA(E1:E3)", 3.5), ("MINA(E1:E3)", 0), ("VARA(1,2,3)", 1), ("STDEVA(1,2,3)", 1), ("MODE.SNGL(1,1,2)", 1), ("COUNT(1,\"2\",TRUE,\"x\")", 3),
("SUMSQ(A1:A5)", 55), ("T.TEST({1,2,3},{1,2,3},2,1)", E("#DIV/0!")), ("GROWTH({2,4,8},{1,2,3},4)", 16), ("TREND({2,4,6},{1,2,3},4)", 8),
(J("LINEST(B1:B5,A1:A5)"), "10,0"), (J("FREQUENCY({1,2,3,4,5},{2,4})"), "2,2,1"), (J("MODE.MULT({1,1,2,2,3})"), "1,2"),
# criteria and aggregation semantics
('SUM("a")', E("#VALUE!")), ("SUM(C1:C5)", 0), ("SUM(E1:E3)", 3.5), ("SUM(TRUE,1)", 2), ("AVERAGE(E1:E3)", 3.5), ("AVERAGEA(E1:E3)", 1.5),
("COUNT(E1:E3)", 1), ("COUNTA(E1:E3)", 3), ("MAX(C1:C5)", 0), ('COUNTIF(A1:A5,"<>3")', 4), ('COUNTIF(C1:C5,"<>apple")', 3),
('COUNTIF(D1:D5,"")', 5), ("COUNTIF(A1:A5,3)", 1), ('COUNTIF(C1:C5,"APPLE")', 2), ('SUMIF(A1:A5,"3",B1:B5)', 30), ('COUNTIF(A1:A5,">=2")', 4),
('COUNTIFS(A1:A5,">1",C1:C5,"apple")', 1), ('MAXIFS(B1:B5,C1:C5,"apple")', 50), ('MINIFS(B1:B5,A1:A5,">2")', 30),
('AVERAGEIFS(B1:B5,A1:A5,">2")', 40), ("SUMPRODUCT((A1:A5>2)*B1:B5)", 120), ("SUMPRODUCT(A1:A5,C1:C5)", 0),
("LOOKUP(2,1/(A1:A5>2),B1:B5)", 50), ("INDEX(A1:A5,MATCH(MAX(B1:B5),B1:B5,0))", 5), ('MATCH("APPLE",C1:C5,0)', 1),
("SUM(A1:A5*B1:B5)", 550), ("SUM(IF(A1:A5>2,B1:B5))", 120), ('SUM(--(C1:C5="apple"))', 2), ('COUNTIF(C1:C5,"~*")', 0),
('SUMIF(C1:C5,"a*",B1:B5)', 60), ('COUNTIF(E1:E3,TRUE)', 1), ('COUNTIF(A1:A5,"=3")', 1), ('SUMIF(A1:A5,">"&2,B1:B5)', 120),
('COUNTIF(F1,">1/1/2024")', 1), ('SUMIFS(B1:B5,C1:C5,"apple",A1:A5,1)', 10), ('COUNTIF(C1:C5,"<b")', 2),
# text
('LEN("héllo")', 5), ('LEFT("hello",2)', "he"), ('RIGHT("hello",2)', "lo"), ('MID("hello",2,3)', "ell"), ('MID("hello",10,2)', ""),
('LEFT("hello")', "h"), ('LEFT("hello",-1)', E("#VALUE!")), ('UPPER("abc")', "ABC"), ('LOWER("ABC")', "abc"),
('PROPER("hello wORLD o\'neil 2nd")', "Hello World O'Neil 2Nd"), ('TRIM("  a   b  ")', "a b"), ('CONCATENATE("a",1,TRUE)', "a1TRUE"),
("CONCAT(A1:A3)", "123"), ('TEXTJOIN(",",TRUE,C1:C3,"")', "apple,Banana,cherry"), ('TEXTJOIN("-",FALSE,"a","","b")', "a--b"),
('FIND("l","hello")', 3), ('FIND("L","hello")', E("#VALUE!")), ('FIND("l","hello",4)', 4), ('SEARCH("L","hello")', 3), ('SEARCH("l?o","hello")', 3),
('SEARCH("~?","a?b")', 2), ('SUBSTITUTE("aaa","a","b",2)', "aba"), ('SUBSTITUTE("aaa","a","b")', "bbb"), ('REPLACE("hello",2,3,"XY")', "hXYo"),
('REPT("ab",3)', "ababab"), ('EXACT("a","A")', False), ("CHAR(65)", "A"), ('CODE("A")', 65), ("UNICHAR(8364)", "€"), ('UNICODE("€")', 8364),
('VALUE("1,234.5")', 1234.5), ('VALUE("12%")', 0.12), ('VALUE("$5")', 5), ('VALUE("abc")', E("#VALUE!")), ('VALUE("")', E("#VALUE!")),
('NUMBERVALUE("1.234,5",",",".")', 1234.5), ('T("a")', "a"), ("T(1)", ""), ('N("a")', 0), ("N(TRUE)", 1), ("FIXED(1234.567,1)", "1,234.6"),
("FIXED(1234.567,-2)", "1,200"), ("FIXED(1234.567,1,TRUE)", "1234.6"), ("FIXED(0.5,0)", "1"), ("DOLLAR(1234.567)", "$1,234.57"),
("DOLLAR(-1234.567,0)", "($1,235)"), ('TEXT(1234.5,"#,##0.00")', "1,234.50"), ('TEXT(0.256,"0.0%")', "25.6%"),
('TEXT(45000,"yyyy-mm-dd")', "2023-03-15"), ('TEXT(45000.5,"hh:mm AM/PM")', "12:00 PM"), ('TEXT(1234567,"0.00E+00")', "1.23E+06"),
('TEXT(-5,"0;(0)")', "(5)"), ('TEXT(0.5,"# ?/?")', " 1/2"), ('TEXT(1.5,"# ?/?")', "1 1/2"), ('TEXT(5,"000")', "005"), ('TEXT("abc","0")', "abc"),
('TEXT(45000,"dddd, mmmm d, yyyy")', "Wednesday, March 15, 2023"), ('TEXT(45000,"ddd mmm yy")', "Wed Mar 23"), ('TEXT(1.5,"[h]:mm")', "36:00"),
('TEXT(12345678,"#,##0,,""M""")', "12M"), ('TEXT(12,"0.0#")', "12.0"), ('TEXT(TRUE,"0")', "TRUE"), ('TEXT(0.75,"h:mm")', "18:00"),
('TEXT(1234.5,"$#,##0.00")', "$1,234.50"), ('TEXT(5,"General")', "5"), ('TEXT(45351,"mm/dd/yy")', "02/29/24"), ('TEXT(0.5,"0%")', "50%"),
('TEXT(1234,"0000000")', "0001234"), ('TEXT(45351,"d-mmm")', "29-Feb"), ('TEXT(2.5,"0")', "3"), ('TEXT(-2.5,"0")', "-3"), ('TEXT(1E+20,"0")', "100000000000000000000"),
('TEXTBEFORE("a-b-c","-")', "a"), ('TEXTAFTER("a-b-c","-")', "b-c"), ('TEXTAFTER("a-b-c","-",-1)', "c"), ('TEXTBEFORE("abc","x")', E("#N/A")),
('TEXTBEFORE("a-b-c","-",2)', "a-b"), ('VALUETOTEXT("a",1)', '"a"'), ("ARRAYTOTEXT({1,2;3,4})", "1, 2, 3, 4"), ("ARRAYTOTEXT({1,2;3,4},1)", "{1,2;3,4}"),
('CLEAN("a"&CHAR(10)&"b")', "ab"), ('LEN(REPT("x",32767))', 32767), ('REPT("x",32768)', E("#VALUE!")), ('ENCODEURL("a b&c")', "a%20b%26c"),
('LENB("abc")', 3), ('"a"&TRUE', "aTRUE"), ('"x"&1.50', "x1.5"), ('"x"&1/3', "x0.333333333333333"), ('"x"&1E+20', "x1E+20"), ('""&0.1+0.2', "0.3"),
('""&123456789012345678', "1.23456789012346E+17"), ('""&DATE(2024,1,1)', "45292"), ('TEXT(,"0")', "0"),
# logical and information
('IF(1,"y","n")', "y"), ('IF(0,"y")', False), ('IF(1,,"n")', 0), ('IFERROR(1/0,"e")', "e"), ('IFNA(NA(),"x")', "x"), ("IFNA(1/0,1)", E("#DIV/0!")),
("IFS(FALSE,1,TRUE,2)", 2), ("IFS(FALSE,1)", E("#N/A")), ('SWITCH(2,1,"a",2,"b","z")', "b"), ('SWITCH(9,1,"a")', E("#N/A")), ('SWITCH(9,1,"a","z")', "z"),
("AND(A1:A5)", True), ("OR(0,0)", False), ("XOR(TRUE,TRUE,TRUE)", True), ("NOT(0)", True), ("ISBLANK(D1)", True), ('ISBLANK("")', False),
('ISNUMBER("1")', False), ('ISTEXT("1")', True), ("ISERROR(NA())", True), ("ISERR(NA())", False), ("ISNA(NA())", True), ("ISLOGICAL(TRUE)", True),
("ISEVEN(2.5)", True), ("ISODD(3)", True), ("ISNONTEXT(1)", True), ("TYPE(1)", 1), ('TYPE("a")', 2), ("TYPE(TRUE)", 4), ("TYPE(NA())", 16),
("TYPE({1,2})", 64), ("ERROR.TYPE(1/0)", 2), ("ERROR.TYPE(NA())", 7), ("ERROR.TYPE(1)", E("#N/A")), ("ISREF(A1)", True), ("ISREF(1)", False),
("ISFORMULA(A1)", False), ("NA()", E("#N/A")), ('AND("a",TRUE)', E("#VALUE!")), ("TRUE()", True), ("OR(C1:C5)", E("#VALUE!")),
('CELL("row",B3)', 3), ('CELL("col",B3)', 2), ('CELL("address",B3)', "$B$3"), ('CELL("contents",A2)', 2), ('CELL("type",A1)', "v"), ('CELL("type",C1)', "l"),
('INFO("numfile")', 1), ("ISERROR(#REF!)", True), ("#N/A", E("#N/A")), ("ERROR.TYPE(#NAME?)", 5), ("ERROR.TYPE(#NULL!)", 1), ("NOSUCHFN(1)", E("#NAME?")),
# lookup and reference
("ROW(A5)", 5), ("COLUMN(C1)", 3), ("ROWS(A1:B5)", 5), ("COLUMNS(A1:B5)", 2), ("ADDRESS(1,1)", "$A$1"), ("ADDRESS(2,3,4)", "C2"),
("ADDRESS(2,3,2)", "C$2"), ("ADDRESS(1,1,1,FALSE)", "R1C1"), ('ADDRESS(1,1,1,TRUE,"Sheet 2")', "'Sheet 2'!$A$1"), ('INDIRECT("A"&2)', 2),
('INDIRECT("R1C2",FALSE)', 10), ('SUM(INDIRECT("A1:A5"))', 15), ('INDIRECT("zz!!")', E("#REF!")), ("OFFSET(A1,1,1)", 20),
("SUM(OFFSET(A1,0,0,5,1))", 15), ("OFFSET(A1,-1,0)", E("#REF!")), ("INDEX(B1:B5,3)", 30), ("INDEX(A1:B5,2,2)", 20), ("SUM(INDEX(A1:B5,0,2))", 150),
("INDEX(A1:B5,9,1)", E("#REF!")), ("SUM(A1:INDEX(A1:A5,3))", 6), ("MATCH(30,B1:B5,0)", 3), ("MATCH(35,B1:B5,1)", 3), ("MATCH(35,B1:B5,0)", E("#N/A")),
('MATCH("b*",C1:C5,0)', 2), ("MATCH(35,{50,40,30,20,10},-1)", 2), ("VLOOKUP(3,A1:B5,2,FALSE)", 30), ("VLOOKUP(3.5,A1:B5,2)", 30),
("VLOOKUP(0,A1:B5,2)", E("#N/A")), ("VLOOKUP(3,A1:B5,3,FALSE)", E("#REF!")), ('VLOOKUP("APPLE",C1:C5,1,FALSE)', "apple"),
('HLOOKUP(2,{1,2,3;"a","b","c"},2,FALSE)', "b"), ("LOOKUP(3.5,A1:A5,B1:B5)", 30), ("LOOKUP(0,A1:A5,B1:B5)", E("#N/A")), ("LOOKUP(99,A1:B5)", 50),
("XLOOKUP(3,A1:A5,B1:B5)", 30), ('XLOOKUP(9,A1:A5,B1:B5,"none")', "none"), ("XLOOKUP(3.5,A1:A5,B1:B5,,-1)", 30), ("XLOOKUP(3.5,A1:A5,B1:B5,,1)", 40),
('XLOOKUP("b*",C1:C5,A1:A5,,2)', 2), ('XLOOKUP("apple",C1:C5,A1:A5,,0,-1)', 5), ("XMATCH(4,A1:A5)", 4), ("XMATCH(3.5,A1:A5,1)", 4),
('CHOOSE(2,"a","b")', "b"), ('CHOOSE(5,"a")', E("#VALUE!")), ("SUM(CHOOSE(2,A1:A5,B1:B5))", 150), ("SUM(TRANSPOSE(A1:A5))", 15), ("AREAS((A1:A2,B1))", 2),
("SHEET()", 1), ("SHEETS()", 1), ("ROW()", 8), ("COLUMN()", 8), ("ROWS({1,2;3,4;5,6})", 3), ("SUM(Sheet1!A1:A5)", 15), ("SUM($A$1:$A$5)", 15),
# date and time
("DATE(2024,2,29)", 45351), ("DATE(2024,13,1)", 45658), ("DATE(2024,1,0)", 45291), ("DATE(1900,1,1)", 1), ("DATE(1900,3,1)", 61),
("DATE(1900,2,29)", 60), ("DATE(0,1,1)", 1), ("DATE(10000,1,1)", E("#NUM!")), ("DATE(1899,12,31)", 693962 + 0 if False else None),
("YEAR(45351)", 2024), ("MONTH(45351)", 2), ("DAY(45351)", 29), ("DAY(60)", 29), ("MONTH(60)", 2), ("YEAR(0)", 1900), ("WEEKDAY(45351)", 5),
("WEEKDAY(45351,2)", 4), ("WEEKDAY(45351,3)", 3), ("WEEKDAY(1)", 1), ("WEEKDAY(45351,11)", 4), ("WEEKDAY(45351,16)", 6), ("WEEKNUM(DATE(2024,1,1))", 1),
("WEEKNUM(DATE(2024,12,31))", 53), ("WEEKNUM(DATE(2024,1,7),2)", 1), ("WEEKNUM(DATE(2024,1,8),2)", 2), ("WEEKNUM(DATE(2021,1,1),21)", 53),
("ISOWEEKNUM(DATE(2024,12,30))", 1), ("ISOWEEKNUM(DATE(2021,1,1))", 53), ("EDATE(DATE(2024,1,31),1)", 45351), ("EOMONTH(DATE(2024,2,1),0)", 45351),
("EOMONTH(DATE(2024,1,15),-1)", 45291), ('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"y")', 4), ('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"m")', 49),
('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"d")', 1506), ('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"md")', 14),
('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"ym")', 1), ('DATEDIF(DATE(2020,1,15),DATE(2024,2,29),"yd")', 45),
('DATEDIF(DATE(2024,2,29),DATE(2020,1,15),"d")', E("#NUM!")), ("DAYS(DATE(2024,3,1),DATE(2024,2,1))", 29),
("DAYS360(DATE(2024,1,31),DATE(2024,2,29))", 29), ("DAYS360(DATE(2024,2,29),DATE(2024,3,31))", 30), ("DAYS360(DATE(2024,1,30),DATE(2024,3,31),TRUE)", 60),
("NETWORKDAYS(DATE(2024,1,1),DATE(2024,1,31))", 23), ("NETWORKDAYS(DATE(2024,1,1),DATE(2024,1,31),DATE(2024,1,15))", 22),
("NETWORKDAYS(DATE(2024,1,31),DATE(2024,1,1))", -23), ("WORKDAY(DATE(2024,1,1),10)", 45306), ("WORKDAY(DATE(2024,1,5),1)", 45299),
("WORKDAY(DATE(2024,1,8),-1)", 45296), ("NETWORKDAYS.INTL(DATE(2024,1,1),DATE(2024,1,31),11)", 27),
('NETWORKDAYS.INTL(DATE(2024,1,1),DATE(2024,1,31),"0000011")', 23), ("WORKDAY.INTL(DATE(2024,1,5),1,7)", 45297),
("YEARFRAC(DATE(2024,1,1),DATE(2024,7,1))", 0.5), ("YEARFRAC(DATE(2024,1,1),DATE(2024,7,1),1)", 182 / 366), ("YEARFRAC(DATE(2024,1,1),DATE(2024,7,1),2)", 182 / 360),
("YEARFRAC(DATE(2024,1,1),DATE(2024,7,1),3)", 182 / 365), ("TIME(12,30,0)", 0.5208333333333334), ("TIME(25,0,0)", 1 / 24), ("HOUR(0.75)", 18),
("MINUTE(0.5208333333333334)", 30), ("SECOND(TIME(1,2,3))", 3), ('DATEVALUE("2024-02-29")', 45351), ('DATEVALUE("29-Feb-2024")', 45351),
('DATEVALUE("February 29, 2024")', 45351), ('DATEVALUE("2/30/2024")', E("#VALUE!")), ('DATEVALUE("2/29/24")', 45351), ('DATEVALUE("1/1/30")', 10959),
('DATEVALUE("1/1/29")', 47119), ('TIMEVALUE("6:00 PM")', 0.75), ('TIMEVALUE("18:30:15")', 0.7710069444444445), ('TEXT(60,"m/d/yyyy")', "2/29/1900"),
('TEXT(0,"m/d/yyyy")', "1/0/1900"), ("F1", 45351), ("YEAR(F1)", 2024), ("F1+1", 45352), ("INT(NOW())=TODAY()", True), ("TODAY()>46000", True),
("YEAR(-1)", E("#NUM!")), ("HOUR(1.5)", 12), ("DAY(DATE(2023,2,29))", 1), ("MONTH(DATE(2024,0,15))", 12), ('DAYS("3/1/2024","2/1/2024")', 29),
# financial
("PMT(0.05/12,60,10000)", -188.71233644010877), ("FV(0.05,10,-100)", 1257.7892535548839), ("PV(0.05,10,-100)", 772.1734929184813),
("NPER(0.05,-100,1000)", 14.206699082890463), ("RATE(10,-100,772.1734929184813)", 0.05), ("NPV(0.1,100,200,300)", 481.5927873779113),
("IRR({-100,60,60})", 0.13066238629180749), ("SLN(1000,100,5)", 180), ("SYD(1000,100,5,1)", 300), ("DDB(1000,100,5,1)", 400), ("DB(1000,100,5,1)", 369),
("IPMT(0.05/12,1,60,10000)", -41.666666666666664), ("PPMT(0.05/12,1,60,10000)", -147.04566977344210), ("EFFECT(0.05,12)", 0.05116189788173342),
("NOMINAL(0.05116189788173342,12)", 0.05), ("MIRR({-100,60,60},0.1,0.1)", math.sqrt(1.26) - 1), ("DOLLARDE(1.02,16)", 1.125), ("DOLLARFR(1.125,16)", 1.02),
("RRI(10,100,200)", 0.0717734625362931), ("PDURATION(0.05,100,200)", 14.206699082890463), ("ISPMT(0.05,1,10,1000)", -45), ("FVSCHEDULE(100,{0.1,0.2})", 132),
("PMT(0,10,1000)", -100), ("FV(0,10,-100)", 1000), ("XNPV(0.1,{-100,110},{45292,45658})", -100 + 110 / 1.1 ** (366 / 365)),
("XIRR({-100,110},{45292,45657})", 0.1), ("CUMPRINC(0.05/12,60,10000,1,1,0)", -147.04566977344210), ("CUMIPMT(0.05/12,60,10000,1,1,0)", -41.666666666666664),
("VDB(1000,100,5,0,1)", 400), ("DISC(DATE(2024,1,1),DATE(2025,1,1),95,100,2)", 0.05 * 360 / 366), ("TBILLPRICE(DATE(2024,1,1),DATE(2024,4,1),0.05)", 100 * (1 - 0.05 * 91 / 360)),
("ACCRINTM(DATE(2024,1,1),DATE(2025,1,1),0.1,1000,0)", 100), ("COUPNUM(DATE(2024,1,1),DATE(2026,1,1),2)", 4), ("IRR({1,2})", E("#NUM!")),
# engineering
("DEC2BIN(10)", "1010"), ("DEC2BIN(-1)", "1111111111"), ("DEC2BIN(512)", E("#NUM!")), ("DEC2HEX(255)", "FF"), ("DEC2HEX(-1)", "FFFFFFFFFF"),
('HEX2DEC("FF")', 255), ('HEX2DEC("FFFFFFFFFF")', -1), ('BIN2DEC("1010")', 10), ('BIN2DEC("1111111111")', -1), ('OCT2DEC("17")', 15),
("DEC2OCT(8)", "10"), ('BIN2HEX("1111")', "F"), ('HEX2BIN("F",8)', "00001111"), ('HEX2OCT("1F")', "37"), ('CONVERT(1,"mi","km")', 1.609344),
('CONVERT(100,"C","F")', 212), ('CONVERT(1,"lbm","kg")', 0.45359237), ('CONVERT(1,"hr","sec")', 3600), ('CONVERT(1,"m","xx")', E("#N/A")),
('CONVERT(1,"km","m")', 1000), ('CONVERT(1,"gal","l")', 3.785411784), ('CONVERT(1,"m","kg")', E("#N/A")), ('CONVERT(1,"byte","bit")', 8),
("COMPLEX(1,2)", "1+2i"), ('COMPLEX(0,1)', "i"), ('COMPLEX(1,-1,"j")', "1-j"), ('IMABS("3+4i")', 5), ('IMSUM("1+2i","3+4i")', "4+6i"),
('IMPRODUCT("1+2i","3+4i")', "-5+10i"), ('IMREAL("3+4i")', 3), ('IMAGINARY("3-4i")', -4), ('IMCONJUGATE("3+4i")', "3-4i"), ('IMSUB("4+6i","1+2i")', "3+4i"),
('IMDIV("-5+10i","1+2i")', "3+4i"), ('IMPOWER("i",2)', None), ("BESSELJ(1,0)", 0.765197686557967), ("ERF(0,1)", 0.8427007929497149),
# dynamic arrays
("SUM(SEQUENCE(4))", 10), (J("SEQUENCE(2,3)"), "1,2,3,4,5,6"), (J("SEQUENCE(2,2,10,5)"), "10,15,20,25"), (J("SORT({3;1;2})"), "1,2,3"),
(J("SORT({3;1;2},,-1)"), "3,2,1"), (J("SORT({3,1,2})"), "3,1,2"), (J("SORT({3,1,2},,,TRUE)"), "1,2,3"), (J("UNIQUE({1;2;2;3})"), "1,2,3"),
(J("UNIQUE({1;2;2;3},,TRUE)"), "1,3"), (J("FILTER(A1:A5,A1:A5>3)"), "4,5"), ("FILTER(A1:A5,A1:A5>9)", E("#CALC!")), ('FILTER(A1:A5,A1:A5>9,"none")', "none"),
(J("SORTBY(C1:C3,A1:A3,-1)"), "cherry,Banana,apple"), (J("SORT(C1:C5)"), "apple,apple,Banana,cherry,date"), (J("TAKE(A1:A5,2)"), "1,2"),
(J("TAKE(A1:A5,-2)"), "4,5"), (J("DROP(A1:A5,3)"), "4,5"), (J("CHOOSEROWS(A1:A5,1,-1)"), "1,5"), (J("CHOOSECOLS(A1:B2,2)"), "10,20"),
(J("HSTACK({1,2},{3})"), "1,2,3"), (J("VSTACK({1},{2;3})"), "1,2,3"), (J("TOCOL({1,2;3,4})"), "1,2,3,4"), (J("TOROW({1,2;3,4},,TRUE)"), "1,3,2,4"),
("ROWS(WRAPROWS({1,2,3,4,5},2))", 3), ("COLUMNS(WRAPCOLS({1,2,3,4,5},2))", 3), (J("EXPAND({1},2,2,0)"), "1,0,0,0"), ('ROWS(TEXTSPLIT("a,b;c",",",";"))', 2),
(J('TEXTSPLIT("a b c"," ")'), "a,b,c"), ("LET(x,2,y,3,x*y)", 6), ("LET(x,2,y,x+1,y*x)", 6), ("LAMBDA(x,x*2)(5)", 10),
(J("MAP({1,2,3},LAMBDA(x,x^2))"), "1,4,9"), ("REDUCE(0,{1,2,3},LAMBDA(a,b,a+b))", 6), (J("SCAN(0,{1,2,3},LAMBDA(a,b,a+b))"), "1,3,6"),
(J("BYROW({1,2;3,4},LAMBDA(r,SUM(r)))"), "3,7"), (J("BYCOL({1,2;3,4},LAMBDA(c,SUM(c)))"), "4,6"), (J("MAKEARRAY(2,2,LAMBDA(r,c,r*c))"), "1,2,2,4"),
(J("MMULT({1,2;3,4},{5;6})"), "17,39"), (J("MUNIT(2)"), "1,0,0,1"), (J("MINVERSE({1,2;3,4})"), "-2,1,1.5,-0.5"), ("COUNT(RANDARRAY(3,2))", 6),
(J("TRANSPOSE({1,2;3,4})"), "1,3,2,4"), (J("XLOOKUP(2,A1:A5,A1:B5)"), "2,20"), ("SUM(TAKE(SORT(B1:B5,,-1),2))", 90), (J("UNIQUE(C1:C5)"), "apple,Banana,cherry,date"),
(J("A1:A3&\"x\""), "1x,2x,3x"), (J("IF({1,0,1},\"y\",\"n\")"), "y,n,y"), ("SUM(ABS({-1,-2}))", 3), ("SUM(LEN(C1:C5))", 25), ("MAX(LEN(C1:C5))", 6),
(J("INDEX(A1:B5,{1,2},2)"), "10,20"), ("SUM(SEQUENCE(3)*SEQUENCE(1,3))", 36), ('REGEXTEST("abc","b")', True), ('REGEXEXTRACT("a1b22","[0-9]+")', "1"),
('REGEXREPLACE("a1b22","[0-9]+","#")', "a#b#"), ("PERCENTOF({1,2},{1,2,3,4})", 0.3), (J("TRIMRANGE(A1:A9)"), "1,2,3,4,5"),
(J('GROUPBY(C1:C5,A1:A5,SUM,,0)'), "apple,6,Banana,2,cherry,3,date,4"), ("ISOMITTED(1)", False), ("LAMBDA(x,y,ISOMITTED(y))(1)", True),
("SUM(SORT(A1:A5,1,-1)*{1;0;0;0;0})", 5), ("COUNTA(TEXTSPLIT(\"a,,b\",\",\",,TRUE))", 2), (J("DROP({1,2,3;4,5,6},1,1)"), "5,6"),
("SUM(XLOOKUP({1,2},A1:A5,B1:B5))", 30), ("ROWS(FILTER(A1:B5,C1:C5=\"apple\"))", 2), ("@A1:A5", None), ("SUM(A1:A5^2)", 55),
]
bad = 0; n = 0; missing = []
for f, want in T:
    if want is None: continue
    n += 1
    try:
        b.set_input(S, 7, 7, '=' + f)
        got = b.value(S, 7, 7)
    except Exception as e:
        got = ("EXC", str(e)[:80])
    ok = False
    if isinstance(want, tuple):
        ok = tuple(got) == want if isinstance(got, (tuple, list)) else False
    elif isinstance(want, bool):
        ok = got is want
    elif isinstance(want, (int, float)):
        ok = isinstance(got, (int, float)) and not isinstance(got, bool) and abs(got - want) <= 1e-9 * max(1, abs(want))
    else:
        ok = got == want
    if not ok:
        bad += 1
        print("DIFF  =%s   want %r   got %r" % (f, want, got))
print("formulas: %d checked, %d differ" % (n, bad))
fns = set(core.function_names())
ex = set(open(sys.argv[3]).read().split())
print("engine knows %d functions; of Excel's %d listed, missing: %s" % (len(fns), len(ex), " ".join(sorted(ex - fns))))
