# WordCraft roadmap

**Stage: alpha** · next: beta, ~15 points (60% → 75% ready for real work) and ~150–230 h away

> **Last reviewed:** 2026-10-11 · **Last updated:** 2026-10-14 · **Change:** WordArt warp presets (`format.textWarp`) on text runs, on screen and in PDF; previously: Copilot agent edits the document; .doc converts to .docx on open, .doc table rows end correctly; previously: Copilot chat with settings and a non-blocking provider call, in-cell selection shading; previously: right-to-left tables, three-key date sort, ruler column markers, nested tables fit their cell; previously: pasting over selected cells fills each one; more table fixes; previously: table fixes (a Split Cells crash, the calendar's web clock, merged-cell geometry); previously: merged upstream/main with fork features (Bangla/Bijoy input & decoding, AI Copilot suite, EPUB export, Android cdylib, Windows Inno Setup installer, readability auditor, PDF/A, contour tight wrap) · **Target:** Microsoft Word (Microsoft 365) for Mac 16.113.4

WordCraft aims for complete parity with Microsoft Word, then goes further on speed, openness and
agent control. This page is the summary; the assessment is
[`docs/target-app-parity.md`](docs/target-app-parity.md), the work list is
[`docs/gaps.md`](docs/gaps.md), and milestones and the current focus are in
[`docs/roadmap.md`](docs/roadmap.md).

## Headline numbers (2026-10-11)

| Number | Value | Kind |
|---|---|---|
| Ribbon/menu catalog coverage | **100%** | measured (`cargo xtask parity` → [`docs/parity-checklist.md`](docs/parity-checklist.md)) |
| **Feature breadth** (weighted, incl. dialog options, styles, languages beyond the ribbon) | **~80%** | estimated |
| **Ready for real work** (full target) | **~60%** (55–63%) | estimated, additive weighted sum over the dimensions |
| **Mainstream practitioner** | **~55%** | estimated: weekly-work depth × discounts for interaction (×0.93), stability (×0.90), file exchange (×0.90) |
| **Essentials user** | **~63%** | estimated: core-feature depth × discounts for launch (×0.90), discoverability (×0.95), opening received files (×0.92) |
| Remaining to **beta** | **~150–230 h** of Opus 5.5 agent wall-clock (gaps #1–#7) | estimated |
| Remaining to **full parity** with Word desktop | **~750–1,250 h** (70–80% parallelizable) | estimated |
| Commands / tests / code | 450+ commands · 855+ tests + 10 property blocks · ~95,000 lines of Rust | measured (source) |
| Releases | v0.6.0 (2026-10-11): Windows x64 Inno Setup installer & portable zip, Android native cdylib, macOS DMG, Linux AppImage/deb/rpm/Flatpak, FreeBSD, web | measured |

Hours are calibrated from this repo's PRs (equations #191: ~9.7k lines in ~5 h; `.doc` reader
#145: ~4.5k lines in ~10 h; layout fidelity PRs run 5–10× slower per line). The earlier "≈120–150 h
to 100%" (2026-10-06) left out localization, multilingual proofing, ecosystem, hardware, PDF Reflow
and `.doc` writing, and priced the DOCX corpus at 10 h; see
[methodology](docs/target-app-parity.md#methodology). The three readiness numbers, their weights,
discounts and the user evidence (120 issues from outside users, 4 praise, 0 "switched from Word",
~21 open core-path bugs) are in
[readiness numbers](docs/target-app-parity.md#readiness-numbers-full-mainstream-practitioner-essentials-user).

| Audience | Ready | Opus 5.5 agent wall-clock hours to ~95% | Work that dominates |
|---|---|---|---|
| Full target (ready for real work) | **~60%** | **~650–1,100 h** (70–80% parallelizes) | Breadth: charts/SmartArt/Draw, localization and proofing languages, East Asian and RTL typography, `.doc` writing, PDF Reflow, ecosystem; plus everything below |
| Mainstream practitioner | **~55%** | **~300–500 h** (~65% parallelizes) | DOCX real-world fidelity and preserving charts/SmartArt, pagination fidelity, formatting revisions, dialog depth, objects, stability |
| Essentials user | **~63%** | **~90–140 h** (~50% parallelizes) | Install/launch stability, opening files people send (corpus subset, chart fallbacks), native printing, picture handling, English spelling depth |

## By dimension

| Dimension | Parity | Hours to full | Doc |
|---|---|---|---|
| Features (depth) | 68% | 210–335 | [target-app-parity.md](docs/target-app-parity.md#feature-areas) |
| UI/UX fidelity | 65% | 65–100 | [ui-parity.md](docs/ui-parity.md) · [typing-parity.md](docs/typing-parity.md) |
| File formats | 60% | 185–300 | [file-format-parity.md](docs/file-format-parity.md) |
| Layout and pagination fidelity | 55% | 90–145 | [layout-parity.md](docs/layout-parity.md) |
| Hardware | 40% | 20–40 | [hardware-parity.md](docs/hardware-parity.md) |
| Localization | 25% | 80–130 + native review | [localization-parity.md](docs/localization-parity.md) |
| Performance | 75% | 10–20 | [layout-parity.md](docs/layout-parity.md#performance) |
| Stability | 50% | 25–45 | [gaps.md](docs/gaps.md) (#3) |
| Platforms | 95% (beyond Word on Linux, BSD, Android, web) | 5–10 | this page |
| Ecosystem and plugins | 15% | 40–80 + owner decisions | [gaps.md](docs/gaps.md) (#22) |
| AI features | 75% (native Copilot suite + external MCP/CLI agents) | 15–30 | [gaps.md](docs/gaps.md) (#25) |

## Features

| Area | Parity | Hours |
|---|---|---|
| Typing, selection, clipboard, undo, find/replace | 85% | 8–12 |
| Character and paragraph formatting | 82% | 8–12 |
| Lists | 78% | 5–8 |
| Equations | 75% | 4–8 |
| Tables | 76% | 10–16 |
| Styles and themes | 74% | 8–12 |
| Review (comments, track changes, compare, protect) | 72% | 15–25 |
| View | 72% | 6–10 |
| Page layout and sections | 70% | 12–20 |
| Mailings | 70% | 6–10 |
| Headers, footers, page numbers, fields | 70% | 8–12 |
| References (TOC, citations, bibliography, captions, index) | 65% | 12–20 |
| Footnotes and endnotes | 60% | 5–8 |
| Pictures, shapes, text boxes, WordArt | 60% | 20–30 |
| Backstage, printing, options | 58% | 12–20 |
| Right-to-left and complex scripts (incl. Bangla Phonetic/Jatiya/Bijoy) | 65% | 15–25 |
| Proofing (spelling, grammar, thesaurus, readability auditor) | 45% | 25–45 |
| Accessibility | 35% | 15–25 |
| East Asian typography | 15% | 20–30 |
| Charts, SmartArt, Draw/ink, 3D models, icons | 25% | 70–110 |
| Agent control & Copilot AI (CLI, MCP, control channel, in-app Copilot) | beyond Word | — |

Weights and evidence per area: [docs/target-app-parity.md](docs/target-app-parity.md#feature-areas).

## Languages

| Language | Code | Status | UI strings |
|---|---|---|---|
| English | en | full | 100% |
| Bangla / Bengali | bn | native input engine (Phonetic, Jatiya, Bijoy/SutonnyMJ) | — |
| Simplified Chinese | zh-hans | partial | ~95% |
| Spanish | es | partial | ~95% |
| Hindi | hi | none | 0% |
| Arabic | ar | none (document shaping and bidi work; no mirrored UI) | 0% |
| French | fr | none | 0% |
| Portuguese | pt-br (pt-PT none) | partial | ~95% |
| Indonesian | id | none | 0% |
| Japanese | ja | partial | ~95% |
| German | de | none (PR #203 open) | 0% |
| Korean | ko | none | 0% |
| Vietnamese | vi | none | 0% |

Also shipped: Traditional Chinese (`zh-hant`), Ukrainian (`uk`) and Serbian (`sr`, `sr-latn`, #250), all partial (~95%). Estonian (`et`) now has a 1,161-entry interface catalog with system-locale
selection and a saved language choice; Estonian proofing is not included. Word ships
30 interface languages and proofing for ~50. Detail: [docs/localization-parity.md](docs/localization-parity.md).

## Progress log

| 2026-10-14 | WordArt slice 3: warp presets (`format.textWarp`), Word's Transform: eleven DrawingML presets (arch up and down, circle, chevron and inverted chevron, wave, double wave, inflate, deflate, slant up and down), each at an amount from 0 to 1 of its full bend. Screen and PDF bend the run's glyph outlines within the run's own bounds, and the shadow, gradient fill and outline follow the bent glyphs. Arch and circle bend the baseline along a parabola, so glyphs stay upright rather than turning along a circle. Layout is unchanged, so a warp can overlap the lines around it; the run keeps its line box. A warped run is vector paths in PDF, so its text is not extractable. DOCX does not save the warp yet: Word keeps it on the WordArt object (`a:prstTxWarp` in the shape's `a:bodyPr`), which WordCraft's WordArt does not model, so the warp is dropped on save. Still open: the WordArt gallery, the Shape Format tab, picture and texture text fill, and WordArt objects
| 2026-10-14 | WordArt slice 2: gradient text fill (`format.textFill`, two to eight stops, any angle). Screen and PDF paint the run with a linear gradient across the whole run, and the PDF text stays extractable. DOCX `w14:textFill` with `w14:gradFill`, in the schema order MS-DOCX gives for text effects (shadow, outline, fill). Still open: warp, picture and texture text fill, the WordArt gallery and Shape Format tab |
| 2026-10-14 | WordArt slice 1: Text Outline and Text Shadow on text runs (`format.textOutline`, `format.textShadow`). Drawn on screen and in PDF as paths, so the text stays extractable; round-tripped as `w14:textOutline` and `w14:shadow` in DOCX. Blur filters are not used: `vello_cpu` filters panic under multi-threaded rendering, which the native renderer uses. Warp, gradient and picture text fill, and the WordArt gallery and Shape Format tab, are still open |
| 2026-10-13 | Space before at the top of a page like Word after page breaks (32 cases measured in Word for Windows, compatibility modes 12 and 15): a paragraph ending with a page break keeps its mark with it and the next paragraph gets no space before; a paragraph starting with a page break starts the new page with its space before; a page break before keeps the space before up to mode 14 and drops it in mode 15; the previous paragraph's space after comes off after a page break before or a section break |
| 2026-10-13 | Native printing goes through the system print spooler in its own crate (`wordcraft-print`, ported from craft-libs): CUPS `lp` on macOS and Linux with copies, collation, duplex, colour and the printer's own options; the Windows spooler through PowerShell, with each sheet at its physical size. File › Print (Native) takes a `printer` and `copies` (a PDF `path` also prints); no print dialog of our own yet |
| 2026-10-13 | `.doc` tables compared with Word's PDF of the same form: tables whose cells all state a width keep their widths (no re-sharing to fit content), so the text column keeps its width. 12 → 9 pages; Word has 7. The rest is the font: Word sets the Bangla text in SutonnyMJ, which is not installed on the machine that renders here, so the fallback font's taller lines fill more pages |
| 2026-10-12 | `.doc` tables: rows with their own grids (Word lays each row out on its own) map onto one table grid made from every row's boundaries, with spans; the converted `.docx` now has each row's cells on the right columns. Page layout still differs from Word (12 pages vs 7): the layout engine's autofit squeezes a 551pt table into a 540pt text width |
| 2026-10-12 | `.doc` tables: the column grid is chosen by the table's column count (rows of one table carry different grids; the first one won, so tables were 40% wider than the page); an empty end-of-row mark closes its row without adding a cell; cells without their own width take the grid's columns. Checked against a real appraisal form: columns, cell text and grid widths now match the original. Page layout still differs from Word (12 pages vs 7) |
| 2026-10-12 | Copilot agent: messages (or `/edit`) change the document itself. The model works in rounds through WordCraft's own commands — an allowlist of editing, formatting, style, table and layout commands, never files, the clipboard, dialogs or macros — reading a compact outline of the document and each command's result, and one Undo reverts a whole run; it stops if the user edits meanwhile; an Agent toggle and settings (most rounds) |
| 2026-10-12 | `.doc`: opening a Word 97–2003 file converts it to a `.docx` beside the original (never overwriting a file; File › Info › Convert; an Options switch); table rows whose properties Word keeps in the Data stream (`sprmPHugePapx`) now end where they should — a real 7-page appraisal form had its rows run into one 46-cell row and laid out on 19 pages (12 now) |
| 2026-10-12 | Copilot: the pane is a chat (slash commands `/check-doc`, `/proofread`, `/summarize`, `/rewrite`, `/draft`, `/table`, `/ask`; Insert, Replace and Copy on each answer) with a settings page behind a gear (provider, key, model, creativity, longest answer, wait, answer language, tone, standing instructions, document scope and size, conversation memory); provider calls run off the UI thread, so a slow model no longer freezes the window; Ask goes to the model when a key is set instead of the offline keyword match; Rewrite/Summarize/Proofread work on the selection or document with the box text as instructions. Tables: a text selection inside a cell is shaded as text, not as the whole cell |
| 2026-10-12 | Tables: right-to-left tables (`w:bidiVisual`, Table Properties direction); Sort by up to three keys with a date type and only the selected rows; column markers on the ruler; nested tables fit their cell; selections take vertically merged cells whole; Update Field settles chains of formulas; importers keep every cell's text past the column limit and show orphaned merged cells; tests pin floating-table overlap, live AutoFit, clicks into merged/nested/split cells and typing speed in a 1000-row table |
| 2026-10-12 | Tables: pasting text over selected cells fills every selected cell, as Word does (it filled only the first); sort puts accented letters with their base letter; header rows from Table Properties stay a block at the top; a paste over a merged cell keeps every copied cell's text; Distribute Rows reads text-sized rows from the layout; widening a column against its neighbour keeps the table width; clipboard text keeps one line per row; table commands no longer walk the whole story |
| 2026-10-12 | Table fixes: Split Cells no longer panics on a row wider than its grid; the Quick Tables calendar reads the web-safe clock; Delete Columns takes every column of a merged cell; Merge and Select Cell Range work in grid columns across rows with merged cells; Split Table, the .docx/.doc readers and Delete Rows keep vertical merges valid; a one-side table border keeps the cells' other borders; Delete Cells › Shift Up takes bottom cells that end their row together |
| 2026-10-11 | Merged upstream capabilities with fork features: Native Bangla input engine (Phonetic, Jatiya, ANSI/Bijoy) & automatic SutonnyMJ/Bijoy DOCX decoding; AI Document Intelligence Suite (`ai.rewrite`, `ai.generateTable`, `ai.summarize`, `ai.ask`, `ai.autocomplete`); EPUB export; readability auditor; PDF/A compliance; geometric contour tight wrapping; Windows Inno Setup installer & Android native `cdylib` |
| 2026-10-11 | Citations and bibliography in all 12 of Word's styles: Harvard, ISO 690 (author-date and numerical), Turabian, GB/T 7714, GOST (name and title sort) and SIST02 join APA, MLA, Chicago and IEEE; numbered styles count in order of first citation (#383) |
| 2026-10-11 | Interface size setting (#475): File › Options › General scales the interface 80–200 % on top of the system display scale, kept between runs; `ui.interfaceSize` for agents |
| 2026-10-11 | Formatting revisions (#41): `w:rPrChange`/`w:pPrChange`, table/row/cell, section and numbering changes kept in DOCX; formatting recorded as revisions while tracking; accept/reject, Reviewing Pane descriptions, "Formatted: …" balloons and change bars |
| 2026-10-10 | Estonian (`et`) interface (#294): 1,161 translated entries, locale and saved-preference tests, bundled-font checks; no Estonian proofing resources |
| 2026-10-10 | Draw tab: pen, pencil and highlighter ink, stroke eraser, Select, Review › Hide Ink; ink saved to DOCX as DrawingML freeforms (#307) |
| 2026-10-10 | Draw Table and Eraser (#303): a pen draws one-cell tables and splits cells along drawn lines; the eraser merges the cells beside a border |
| 2026-10-10 | DOCX charts, SmartArt and OLE objects survive open and save with their parts (#319) |
| 2026-10-10 | Equations with OMML and an Equation tab (#191); Word 97-2003 `.doc` import (#145); right-to-left and Persian text (#207); Zotero integration (#189); Read Aloud player (#190); password-protected `.docx` (#281); WMF/EMF metafiles (#293); group objects & shape effects (#267, #280); Asian typography kinsoku (#271); clipboard pane (#264); v0.4.0 released |
| 2026-10-09 | Interface languages follow the system or Options (#12; zh-hans, zh-hant, ja); TOC page numbers as an updatable field (#52); rotating log file (#17); window geometry remembered (#38) |
| 2026-10-08 | v0.2.0 and v0.3.0 released; Flatpak and AppImage auto-update (#22) |
| 2026-10-07 | v0.1.0 released; contributor credits in About (#6) |
