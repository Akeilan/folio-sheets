"""The function catalog: category, argument list and a one-line
description for every worksheet function (descriptions are our own)."""

# name | category | arguments | description
_DATA = r"""
ABS|Math & Trig|number|The absolute value of a number.
ACOS|Math & Trig|number|The angle, in radians, whose cosine is the number.
ACOSH|Math & Trig|number|The inverse hyperbolic cosine.
ACOT|Math & Trig|number|The angle, in radians, whose cotangent is the number.
ACOTH|Math & Trig|number|The inverse hyperbolic cotangent.
AGGREGATE|Math & Trig|function_num, options, ref1, [ref2], ...|An aggregate (sum, average, max, …) that can skip hidden rows, errors and nested subtotals.
ARABIC|Math & Trig|text|Converts a Roman numeral to a number.
ASIN|Math & Trig|number|The angle, in radians, whose sine is the number.
ASINH|Math & Trig|number|The inverse hyperbolic sine.
ATAN|Math & Trig|number|The angle, in radians, whose tangent is the number.
ATAN2|Math & Trig|x_num, y_num|The angle, in radians, of the point (x, y) from the x-axis.
ATANH|Math & Trig|number|The inverse hyperbolic tangent.
BASE|Math & Trig|number, radix, [min_length]|Writes a number in another base (2–36) as text.
CEILING.MATH|Math & Trig|number, [significance], [mode]|Rounds up to the nearest integer or multiple of significance.
CEILING.PRECISE|Math & Trig|number, [significance]|Rounds up to a multiple of significance, whatever the sign.
COMBIN|Math & Trig|number, number_chosen|The number of combinations of items chosen from a set.
COMBINA|Math & Trig|number, number_chosen|Combinations with repetition allowed.
COS|Math & Trig|number|The cosine of an angle in radians.
COSH|Math & Trig|number|The hyperbolic cosine.
COT|Math & Trig|number|The cotangent of an angle in radians.
COTH|Math & Trig|number|The hyperbolic cotangent.
CSC|Math & Trig|number|The cosecant of an angle in radians.
CSCH|Math & Trig|number|The hyperbolic cosecant.
DECIMAL|Math & Trig|text, radix|Converts text in a given base to a decimal number.
DEGREES|Math & Trig|angle|Converts radians to degrees.
EVEN|Math & Trig|number|Rounds away from zero to the nearest even integer.
EXP|Math & Trig|number|e raised to a power.
FACT|Math & Trig|number|The factorial of a number.
FACTDOUBLE|Math & Trig|number|The double factorial (n·(n−2)·…).
FLOOR.MATH|Math & Trig|number, [significance], [mode]|Rounds down to the nearest integer or multiple of significance.
FLOOR.PRECISE|Math & Trig|number, [significance]|Rounds down to a multiple of significance, whatever the sign.
GCD|Math & Trig|number1, [number2], ...|The greatest common divisor.
INT|Math & Trig|number|Rounds down to the nearest integer.
ISO.CEILING|Math & Trig|number, [significance]|Rounds up to a multiple of significance (ISO rule).
LCM|Math & Trig|number1, [number2], ...|The least common multiple.
LN|Math & Trig|number|The natural logarithm.
LOG|Math & Trig|number, [base]|The logarithm in a given base (10 by default).
LOG10|Math & Trig|number|The base-10 logarithm.
MDETERM|Math & Trig|array|The determinant of a square matrix.
MINVERSE|Math & Trig|array|The inverse of a square matrix.
MMULT|Math & Trig|array1, array2|The matrix product of two arrays.
MOD|Math & Trig|number, divisor|The remainder after division (sign of the divisor).
MROUND|Math & Trig|number, multiple|Rounds to the nearest multiple.
MULTINOMIAL|Math & Trig|number1, [number2], ...|The multinomial coefficient of a set of numbers.
MUNIT|Math & Trig|dimension|The identity matrix of a given size.
ODD|Math & Trig|number|Rounds away from zero to the nearest odd integer.
PI|Math & Trig||The value of π.
POWER|Math & Trig|number, power|A number raised to a power.
PRODUCT|Math & Trig|number1, [number2], ...|Multiplies its arguments.
QUOTIENT|Math & Trig|numerator, denominator|The integer part of a division.
RADIANS|Math & Trig|angle|Converts degrees to radians.
RAND|Math & Trig||A random number from 0 up to 1 (recalculates).
RANDARRAY|Math & Trig|[rows], [columns], [min], [max], [whole_number]|An array of random numbers.
RANDBETWEEN|Math & Trig|bottom, top|A random whole number between two values (recalculates).
ROMAN|Math & Trig|number, [form]|Writes a number as Roman numerals.
ROUND|Math & Trig|number, num_digits|Rounds to a number of digits.
ROUNDDOWN|Math & Trig|number, num_digits|Rounds toward zero.
ROUNDUP|Math & Trig|number, num_digits|Rounds away from zero.
SEC|Math & Trig|number|The secant of an angle in radians.
SECH|Math & Trig|number|The hyperbolic secant.
SEQUENCE|Math & Trig|rows, [columns], [start], [step]|An array of sequential numbers.
SERIESSUM|Math & Trig|x, n, m, coefficients|The sum of a power series.
SIGN|Math & Trig|number|1, 0 or −1 by the sign of a number.
SIN|Math & Trig|number|The sine of an angle in radians.
SINH|Math & Trig|number|The hyperbolic sine.
SQRT|Math & Trig|number|The square root.
SQRTPI|Math & Trig|number|The square root of (number × π).
SUBTOTAL|Math & Trig|function_num, ref1, [ref2], ...|A subtotal that skips other subtotals (101–111 also skip hidden rows).
SUM|Math & Trig|number1, [number2], ...|Adds its arguments.
SUMIF|Math & Trig|range, criteria, [sum_range]|Adds the cells that meet a condition.
SUMIFS|Math & Trig|sum_range, criteria_range1, criteria1, ...|Adds the cells that meet several conditions.
SUMPRODUCT|Math & Trig|array1, [array2], ...|The sum of the products of corresponding items.
SUMSQ|Math & Trig|number1, [number2], ...|The sum of the squares.
SUMX2MY2|Math & Trig|array_x, array_y|The sum of the differences of squares.
SUMX2PY2|Math & Trig|array_x, array_y|The sum of the sums of squares.
SUMXMY2|Math & Trig|array_x, array_y|The sum of squared differences.
TAN|Math & Trig|number|The tangent of an angle in radians.
TANH|Math & Trig|number|The hyperbolic tangent.
TRUNC|Math & Trig|number, [num_digits]|Cuts a number to a number of digits.
CEILING|Compatibility|number, significance|Rounds up to a multiple of significance.
FLOOR|Compatibility|number, significance|Rounds down to a multiple of significance.
DATE|Date & Time|year, month, day|The serial number of a date.
DATEDIF|Date & Time|start_date, end_date, unit|Whole days, months or years between two dates.
DATEVALUE|Date & Time|date_text|Converts a date written as text to a serial number.
DAY|Date & Time|serial_number|The day of the month (1–31).
DAYS|Date & Time|end_date, start_date|The number of days between two dates.
DAYS360|Date & Time|start_date, end_date, [method]|Days between two dates on a 360-day year.
EDATE|Date & Time|start_date, months|The date a number of months before or after a date.
EOMONTH|Date & Time|start_date, months|The last day of the month a number of months away.
HOUR|Date & Time|serial_number|The hour (0–23).
ISOWEEKNUM|Date & Time|date|The ISO week number of the year.
MINUTE|Date & Time|serial_number|The minute (0–59).
MONTH|Date & Time|serial_number|The month (1–12).
NETWORKDAYS|Date & Time|start_date, end_date, [holidays]|Whole working days between two dates.
NETWORKDAYS.INTL|Date & Time|start_date, end_date, [weekend], [holidays]|Working days between two dates with a chosen weekend.
NOW|Date & Time||The current date and time (recalculates).
SECOND|Date & Time|serial_number|The second (0–59).
TIME|Date & Time|hour, minute, second|The serial number of a time.
TIMEVALUE|Date & Time|time_text|Converts a time written as text to a serial number.
TODAY|Date & Time||Today's date (recalculates).
WEEKDAY|Date & Time|serial_number, [return_type]|The day of the week as a number.
WEEKNUM|Date & Time|serial_number, [return_type]|The week number of the year.
WORKDAY|Date & Time|start_date, days, [holidays]|The date a number of working days away.
WORKDAY.INTL|Date & Time|start_date, days, [weekend], [holidays]|The date a number of working days away, with a chosen weekend.
YEAR|Date & Time|serial_number|The year.
YEARFRAC|Date & Time|start_date, end_date, [basis]|The fraction of a year between two dates.
ACCRINT|Financial|issue, first_interest, settlement, rate, par, frequency, [basis], [calc_method]|Accrued interest of a security paying periodic interest.
ACCRINTM|Financial|issue, settlement, rate, par, [basis]|Accrued interest of a security paying at maturity.
COUPDAYBS|Financial|settlement, maturity, frequency, [basis]|Days from the start of the coupon period to settlement.
COUPDAYS|Financial|settlement, maturity, frequency, [basis]|Days in the coupon period containing settlement.
COUPDAYSNC|Financial|settlement, maturity, frequency, [basis]|Days from settlement to the next coupon date.
COUPNCD|Financial|settlement, maturity, frequency, [basis]|The next coupon date after settlement.
COUPNUM|Financial|settlement, maturity, frequency, [basis]|The number of coupons left to maturity.
COUPPCD|Financial|settlement, maturity, frequency, [basis]|The previous coupon date before settlement.
CUMIPMT|Financial|rate, nper, pv, start_period, end_period, type|Interest paid between two periods.
CUMPRINC|Financial|rate, nper, pv, start_period, end_period, type|Principal paid between two periods.
DB|Financial|cost, salvage, life, period, [month]|Depreciation by the fixed-declining-balance method.
DDB|Financial|cost, salvage, life, period, [factor]|Depreciation by the double-declining-balance method.
DISC|Financial|settlement, maturity, pr, redemption, [basis]|The discount rate of a security.
DOLLARDE|Financial|fractional_dollar, fraction|Converts a price written as a fraction to a decimal.
DOLLARFR|Financial|decimal_dollar, fraction|Writes a decimal price as a fraction.
DURATION|Financial|settlement, maturity, coupon, yld, frequency, [basis]|The Macaulay duration of a security.
EFFECT|Financial|nominal_rate, npery|The effective annual interest rate.
FV|Financial|rate, nper, pmt, [pv], [type]|The future value of an investment.
FVSCHEDULE|Financial|principal, schedule|Future value after a series of interest rates.
INTRATE|Financial|settlement, maturity, investment, redemption, [basis]|The interest rate of a fully invested security.
IPMT|Financial|rate, per, nper, pv, [fv], [type]|The interest part of a payment.
IRR|Financial|values, [guess]|The internal rate of return of cash flows.
ISPMT|Financial|rate, per, nper, pv|Interest paid in a period of a straight-line loan.
MDURATION|Financial|settlement, maturity, coupon, yld, frequency, [basis]|The modified duration of a security.
MIRR|Financial|values, finance_rate, reinvest_rate|The modified internal rate of return.
NOMINAL|Financial|effect_rate, npery|The nominal annual interest rate.
NPER|Financial|rate, pmt, pv, [fv], [type]|The number of periods of an investment.
NPV|Financial|rate, value1, [value2], ...|The net present value of cash flows.
PDURATION|Financial|rate, pv, fv|Periods needed for an investment to reach a value.
PMT|Financial|rate, nper, pv, [fv], [type]|The payment of a loan.
PPMT|Financial|rate, per, nper, pv, [fv], [type]|The principal part of a payment.
PRICE|Financial|settlement, maturity, rate, yld, redemption, frequency, [basis]|Price per 100 face value of a security paying periodic interest.
PRICEDISC|Financial|settlement, maturity, discount, redemption, [basis]|Price per 100 face value of a discounted security.
PRICEMAT|Financial|settlement, maturity, issue, rate, yld, [basis]|Price per 100 face value of a security paying at maturity.
PV|Financial|rate, nper, pmt, [fv], [type]|The present value of an investment.
RATE|Financial|nper, pmt, pv, [fv], [type], [guess]|The interest rate per period.
RECEIVED|Financial|settlement, maturity, investment, discount, [basis]|The amount received at maturity.
RRI|Financial|nper, pv, fv|The equivalent interest rate for the growth of an investment.
SLN|Financial|cost, salvage, life|Straight-line depreciation for one period.
SYD|Financial|cost, salvage, life, per|Sum-of-years'-digits depreciation.
TBILLEQ|Financial|settlement, maturity, discount|The bond-equivalent yield of a Treasury bill.
TBILLPRICE|Financial|settlement, maturity, discount|The price per 100 face value of a Treasury bill.
TBILLYIELD|Financial|settlement, maturity, pr|The yield of a Treasury bill.
VDB|Financial|cost, salvage, life, start_period, end_period, [factor], [no_switch]|Declining-balance depreciation over any span of periods.
XIRR|Financial|values, dates, [guess]|The internal rate of return of irregular cash flows.
XNPV|Financial|rate, values, dates|The net present value of irregular cash flows.
YIELD|Financial|settlement, maturity, rate, pr, redemption, frequency, [basis]|The yield of a security paying periodic interest.
YIELDDISC|Financial|settlement, maturity, pr, redemption, [basis]|The annual yield of a discounted security.
YIELDMAT|Financial|settlement, maturity, issue, rate, pr, [basis]|The annual yield of a security paying at maturity.
AND|Logical|logical1, [logical2], ...|TRUE if all arguments are TRUE.
FALSE|Logical||The logical value FALSE.
IF|Logical|logical_test, [value_if_true], [value_if_false]|One value if a condition is TRUE, another if it is FALSE.
IFERROR|Logical|value, value_if_error|A value, or an alternative when it is an error.
IFNA|Logical|value, value_if_na|A value, or an alternative when it is #N/A.
IFS|Logical|logical_test1, value_if_true1, ...|The value for the first condition that is TRUE.
LET|Logical|name1, name_value1, calculation_or_name2, ...|Names intermediate results for use within one formula.
NOT|Logical|logical|Reverses a logical value.
OR|Logical|logical1, [logical2], ...|TRUE if any argument is TRUE.
SWITCH|Logical|expression, value1, result1, [default_or_value2], ...|Compares a value with a list and returns the matching result.
TRUE|Logical||The logical value TRUE.
XOR|Logical|logical1, [logical2], ...|TRUE if an odd number of arguments are TRUE.
ARRAYTOTEXT|Text|array, [format]|An array written as text.
CHAR|Text|number|The character for a code number.
CLEAN|Text|text|Removes non-printing characters.
CODE|Text|text|The code number of the first character.
CONCAT|Text|text1, [text2], ...|Joins text from items and ranges.
CONCATENATE|Text|text1, [text2], ...|Joins text items.
DOLLAR|Text|number, [decimals]|A number as currency text.
EXACT|Text|text1, text2|TRUE if two texts are identical (case-sensitive).
FIND|Text|find_text, within_text, [start_num]|The position of text within text (case-sensitive).
FINDB|Text|find_text, within_text, [start_num]|FIND counting bytes.
FIXED|Text|number, [decimals], [no_commas]|A number rounded and written as text.
LEFT|Text|text, [num_chars]|The first characters of a text.
LEFTB|Text|text, [num_bytes]|LEFT counting bytes.
LEN|Text|text|The number of characters.
LENB|Text|text|The number of bytes.
LOWER|Text|text|Text in lower case.
MID|Text|text, start_num, num_chars|Characters from the middle of a text.
MIDB|Text|text, start_num, num_bytes|MID counting bytes.
NUMBERVALUE|Text|text, [decimal_separator], [group_separator]|Text to a number with given separators.
PROPER|Text|text|Capitalises the first letter of each word.
REPLACE|Text|old_text, start_num, num_chars, new_text|Replaces characters at a position.
REPLACEB|Text|old_text, start_num, num_bytes, new_text|REPLACE counting bytes.
REPT|Text|text, number_times|Repeats text.
RIGHT|Text|text, [num_chars]|The last characters of a text.
RIGHTB|Text|text, [num_bytes]|RIGHT counting bytes.
SEARCH|Text|find_text, within_text, [start_num]|The position of text within text (not case-sensitive, wildcards).
SEARCHB|Text|find_text, within_text, [start_num]|SEARCH counting bytes.
SUBSTITUTE|Text|text, old_text, new_text, [instance_num]|Replaces occurrences of text.
T|Text|value|The text of a value, or empty text.
TEXT|Text|value, format_text|A number formatted as text.
TEXTAFTER|Text|text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found]|The text after a delimiter.
TEXTBEFORE|Text|text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found]|The text before a delimiter.
TEXTJOIN|Text|delimiter, ignore_empty, text1, [text2], ...|Joins text with a delimiter.
TEXTSPLIT|Text|text, col_delimiter, [row_delimiter], [ignore_empty], [match_mode], [pad_with]|Splits text into an array.
TRIM|Text|text|Removes extra spaces.
UNICHAR|Text|number|The Unicode character for a code.
UNICODE|Text|text|The Unicode code of the first character.
UPPER|Text|text|Text in upper case.
VALUE|Text|text|Text that looks like a number, as a number.
VALUETOTEXT|Text|value, [format]|A value written as text.
ADDRESS|Lookup & Reference|row_num, column_num, [abs_num], [a1], [sheet_text]|A cell address as text.
AREAS|Lookup & Reference|reference|The number of areas in a reference.
CHOOSE|Lookup & Reference|index_num, value1, [value2], ...|Picks a value from a list by position.
CHOOSECOLS|Lookup & Reference|array, col_num1, [col_num2], ...|Chosen columns of an array.
CHOOSEROWS|Lookup & Reference|array, row_num1, [row_num2], ...|Chosen rows of an array.
COLUMN|Lookup & Reference|[reference]|The column number of a reference.
COLUMNS|Lookup & Reference|array|The number of columns.
DROP|Lookup & Reference|array, rows, [columns]|An array without its first or last rows/columns.
EXPAND|Lookup & Reference|array, rows, [columns], [pad_with]|An array grown to a size.
FILTER|Lookup & Reference|array, include, [if_empty]|The rows (or columns) that meet a condition.
FORMULATEXT|Lookup & Reference|reference|The formula of a cell as text.
HLOOKUP|Lookup & Reference|lookup_value, table_array, row_index_num, [range_lookup]|Looks across the top row and returns a value below.
HSTACK|Lookup & Reference|array1, [array2], ...|Arrays side by side.
HYPERLINK|Lookup & Reference|link_location, [friendly_name]|A link to a document or place.
INDEX|Lookup & Reference|array, row_num, [column_num], [area_num]|The value at a row and column of a range.
INDIRECT|Lookup & Reference|ref_text, [a1]|The reference named by a text (recalculates).
LOOKUP|Lookup & Reference|lookup_value, lookup_vector, [result_vector]|Looks up a value in one row or column.
MATCH|Lookup & Reference|lookup_value, lookup_array, [match_type]|The position of a value in a range.
OFFSET|Lookup & Reference|reference, rows, cols, [height], [width]|A reference shifted from a starting point (recalculates).
ROW|Lookup & Reference|[reference]|The row number of a reference.
ROWS|Lookup & Reference|array|The number of rows.
SORT|Lookup & Reference|array, [sort_index], [sort_order], [by_col]|An array sorted.
SORTBY|Lookup & Reference|array, by_array1, [sort_order1], ...|An array sorted by other arrays.
TAKE|Lookup & Reference|array, rows, [columns]|The first or last rows/columns of an array.
TOCOL|Lookup & Reference|array, [ignore], [scan_by_column]|An array as one column.
TOROW|Lookup & Reference|array, [ignore], [scan_by_column]|An array as one row.
TRANSPOSE|Lookup & Reference|array|Rows turned into columns.
UNIQUE|Lookup & Reference|array, [by_col], [exactly_once]|The distinct rows of an array.
VLOOKUP|Lookup & Reference|lookup_value, table_array, col_index_num, [range_lookup]|Looks down the first column and returns a value across.
VSTACK|Lookup & Reference|array1, [array2], ...|Arrays one above another.
WRAPCOLS|Lookup & Reference|vector, wrap_count, [pad_with]|A row or column wrapped into columns.
WRAPROWS|Lookup & Reference|vector, wrap_count, [pad_with]|A row or column wrapped into rows.
XLOOKUP|Lookup & Reference|lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode]|Finds a value and returns the matching item.
XMATCH|Lookup & Reference|lookup_value, lookup_array, [match_mode], [search_mode]|The position of a value, with match and search options.
CELL|Information|info_type, [reference]|Information about a cell (formatting, location, contents).
ERROR.TYPE|Information|error_val|A number for an error value.
INFO|Information|type_text|Information about the environment.
ISBLANK|Information|value|TRUE if a cell is empty.
ISERR|Information|value|TRUE for errors other than #N/A.
ISERROR|Information|value|TRUE for any error.
ISEVEN|Information|number|TRUE if a number is even.
ISFORMULA|Information|reference|TRUE if a cell holds a formula.
ISLOGICAL|Information|value|TRUE for a logical value.
ISNA|Information|value|TRUE for #N/A.
ISNONTEXT|Information|value|TRUE for anything but text.
ISNUMBER|Information|value|TRUE for a number.
ISODD|Information|number|TRUE if a number is odd.
ISREF|Information|value|TRUE for a reference.
ISTEXT|Information|value|TRUE for text.
N|Information|value|A value converted to a number.
NA|Information||The error value #N/A.
SHEET|Information|[value]|The sheet number of a reference or sheet name.
SHEETS|Information|[reference]|The number of sheets.
TYPE|Information|value|A number for the type of a value.
DAVERAGE|Database|database, field, criteria|The average of matching database entries.
DCOUNT|Database|database, field, criteria|Counts numbers in matching database entries.
DCOUNTA|Database|database, field, criteria|Counts non-empty matching database entries.
DGET|Database|database, field, criteria|The single entry that matches.
DMAX|Database|database, field, criteria|The largest matching entry.
DMIN|Database|database, field, criteria|The smallest matching entry.
DPRODUCT|Database|database, field, criteria|The product of matching entries.
DSTDEV|Database|database, field, criteria|Sample standard deviation of matching entries.
DSTDEVP|Database|database, field, criteria|Population standard deviation of matching entries.
DSUM|Database|database, field, criteria|The sum of matching entries.
DVAR|Database|database, field, criteria|Sample variance of matching entries.
DVARP|Database|database, field, criteria|Population variance of matching entries.
BESSELI|Engineering|x, n|The modified Bessel function In(x).
BESSELJ|Engineering|x, n|The Bessel function Jn(x).
BESSELK|Engineering|x, n|The modified Bessel function Kn(x).
BESSELY|Engineering|x, n|The Bessel function Yn(x).
BIN2DEC|Engineering|number|Binary to decimal.
BIN2HEX|Engineering|number, [places]|Binary to hexadecimal.
BIN2OCT|Engineering|number, [places]|Binary to octal.
BITAND|Engineering|number1, number2|Bitwise AND.
BITLSHIFT|Engineering|number, shift_amount|Shifts bits left.
BITOR|Engineering|number1, number2|Bitwise OR.
BITRSHIFT|Engineering|number, shift_amount|Shifts bits right.
BITXOR|Engineering|number1, number2|Bitwise exclusive OR.
COMPLEX|Engineering|real_num, i_num, [suffix]|A complex number from real and imaginary parts.
CONVERT|Engineering|number, from_unit, to_unit|Converts between units of measure.
DEC2BIN|Engineering|number, [places]|Decimal to binary.
DEC2HEX|Engineering|number, [places]|Decimal to hexadecimal.
DEC2OCT|Engineering|number, [places]|Decimal to octal.
DELTA|Engineering|number1, [number2]|1 if two numbers are equal, else 0.
ERF|Engineering|lower_limit, [upper_limit]|The error function.
ERF.PRECISE|Engineering|x|The error function from 0 to x.
ERFC|Engineering|x|The complementary error function.
ERFC.PRECISE|Engineering|x|The complementary error function.
GESTEP|Engineering|number, [step]|1 if a number is at least a step value, else 0.
HEX2BIN|Engineering|number, [places]|Hexadecimal to binary.
HEX2DEC|Engineering|number|Hexadecimal to decimal.
HEX2OCT|Engineering|number, [places]|Hexadecimal to octal.
IMABS|Engineering|inumber|The absolute value of a complex number.
IMAGINARY|Engineering|inumber|The imaginary part of a complex number.
IMARGUMENT|Engineering|inumber|The angle of a complex number.
IMCONJUGATE|Engineering|inumber|The complex conjugate.
IMCOS|Engineering|inumber|The cosine of a complex number.
IMDIV|Engineering|inumber1, inumber2|The quotient of two complex numbers.
IMEXP|Engineering|inumber|The exponential of a complex number.
IMLN|Engineering|inumber|The natural logarithm of a complex number.
IMLOG10|Engineering|inumber|The base-10 logarithm of a complex number.
IMLOG2|Engineering|inumber|The base-2 logarithm of a complex number.
IMPOWER|Engineering|inumber, number|A complex number raised to a power.
IMPRODUCT|Engineering|inumber1, [inumber2], ...|The product of complex numbers.
IMREAL|Engineering|inumber|The real part of a complex number.
IMSIN|Engineering|inumber|The sine of a complex number.
IMSQRT|Engineering|inumber|The square root of a complex number.
IMSUB|Engineering|inumber1, inumber2|The difference of two complex numbers.
IMSUM|Engineering|inumber1, [inumber2], ...|The sum of complex numbers.
IMTAN|Engineering|inumber|The tangent of a complex number.
OCT2BIN|Engineering|number, [places]|Octal to binary.
OCT2DEC|Engineering|number|Octal to decimal.
OCT2HEX|Engineering|number, [places]|Octal to hexadecimal.
AVEDEV|Statistical|number1, [number2], ...|The average absolute deviation from the mean.
AVERAGE|Statistical|number1, [number2], ...|The arithmetic mean.
AVERAGEA|Statistical|value1, [value2], ...|The mean, counting text as 0 and logicals as 1/0.
AVERAGEIF|Statistical|range, criteria, [average_range]|The mean of the cells that meet a condition.
AVERAGEIFS|Statistical|average_range, criteria_range1, criteria1, ...|The mean of the cells that meet several conditions.
BETA.DIST|Statistical|x, alpha, beta, cumulative, [A], [B]|The beta distribution.
BETA.INV|Statistical|probability, alpha, beta, [A], [B]|The inverse of the cumulative beta distribution.
BINOM.DIST|Statistical|number_s, trials, probability_s, cumulative|The binomial distribution.
BINOM.INV|Statistical|trials, probability_s, alpha|The smallest count whose cumulative binomial is at least alpha.
CHISQ.DIST|Statistical|x, deg_freedom, cumulative|The chi-squared distribution.
CHISQ.DIST.RT|Statistical|x, deg_freedom|The right-tailed chi-squared probability.
CHISQ.INV|Statistical|probability, deg_freedom|The inverse of the left-tailed chi-squared distribution.
CHISQ.INV.RT|Statistical|probability, deg_freedom|The inverse of the right-tailed chi-squared distribution.
CHISQ.TEST|Statistical|actual_range, expected_range|The chi-squared test of independence.
CONFIDENCE.NORM|Statistical|alpha, standard_dev, size|The confidence interval half-width (normal).
CONFIDENCE.T|Statistical|alpha, standard_dev, size|The confidence interval half-width (Student's t).
CORREL|Statistical|array1, array2|The correlation coefficient.
COUNT|Statistical|value1, [value2], ...|Counts numbers.
COUNTA|Statistical|value1, [value2], ...|Counts non-empty values.
COUNTBLANK|Statistical|range|Counts empty cells.
COUNTIF|Statistical|range, criteria|Counts cells that meet a condition.
COUNTIFS|Statistical|criteria_range1, criteria1, ...|Counts cells that meet several conditions.
COVARIANCE.P|Statistical|array1, array2|Population covariance.
COVARIANCE.S|Statistical|array1, array2|Sample covariance.
DEVSQ|Statistical|number1, [number2], ...|The sum of squared deviations from the mean.
EXPON.DIST|Statistical|x, lambda, cumulative|The exponential distribution.
F.DIST|Statistical|x, deg_freedom1, deg_freedom2, cumulative|The F distribution.
F.DIST.RT|Statistical|x, deg_freedom1, deg_freedom2|The right-tailed F probability.
F.INV|Statistical|probability, deg_freedom1, deg_freedom2|The inverse of the F distribution.
F.INV.RT|Statistical|probability, deg_freedom1, deg_freedom2|The inverse of the right-tailed F distribution.
F.TEST|Statistical|array1, array2|The F-test probability.
FISHER|Statistical|x|The Fisher transformation.
FISHERINV|Statistical|y|The inverse Fisher transformation.
FORECAST.LINEAR|Statistical|x, known_ys, known_xs|A value on the linear trend.
FREQUENCY|Statistical|data_array, bins_array|How often values fall within ranges.
GAMMA|Statistical|x|The gamma function.
GAMMA.DIST|Statistical|x, alpha, beta, cumulative|The gamma distribution.
GAMMA.INV|Statistical|probability, alpha, beta|The inverse of the cumulative gamma distribution.
GAMMALN|Statistical|x|The natural logarithm of the gamma function.
GAMMALN.PRECISE|Statistical|x|The natural logarithm of the gamma function.
GAUSS|Statistical|z|The probability a standard normal value lies between the mean and z.
GEOMEAN|Statistical|number1, [number2], ...|The geometric mean.
GROWTH|Statistical|known_ys, [known_xs], [new_xs], [const]|Values on an exponential trend.
HARMEAN|Statistical|number1, [number2], ...|The harmonic mean.
HYPGEOM.DIST|Statistical|sample_s, number_sample, population_s, number_pop, cumulative|The hypergeometric distribution.
INTERCEPT|Statistical|known_ys, known_xs|The y-intercept of the regression line.
KURT|Statistical|number1, [number2], ...|The kurtosis.
LARGE|Statistical|array, k|The k-th largest value.
LINEST|Statistical|known_ys, [known_xs], [const], [stats]|The statistics of a least-squares line.
LOGEST|Statistical|known_ys, [known_xs], [const], [stats]|The statistics of an exponential curve fit.
LOGNORM.DIST|Statistical|x, mean, standard_dev, cumulative|The lognormal distribution.
LOGNORM.INV|Statistical|probability, mean, standard_dev|The inverse of the lognormal distribution.
MAX|Statistical|number1, [number2], ...|The largest value.
MAXA|Statistical|value1, [value2], ...|The largest value, counting text and logicals.
MAXIFS|Statistical|max_range, criteria_range1, criteria1, ...|The largest value among cells that meet conditions.
MEDIAN|Statistical|number1, [number2], ...|The median.
MIN|Statistical|number1, [number2], ...|The smallest value.
MINA|Statistical|value1, [value2], ...|The smallest value, counting text and logicals.
MINIFS|Statistical|min_range, criteria_range1, criteria1, ...|The smallest value among cells that meet conditions.
MODE.MULT|Statistical|number1, [number2], ...|All the most frequent values.
MODE.SNGL|Statistical|number1, [number2], ...|The most frequent value.
NEGBINOM.DIST|Statistical|number_f, number_s, probability_s, cumulative|The negative binomial distribution.
NORM.DIST|Statistical|x, mean, standard_dev, cumulative|The normal distribution.
NORM.INV|Statistical|probability, mean, standard_dev|The inverse of the normal distribution.
NORM.S.DIST|Statistical|z, cumulative|The standard normal distribution.
NORM.S.INV|Statistical|probability|The inverse of the standard normal distribution.
PEARSON|Statistical|array1, array2|The Pearson correlation coefficient.
PERCENTILE.EXC|Statistical|array, k|The k-th percentile (k strictly between 0 and 1).
PERCENTILE.INC|Statistical|array, k|The k-th percentile (k from 0 to 1).
PERCENTRANK.EXC|Statistical|array, x, [significance]|The rank of a value as an exclusive percentage.
PERCENTRANK.INC|Statistical|array, x, [significance]|The rank of a value as an inclusive percentage.
PERMUT|Statistical|number, number_chosen|The number of permutations.
PERMUTATIONA|Statistical|number, number_chosen|Permutations with repetition.
PHI|Statistical|x|The standard normal density.
POISSON.DIST|Statistical|x, mean, cumulative|The Poisson distribution.
PROB|Statistical|x_range, prob_range, lower_limit, [upper_limit]|The probability that values lie between limits.
QUARTILE.EXC|Statistical|array, quart|A quartile (exclusive method).
QUARTILE.INC|Statistical|array, quart|A quartile (inclusive method).
RANK.AVG|Statistical|number, ref, [order]|The rank of a number; ties share the average rank.
RANK.EQ|Statistical|number, ref, [order]|The rank of a number; ties share the top rank.
RSQ|Statistical|known_ys, known_xs|The square of the correlation coefficient.
SKEW|Statistical|number1, [number2], ...|The skewness.
SLOPE|Statistical|known_ys, known_xs|The slope of the regression line.
SMALL|Statistical|array, k|The k-th smallest value.
STANDARDIZE|Statistical|x, mean, standard_dev|A normalized value.
STDEV.P|Statistical|number1, [number2], ...|Standard deviation of a population.
STDEV.S|Statistical|number1, [number2], ...|Standard deviation of a sample.
STDEVA|Statistical|value1, [value2], ...|Sample standard deviation counting text and logicals.
STDEVPA|Statistical|value1, [value2], ...|Population standard deviation counting text and logicals.
STEYX|Statistical|known_ys, known_xs|The standard error of the predicted y.
T.DIST|Statistical|x, deg_freedom, cumulative|Student's t distribution (left tail).
T.DIST.2T|Statistical|x, deg_freedom|Student's t distribution (two tails).
T.DIST.RT|Statistical|x, deg_freedom|Student's t distribution (right tail).
T.INV|Statistical|probability, deg_freedom|The left-tailed inverse of Student's t.
T.INV.2T|Statistical|probability, deg_freedom|The two-tailed inverse of Student's t.
T.TEST|Statistical|array1, array2, tails, type|The probability from Student's t-test.
TREND|Statistical|known_ys, [known_xs], [new_xs], [const]|Values on a linear trend.
TRIMMEAN|Statistical|array, percent|The mean without the extreme values.
VAR.P|Statistical|number1, [number2], ...|Variance of a population.
VAR.S|Statistical|number1, [number2], ...|Variance of a sample.
VARA|Statistical|value1, [value2], ...|Sample variance counting text and logicals.
VARPA|Statistical|value1, [value2], ...|Population variance counting text and logicals.
WEIBULL.DIST|Statistical|x, alpha, beta, cumulative|The Weibull distribution.
Z.TEST|Statistical|array, x, [sigma]|The one-tailed probability of a z-test.
BETADIST|Compatibility|x, alpha, beta, [A], [B]|The cumulative beta distribution.
BETAINV|Compatibility|probability, alpha, beta, [A], [B]|The inverse of the cumulative beta distribution.
BINOMDIST|Compatibility|number_s, trials, probability_s, cumulative|The binomial distribution.
CHIDIST|Compatibility|x, deg_freedom|The right-tailed chi-squared probability.
CHIINV|Compatibility|probability, deg_freedom|The inverse of the right-tailed chi-squared distribution.
CHITEST|Compatibility|actual_range, expected_range|The chi-squared test of independence.
CONFIDENCE|Compatibility|alpha, standard_dev, size|The confidence interval half-width (normal).
COVAR|Compatibility|array1, array2|Population covariance.
CRITBINOM|Compatibility|trials, probability_s, alpha|The smallest count whose cumulative binomial is at least alpha.
EXPONDIST|Compatibility|x, lambda, cumulative|The exponential distribution.
FDIST|Compatibility|x, deg_freedom1, deg_freedom2|The right-tailed F distribution.
FINV|Compatibility|probability, deg_freedom1, deg_freedom2|The inverse of the right-tailed F distribution.
FORECAST|Compatibility|x, known_ys, known_xs|A value on the linear trend.
FTEST|Compatibility|array1, array2|The F-test probability.
GAMMADIST|Compatibility|x, alpha, beta, cumulative|The gamma distribution.
GAMMAINV|Compatibility|probability, alpha, beta|The inverse of the cumulative gamma distribution.
HYPGEOMDIST|Compatibility|sample_s, number_sample, population_s, number_pop|The hypergeometric distribution.
LOGINV|Compatibility|probability, mean, standard_dev|The inverse of the lognormal distribution.
LOGNORMDIST|Compatibility|x, mean, standard_dev|The cumulative lognormal distribution.
MODE|Compatibility|number1, [number2], ...|The most frequent value.
NEGBINOMDIST|Compatibility|number_f, number_s, probability_s|The negative binomial distribution.
NORMDIST|Compatibility|x, mean, standard_dev, cumulative|The normal distribution.
NORMINV|Compatibility|probability, mean, standard_dev|The inverse of the normal distribution.
NORMSDIST|Compatibility|z|The standard normal distribution.
NORMSINV|Compatibility|probability|The inverse of the standard normal distribution.
PERCENTILE|Compatibility|array, k|The k-th percentile.
PERCENTRANK|Compatibility|array, x, [significance]|The rank of a value as a percentage.
POISSON|Compatibility|x, mean, cumulative|The Poisson distribution.
QUARTILE|Compatibility|array, quart|A quartile.
RANK|Compatibility|number, ref, [order]|The rank of a number.
STDEV|Compatibility|number1, [number2], ...|Standard deviation of a sample.
STDEVP|Compatibility|number1, [number2], ...|Standard deviation of a population.
TDIST|Compatibility|x, deg_freedom, tails|Student's t distribution.
TINV|Compatibility|probability, deg_freedom|The two-tailed inverse of Student's t.
TTEST|Compatibility|array1, array2, tails, type|The probability from Student's t-test.
VAR|Compatibility|number1, [number2], ...|Variance of a sample.
VARP|Compatibility|number1, [number2], ...|Variance of a population.
WEIBULL|Compatibility|x, alpha, beta, cumulative|The Weibull distribution.
ZTEST|Compatibility|array, x, [sigma]|The one-tailed probability of a z-test.
"""

CATEGORIES = ["Financial", "Date & Time", "Math & Trig", "Statistical", "Lookup & Reference", "Database",
              "Text", "Logical", "Information", "Engineering", "Compatibility"]

FUNCTIONS = {}
for _line in _DATA.strip().splitlines():
    _name, _cat, _args, _desc = _line.split("|")
    FUNCTIONS[_name] = (_cat, _args, _desc)


def in_category(cat):
    return sorted(n for n, (c, _, _) in FUNCTIONS.items() if c == cat)


def syntax(name):
    f = FUNCTIONS.get(name)
    if not f:
        return name + "()"
    return "%s(%s)" % (name, f[1])


def describe(name):
    f = FUNCTIONS.get(name)
    return f[2] if f else ""


def arg_names(name):
    f = FUNCTIONS.get(name)
    if not f or not f[1]:
        return []
    return [a.strip() for a in f[1].split(",")]
