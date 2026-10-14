# Where WordCraft falls short of Word

> **Last reviewed:** 2026-10-10 · **Last updated:** 2026-10-14 · **Change:** minor (Gap #7 & #8: Language dialog & multilingual proofing scoping landed; Gap #10: Shape Connectors; Gap #5: Move tracking & content controls; interactive picture crop; advanced typography) · **Target:** Microsoft Word (Microsoft 365) for Mac 16.113.4

The work list. One entry per shortfall, ranked by how much it stops a Word user from switching
(**B** = blocks beta). Each says what's missing, the evidence, who it hurts, an Opus 5.5 agent
wall-clock estimate, and which parity document tracks it. Agents: pick from the top, check open PRs
first (`gh pr list`), and update this file and the parity doc when a gap closes.

## Ranked

| # | Gap | B | Evidence | Impact | Hours | Doc |
|---|---|---|---|---|---|---|
| 1 | **DOCX never tested on real-world files.** No corpus of files from Word, Google Docs, LibreOffice and Pages has been opened, rendered and round-tripped and compared with Word page by page | B | No corpus in the repo; `crates/docx/tests` are synthetic fixtures; "DOCX fidelity corpus: not started" since 2026-10-06 | Everyone: the files people receive are the files that break | 30–50 | [file-format-parity.md](file-format-parity.md) |
| 2 | **Ink and drawing canvases are dropped on open and lost on save, silently**; OLE objects show only their picture | B | No ink or drawing-canvas (`wpc`) handling in `crates/docx`; group shapes (`wpg`) read and written (#267). Charts and SmartArt are drawn (#292) and, with OLE objects, written back with all their parts on save (#319) | Documents with diagrams drawn in Word or pen notes lose content without warning | 4–8 to preserve ink and drawing canvases; 45–70 to render and edit charts and SmartArt (#13) | [file-format-parity.md](file-format-parity.md) |
| 3 | **Crashes and freezes in the field**: startup crash on Intel UHD Graphics (#170) (lag on GPUs without a DirectX 12 driver fixed, #316), freezes (#77, #59 Thai fonts, #31 font selector), no writes without admin on Windows 11 (#218), broken shape tool (#142), images can't be moved (#82) | B | 62 open issues, ~15 of them crashes, hangs or data-path failures | Anyone hit loses work or trust; the never-crash standard covers panics, not hangs or GPU driver faults | 20–30 | [target-app-parity.md](target-app-parity.md) (Stability) |
| 4 | **Pagination differs from Word**: Aptos has no metric-matched substitute; columns don't balance; custom footnote continuation separators and notices aren't read; legacy compatibility options ignored; no page-by-page comparison harness | B | [layout-parity.md](layout-parity.md); [typing-parity.md](typing-parity.md) Known gaps | Page counts and line breaks differ, which matters for forms, legal and academic work | 30–50 + owner (font) | [layout-parity.md](layout-parity.md) |
| 5 | **Move tracking and content controls**: landed (2026-10-14): `w:moveFrom`/`w:moveTo` round-trip for text runs and paragraph marks with full author/date attribution; inline content controls (`w:sdt`) round-trip with `w:id`, `w:tag`, `w:alias`, `w:lock`, and inner `w:sdtPr` extensions | | Issue #41; `crates/docx/tests/roundtrip.rs` | Legal and editorial review workflows; templates with form controls | 0 | [file-format-parity.md](file-format-parity.md) |
| 6 | **Print dashboard**: landed (2026-10-13): File › Print features live printer discovery, copies, duplex, collation, color/grayscale, page ranges (all, current, custom), and scaling through `file.printNative` | | `crates/ui-egui/src/backstage.rs`, `crates/engine/src/cmd/file.rs` | Everyone who prints | 0 | [hardware-parity.md](hardware-parity.md) |
| 7 | **Dialog depth**: 28 modal dialogs vs Word's ~100 (Insert Caption, Line Numbers, Hyphenation, and Set Proofing Language landed 2026-10-14; Define Multilevel List, Track Changes Options, Advanced Font, Table Properties, Cell Margins…) | B | `Dialog` enum in `crates/ui-egui/src/dialogs.rs` | Power users reach for dialogs the ribbon alone doesn't cover | 12–16 | [ui-parity.md](ui-parity.md) |
| 8 | **Proofing per-run language attribution & multi-language scoping**: landed (2026-10-14): Set Proofing Language dialog (`Dialog::Language`), status bar interactive language indicator, `review.language` run tagging with `noProof` and Normal style defaults, and language-scoped proofing in `crates/proof` and `crates/layout` preventing false squiggles on non-English runs | | `crates/proof`, `crates/layout/src/para.rs`, `crates/ui-egui/src/dialogs.rs` | Every non-English writer | 15–25 | [localization-parity.md](localization-parity.md) |
| 9 | **Password-protected files** can't be opened or saved | | #55; `docbin` detects and refuses RC4/XOR | Corporate and legal users | 6–10 | [file-format-parity.md](file-format-parity.md) |
| 10 | **Objects**: no group (#267 open), rotated text-box text (free rotation and flips landed, #332), contour (tight/through) wrap, shape effects, WordArt, connectors (Elbow and Curved connectors landed 2026-10-14, #12.2), Drawing Canvas, Icons | | [parity-checklist.md](parity-checklist.md) (Shape Format 2/6, Layout › Group) | Newsletters, flyers, reports with diagrams | 15–25 | [target-app-parity.md](target-app-parity.md) |
| 11 | **Interface languages**: 8 of the 12 key languages missing (hi, ar, fr, id, de, ko, vi, pt-PT); no mirrored RTL interface; ~5% of strings still English in the six translated catalogs | | [localization-parity.md](localization-parity.md) | Most of the world's writers | 50–80 | [localization-parity.md](localization-parity.md) |
| 12 | **Draw tab / ink**: pens, pencil, highlighter, eraser, Select and Hide Ink landed (#307); missing: Lasso Select, Add Pen, Ink to Shape, Ink to Math, Ink Replay, pressure, Word's own ink (`w14:contentPart`/InkML) | | [parity-checklist.md](parity-checklist.md) (Draw 5/11) | Pen and tablet users | 8–14 | [hardware-parity.md](hardware-parity.md) |
| 13 | **Charts and SmartArt editing** (own renderer, chart data editing, SmartArt layouts) | | `insert.chart`, `insert.smartArt` missing | Reports and business documents | 45–70 | [target-app-parity.md](target-app-parity.md) |
| 14 | **`.doc` can't be written; metafile pictures and Word 6/95 files refused** | | `crates/docbin` is read-only | Users exchanging files with old Word or legacy systems | 25–40 | [file-format-parity.md](file-format-parity.md) |
| 15 | **References depth**: bibliography sources round-trip in DOCX customXml/item1.xml (#169 closed); no EndNote/Mendeley desktop integration | | Sources round-trip in DOCX | Students and researchers | 3–5 | [target-app-parity.md](target-app-parity.md) |
| 16 | **Right-to-left completeness**: RTL sections (RTL tables landed 2026-10-12), kashida, RTL in HTML/ODT/RTF; eight older RTL bug reports to re-verify after #207 (#215, #211, #199, #66, #63, #48, #19) | | [layout-parity.md](layout-parity.md) | Arabic, Persian, Hebrew writers | 10–15 | [layout-parity.md](layout-parity.md) |
| 17 | **East Asian typography**: vertical text, ruby/Phonetic Guide, Enclose Characters, document grid, Asian Typography options | | Catalog misses `format.phonetic`, `format.enclose`, `para.asianTypography` | Chinese, Japanese and Korean documents | 20–30 | [layout-parity.md](layout-parity.md) |
| 18 | **Screen-reader access** to the document canvas untested | | AccessKit enabled; no tests | Blind and low-vision users; public-sector procurement | 10–15 | [ui-parity.md](ui-parity.md) |
| 19 | **RTF and ODT depth**: notes, comments, revisions, sections, RTL | | ~1,500 lines each in `crates/formats` | Users exchanging with LibreOffice or older tools | 18–27 | [file-format-parity.md](file-format-parity.md) |
| 20 | **Performance on large real-world documents** unmeasured (500+ pages, many pictures) | | Only the 188-page sample benchmark | Thesis and book authors | 10–20 | [layout-parity.md](layout-parity.md) |
| 21 | **Mail merge data sources**: no Excel, Outlook/Contacts, email merge; CSV and typed lists only (#247) | | | Office administrators | 6–10 | [target-app-parity.md](target-app-parity.md) |
| 22 | **Ecosystem**: no VBA execution (macros preserved only), no Office add-ins, no cloud storage, no real-time co-authoring | | `.docm` keeps macros (#172) | Enterprise users with macro templates; teams | 40–80 + owner decisions | [target-app-parity.md](target-app-parity.md) |
| 23 | **PDF Reflow** (open a PDF as an editable document, Word for Windows) | | Landed (2026-10-14): ISO 32000-1 clean-room parsing, Flate stream decompression, Adobe ToUnicode CMap decoding, positioned operator extraction, line/paragraph reflow, heading/list detection, and page breaks | Users who edit PDFs they receive | 0 | [file-format-parity.md](file-format-parity.md) |
| 24 | **Customize Ribbon / Keyboard**, Quick Access Toolbar customization; ~100 of Word's ~250 shortcuts | | [ui-parity.md](ui-parity.md) | Power users | 10–16 | [ui-parity.md](ui-parity.md) |
| 25 | **In-app AI assistant** (Copilot-style draft, rewrite, summarize) | | Chat pane with slash commands, settings and non-blocking provider calls, and an agent that edits the document through commands in rounds with one Undo, landed (2026-10-12); still missing: a preview/diff before the agent applies, provider calls in the web build | Users expecting Copilot | 15–30 + owner (provider) | [target-app-parity.md](target-app-parity.md) |
| 26 | **Minor formats**: Flat OPC XML, Word 2003 XML, MHT, Works/WordPerfect | | | Rare | 20–35 | [file-format-parity.md](file-format-parity.md) |
| 27 | **View windows**: landed (2026-10-13): New Window, Arrange All, Split, View Side by Side, Synchronous Scrolling, and Switch Windows on the View ribbon tab with full keytips and vector icons | | [parity-checklist.md](parity-checklist.md) (View 29/29) | Comparing documents | 0 | [ui-parity.md](ui-parity.md) |
| 28 | **Dictate** | | `tools.dictate` missing | Dictation users | 8–15 + owner (speech model) | [hardware-parity.md](hardware-parity.md) |
| 29 | **Equations**: ink equations | | #191 notes; long display equations wrap at operators (#326) | Maths-heavy documents | 2–3 | [layout-parity.md](layout-parity.md) |

**Beta needs #1–#5, #7** (~145–222 h with the stability and layout work they imply): they are the
blocking gaps, and closing them lifts file formats, layout, stability and UI enough to take the
full number from ~60% to ~75%. Everything else is depth on the way to full parity.

## By kind

The same gaps grouped the way the parity documents are, for agents working in one area.

- **Feature gaps:** #2 (preserve), #5, #10, #12, #13, #15, #17, #21, #25, #27, #28, #29; Clipboard
  pane, Manage Styles, Group (open PRs exist for the first three — review and merge before
  re-implementing). Paste Special, Advanced Find, Column Selection, Style Inspector, custom table
  styles, and Draw Table and Eraser (#303) landed on 2026-10-10.
- **UI/UX gaps:** #7, #18, #24; crop handles on the canvas landed 2026-10-14 (#11.1); table column drag on the
  ruler landed 2026-10-12 (#49, #217) and tables have a context menu; context menus for objects; Linux title bar theming (#78).
- **File-format gaps:** #1, #2, #5, #9, #14, #19, #23, #26; embedded fonts; glossary/building blocks.
- **Hardware gaps:** #12, #28; scanner/Continuity Camera insert; KDE soft text (#140). (Print dashboard landed, #6).
- **Localization gaps:** #8, #11, #16, #17; native-speaker review of every catalog (human).
- **Stability and performance gaps:** #3, #20.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-14 | minor | #23: PDF Reflow and editable import landed in `crates/formats/src/pdf_import.rs` with ISO 32000-1 parsing, Flate stream decompression, Adobe ToUnicode CMap decoding, positioned operator extraction, line/paragraph reflow, heading/list detection, and page breaks |
| 2026-10-14 | minor | #7 & #8: Set Proofing Language dialog (`Dialog::Language`), status bar interactive language indicator, and language-scoped proofing in `crates/proof` & layout |
| 2026-10-14 | minor | #10: Shape Connectors (Elbow and Curved) landed with DrawingML bentConnector3/curvedConnector3 round-trip, vello_cpu & krilla PDF vector rendering, and ribbon Insert Shapes integration |
| 2026-10-14 | minor | #5: Move tracking (`w:moveFrom`/`w:moveTo`) and content controls (`w:sdt`) round-trip in DOCX; interactive canvas picture crop tool; Font dialog Advanced typography tab landed |
| 2026-10-13 | trivial | #15: DOCX Bibliography Sources persistence in customXml/item1.xml landed (#169 closed) |
| 2026-10-13 | trivial | #27: View ribbon tab Window controls (Side by Side, Synchronous Scrolling, Switch Windows) with dedicated vector icons |
| 2026-10-13 | minor | #6: full Backstage Print dashboard in File › Print with live printer discovery, copies, duplex, collation, color/grayscale, page ranges, and scaling |
| 2026-10-11 | trivial | All 12 bibliography styles landed (#383); removed from References depth |
| 2026-10-11 | trivial | Formatting revisions landed (#41); gap #5 narrowed to move tracking and content controls, 8–12 h |
| 2026-10-10 | trivial | Draw Table and Eraser landed (#303); removed from the feature-gap list |
| 2026-10-10 | trivial | Tabs and Borders and Shading dialogs landed (#320); removed from the dialog-depth list |
| 2026-10-10 | trivial | Columns, Symbol and Field dialogs landed (#321); removed from the dialog-depth list |
| 2026-10-10 | trivial | #2 narrowed: charts and SmartArt drawn (#292), and with OLE objects kept on save (#319); group shapes landed (#267); ink and drawing canvases remain |
| 2026-10-10 | trivial | #4: long footnotes continue onto the next page (#352) |
| 2026-10-10 | trivial | Define New Multilevel List and Track Changes Options dialogs landed (#328); removed from the dialog-depth list |
| 2026-10-10 | trivial | #10: free rotation and flips for pictures, shapes and text boxes, with a rotation handle (#332); text boxes rotate their frame; their text stays upright |
| 2026-10-10 | trivial | #29: long display equations wrap across lines (#326) |
| 2026-10-10 | trivial | #3: Windows lag on GPUs without a DirectX 12 driver fixed with an OpenGL fallback (#316) |
| 2026-10-10 | trivial | #6: desktop File › Print opens the system PDF viewer (#286) |
| 2026-10-10 | trivial | Table Properties dialog landed (#44); removed from the dialog-depth list |
| 2026-10-10 | trivial | Merged main: landed features removed from the feature-gap list |
| 2026-10-10 | minor | Beta note restated: full number back to the additive ~60% (method aligned with the standard, no new evidence) |
| 2026-10-10 | major | First version, from the full re-measure: 29 ranked gaps with evidence, impact and hours |
| 2026-10-10 | trivial | #307: Draw tab pens, eraser and Hide Ink landed; ink gap narrowed |
