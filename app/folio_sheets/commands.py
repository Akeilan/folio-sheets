"""Every command the UI can issue, and whether it is implemented.

A command id maps to a window method name (`act_<id>` by convention) or to
None while it is still missing. The click-everything audit (tools/audit.py)
walks the ribbon, menus and Backstage and reports every control whose
command is missing here or raises when run.
"""

# id: label. Implemented commands have an act_<id with - replaced by _>
# method on SheetWindow; the audit checks that.
LABELS = {
    # file / QAT
    "new": "New", "open": "Open", "save": "Save", "save-as": "Save As", "close": "Close",
    "print": "Print", "export-pdf": "Export to PDF", "options": "Options",
    "undo": "Undo", "redo": "Redo",
    # clipboard
    "paste": "Paste", "paste-values": "Paste Values", "paste-special": "Paste Special",
    "paste-formulas": "Paste Formulas", "paste-formatting": "Paste Formatting",
    "paste-transpose": "Paste Transpose", "paste-link": "Paste Link", "paste-picture": "Paste as Picture",
    "paste-keep-widths": "Keep Source Column Widths", "paste-no-borders": "No Borders",
    "cut": "Cut", "copy": "Copy", "copy-picture": "Copy as Picture",
    "format-painter": "Format Painter", "clipboard-pane": "Clipboard",
    # font
    "font-name": "Font", "font-size": "Font Size", "font-grow": "Increase Font Size",
    "font-shrink": "Decrease Font Size", "bold": "Bold", "italic": "Italic",
    "underline": "Underline", "double-underline": "Double Underline", "strikethrough": "Strikethrough",
    "borders": "Borders", "border-bottom": "Bottom Border", "border-top": "Top Border",
    "border-left": "Left Border", "border-right": "Right Border", "border-none": "No Border",
    "border-all": "All Borders", "border-outside": "Outside Borders",
    "border-thick-outside": "Thick Outside Borders", "border-bottom-double": "Bottom Double Border",
    "border-bottom-thick": "Thick Bottom Border", "border-top-bottom": "Top and Bottom Border",
    "border-top-thick-bottom": "Top and Thick Bottom Border",
    "border-top-double-bottom": "Top and Double Bottom Border",
    "border-draw": "Draw Border", "border-draw-grid": "Draw Border Grid", "border-erase": "Erase Border",
    "border-line-color": "Line Color", "border-line-style": "Line Style", "border-more": "More Borders",
    "fill-color": "Fill Color", "font-color": "Font Color", "font-dialog": "Format Cells: Font",
    # alignment
    "align-top": "Top Align", "align-middle": "Middle Align", "align-bottom": "Bottom Align",
    "orientation": "Orientation", "wrap-text": "Wrap Text", "align-left": "Align Left",
    "align-center": "Center", "align-right": "Align Right", "indent-decrease": "Decrease Indent",
    "indent-increase": "Increase Indent", "merge-center": "Merge & Center", "merge-across": "Merge Across",
    "merge-cells": "Merge Cells", "unmerge": "Unmerge Cells", "alignment-dialog": "Format Cells: Alignment",
    # number
    "number-format": "Number Format", "accounting": "Accounting Number Format", "percent": "Percent Style",
    "comma": "Comma Style", "decimal-increase": "Increase Decimal", "decimal-decrease": "Decrease Decimal",
    "number-dialog": "Format Cells: Number",
    # styles
    "conditional-formatting": "Conditional Formatting", "format-as-table": "Format as Table",
    "cell-styles": "Cell Styles",
    # cells
    "insert-cells": "Insert Cells", "insert-rows": "Insert Sheet Rows", "insert-columns": "Insert Sheet Columns",
    "insert-sheet": "Insert Sheet", "delete-cells": "Delete Cells", "delete-rows": "Delete Sheet Rows",
    "delete-columns": "Delete Sheet Columns", "delete-sheet": "Delete Sheet",
    "row-height": "Row Height", "autofit-rows": "AutoFit Row Height", "column-width": "Column Width",
    "autofit-columns": "AutoFit Column Width", "default-width": "Default Width",
    "hide-rows": "Hide Rows", "hide-columns": "Hide Columns", "hide-sheet": "Hide Sheet",
    "unhide-rows": "Unhide Rows", "unhide-columns": "Unhide Columns", "unhide-sheet": "Unhide Sheet",
    "rename-sheet": "Rename Sheet", "move-copy-sheet": "Move or Copy Sheet", "tab-color": "Tab Color",
    "protect-sheet": "Protect Sheet", "lock-cell": "Lock Cell", "format-cells": "Format Cells",
    # editing
    "autosum": "AutoSum", "autosum-average": "Average", "autosum-count": "Count Numbers",
    "autosum-max": "Max", "autosum-min": "Min", "fill-down": "Fill Down", "fill-right": "Fill Right",
    "fill-up": "Fill Up", "fill-left": "Fill Left", "fill-series": "Series", "fill-justify": "Justify",
    "flash-fill": "Flash Fill", "clear-all": "Clear All", "clear-formats": "Clear Formats",
    "clear-contents": "Clear Contents", "clear-comments": "Clear Comments and Notes",
    "clear-hyperlinks": "Clear Hyperlinks", "remove-hyperlinks": "Remove Hyperlinks", "open-hyperlink": "Open Hyperlink", "sort-asc": "Sort A to Z", "sort-desc": "Sort Z to A",
    "sort-custom": "Custom Sort", "filter": "Filter", "filter-clear": "Clear Filter",
    "filter-reapply": "Reapply", "find": "Find", "replace": "Replace", "goto": "Go To",
    "goto-special": "Go To Special", "select-formulas": "Formulas", "select-comments": "Notes",
    "select-cf": "Conditional Formatting", "select-constants": "Constants",
    "select-validation": "Data Validation", "select-objects": "Select Objects", "selection-pane": "Selection Pane",
    # insert
    "pivot-table": "PivotTable", "recommended-pivots": "Recommended PivotTables", "table": "Table",
    "pictures": "Pictures", "shapes": "Shapes", "smartart": "SmartArt", "screenshot": "Screenshot",
    "recommended-charts": "Recommended Charts", "chart-column": "Insert Column or Bar Chart",
    "chart-line": "Insert Line or Area Chart", "chart-pie": "Insert Pie or Doughnut Chart",
    "chart-hierarchy": "Insert Hierarchy Chart", "chart-statistic": "Insert Statistic Chart",
    "chart-combo": "Insert Combo Chart", "chart-scatter": "Insert Scatter or Bubble Chart",
    "chart-waterfall": "Insert Waterfall, Funnel, Stock, Surface or Radar Chart",
    "pivot-chart": "PivotChart", "charts-dialog": "Insert Chart",
    "chart-add-element": "Add Chart Element", "chart-colors": "Change Colors", "chart-switch-row-col": "Switch Row/Column",
    "chart-select-data": "Select Data", "chart-change-type": "Change Chart Type", "chart-format": "Format Chart",
    "chart-delete": "Delete Chart",
    "table-convert": "Convert to Range", "table-resize": "Resize Table", "table-option": "Table Style Options",
    "table-style": "Table Style",
    "edit-note": "Edit Note", "delete-note": "Delete Note", "show-hide-note": "Show/Hide Note",
    "next-note": "Next Note", "prev-note": "Previous Note", "convert-notes": "Convert to Comments", "sparkline-line": "Line Sparkline",
    "sparkline-column": "Column Sparkline", "sparkline-winloss": "Win/Loss Sparkline",
    "slicer": "Slicer", "timeline": "Timeline", "link": "Link", "comment": "New Comment",
    "text-box": "Text Box", "header-footer": "Header & Footer", "wordart": "WordArt",
    "signature-line": "Signature Line", "object": "Object", "equation": "Equation", "symbol": "Symbol",
    # draw
    "draw-select": "Select", "draw-lasso": "Lasso Select", "draw-eraser": "Eraser", "draw-pen": "Pen",
    "draw-pencil": "Pencil", "draw-highlighter": "Highlighter", "draw-ruler": "Ruler",
    "ink-to-shape": "Ink to Shape", "ink-to-math": "Ink to Math", "ink-replay": "Ink Replay",
    # page layout
    "themes": "Themes", "theme-colors": "Colors", "theme-fonts": "Fonts", "theme-effects": "Effects",
    "margins": "Margins", "page-orientation": "Orientation", "page-size": "Size", "print-area": "Print Area",
    "breaks": "Breaks", "background": "Background", "print-titles": "Print Titles",
    "page-setup-dialog": "Page Setup", "scale-width": "Width", "scale-height": "Height", "scale": "Scale",
    "view-gridlines": "View Gridlines", "print-gridlines": "Print Gridlines",
    "view-headings": "View Headings", "print-headings": "Print Headings",
    "bring-forward": "Bring Forward", "send-backward": "Send Backward", "align-objects": "Align",
    "group-objects": "Group", "rotate-objects": "Rotate",
    # formulas
    "insert-function": "Insert Function", "fn-recent": "Recently Used", "fn-financial": "Financial",
    "fn-logical": "Logical", "fn-text": "Text", "fn-datetime": "Date & Time", "fn-lookup": "Lookup & Reference",
    "fn-math": "Math & Trig", "fn-more": "More Functions", "name-manager": "Name Manager",
    "define-name": "Define Name", "use-in-formula": "Use in Formula", "names-from-selection": "Create from Selection",
    "trace-precedents": "Trace Precedents", "trace-dependents": "Trace Dependents", "remove-arrows": "Remove Arrows",
    "show-formulas": "Show Formulas", "error-checking": "Error Checking", "evaluate-formula": "Evaluate Formula",
    "watch-window": "Watch Window", "calc-options": "Calculation Options", "calc-now": "Calculate Now",
    "calc-sheet": "Calculate Sheet", "calc-full": "Calculate Full", "fn-insert": "Insert Function",
    "autosum": "AutoSum", "autosum-average": "Average", "autosum-count": "Count Numbers", "autosum-max": "Max",
    "autosum-min": "Min",
    # data
    "get-data": "Get Data", "from-text-csv": "From Text/CSV", "existing-connections": "Existing Connections",
    "refresh-all": "Refresh All", "queries-connections": "Queries & Connections",
    "workbook-links": "Workbook Links", "advanced-filter": "Advanced", "text-to-columns": "Text to Columns",
    "remove-duplicates": "Remove Duplicates", "data-validation": "Data Validation", "pick-from-list": "Pick From Drop-down List", "consolidate": "Consolidate",
    "what-if": "What-If Analysis", "scenario-manager": "Scenario Manager", "goal-seek": "Goal Seek",
    "data-table": "Data Table", "forecast-sheet": "Forecast Sheet", "group": "Group", "ungroup": "Ungroup",
    "subtotal": "Subtotal", "show-detail": "Show Detail", "hide-detail": "Hide Detail", "solver": "Solver",
    # review
    "spelling": "Spelling", "thesaurus": "Thesaurus", "workbook-statistics": "Workbook Statistics",
    "check-accessibility": "Check Accessibility", "new-comment": "New Comment", "delete-comment": "Delete",
    "prev-comment": "Previous Comment", "next-comment": "Next Comment", "show-comments": "Show Comments",
    "notes": "Notes", "new-note": "New Note", "show-all-notes": "Show All Notes",
    "protect-workbook": "Protect Workbook", "allow-edit-ranges": "Allow Edit Ranges", "hide-ink": "Hide Ink",
    # view
    "view-normal": "Normal", "view-page-break": "Page Break Preview", "view-page-layout": "Page Layout",
    "custom-views": "Custom Views", "view-ruler": "Ruler", "view-formula-bar": "Formula Bar",
    "zoom-dialog": "Zoom", "zoom-100": "100%", "zoom-selection": "Zoom to Selection",
    "new-window": "New Window", "arrange-all": "Arrange All", "freeze-panes": "Freeze Panes",
    "freeze-top-row": "Freeze Top Row", "freeze-first-column": "Freeze First Column",
    "unfreeze": "Unfreeze Panes", "split": "Split", "hide-window": "Hide", "unhide-window": "Unhide",
    "side-by-side": "View Side by Side", "sync-scroll": "Synchronous Scrolling",
    "reset-window": "Reset Window Position", "switch-windows": "Switch Windows", "macros": "Macros",
    # help
    "help": "Help",
    # status bar / misc
    "select-all": "Select All", "context-menu": "Context Menu",
}


def method_name(cmd):
    base = cmd.split(":", 1)[0]
    return "act_" + base.replace("-", "_")


def label(cmd):
    return LABELS.get(cmd.split(":", 1)[0], cmd)
