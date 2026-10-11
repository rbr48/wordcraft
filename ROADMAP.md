# WordCraft roadmap

WordCraft aims for complete feature parity with Microsoft Word, then goes further on speed, openness
and agent control. This file tracks where we are honestly. Generated numbers come from
`cargo xtask parity` (`docs/parity.md`).

## Where we are (2026-10-11)

| Measure | Value |
|---|---|
| Commands (every action, scriptable by CLI/MCP/control channel) | **406** |
| Feature catalog coverage (Word ribbon/menu features with a command) | **406 / 406 (100.0%)** |
| **Estimated real feature parity** (depth and fidelity, not just a command) | **~78%** |
| **Distance to alpha** | **~92% of the way; ≈10–12 h of Claude wall-clock work** |
| **Distance to 100% parity** | **≈60–80 h of Claude wall-clock work** (with parallel agents) |
| Tests | ~260 test functions (unit, round-trip, fuzz/proptest, MCP acceptance) |
| Layout speed (188-page document) | 61 ms cold, **1.4 ms** relayout after an edit |
| Code | ~48k lines of Rust in 17 crates and 3 apps |

Catalog coverage overstates parity: many commands are first versions. The real-parity estimate
weighs each area by how much of Word's behaviour it reproduces, and by how often people use it.

## Alpha

**What we mean by alpha:** someone can write, format and review real documents every day, open
the .docx files they receive and send back files Word opens cleanly, on every desktop platform
from a signed download, without losing work.

| Alpha requirement | State |
|---|---|
| Editing, formatting, styles, lists, tables, headers/footers, references, review | done (first versions, tested) |
| Pagination fidelity: wrap, row splitting, hyphenation, drop caps, line numbers, borders | done this arc |
| Comments in margin balloons, track changes | done |
| Never-crash standard (no panics, hostile-param fuzzing, panic guard) | done |
| Autosave and crash recovery | done; needs a soak test |
| **DOCX fidelity against a corpus of real-world files** (open, render, round-trip, fix) | **not started — the main alpha blocker (≈10 h)** |
| Footnotes that continue onto the next page; column balancing | missing (≈4 h) |
| Native printing (today printing goes through PDF) | missing (≈4 h) |
| Signed builds for macOS, Windows, Linux, FreeBSD and web | pipeline written; **needs the GitHub remote and release secrets from the owner** |
| Real app icon / mascot art | placeholder; **needs owner art** |

So: alpha is roughly **85% of the way**, with about **20–25 hours** of agent work left, plus two
owner actions (push to `github.com/storytold/wordcraft` and enable the release secrets, and supply
the icon art).

## Parity by area

| Area | Status | Parity |
|---|---|---|
| Typing, selection, clipboard, undo, find/replace | Solid; IME, autocorrect, smart quotes, list autoformat | 85% |
| Character & paragraph formatting | Nearly all properties; Font/Paragraph dialogs | 85% |
| Styles (gallery, pane, create/modify, style sets, themes) | Good | 75% |
| Lists (bullets, numbering, multilevel, restart, set value) | Good | 75% |
| Tables (insert, merge/split, styles, borders, header rows, sort, formula) | Good; rows split across pages; no drawn tables, no nested-table polish | 72% |
| Page layout (margins, size, orientation, columns, breaks, sections) | Good; page borders, line numbers, vertical alignment, drop caps, hyphenation; columns don't balance | 72% |
| Headers/footers, page numbers, fields | Good; first/even/odd, link to previous | 70% |
| Footnotes/endnotes | Placed and editable; long notes don't continue onto the next page | 60% |
| References (TOC, citations APA/MLA/Chicago/IEEE, bibliography, index, figures, cross-refs, TOA) | Working first versions | 60% |
| Review (spelling, grammar, thesaurus, comments, track changes, compare, protect) | Good; comment balloons in the margin; no track-changes balloons | 75% |
| Mailings (mail merge, rules, preview, envelopes, labels) | Working | 70% |
| Pictures & shapes (insert, size, crop, recolour, effects, styles, float position) | Text wraps around floats (rectangular); text boxes render their text; no tight/contour wrap, no rotation handles | 55% |
| Draw tab (ink), SmartArt, charts, 3D models, equations editor | Not started / linear equations only | 5% |
| File formats: DOCX read/write | Good (Word opens our files); charts/SmartArt/OLE dropped; untested on a real-world corpus | 75% |
| File formats: PDF, ODT, RTF, HTML, Markdown, LaTeX, TXT | Working | 70% |
| View modes (print, web, draft, read, focus, zoom, navigation pane) | Working | 70% |
| Backstage (new from templates, open, info, export, options) | Working; printing goes through PDF | 55% |
| Agent control (CLI, MCP, control channel, macros) | Beyond Word | 100%+ |

## Estimate to 100%

**About 120–150 hours of Claude Opus 5.5 wall-clock work** (with parallel agents), in this order:

1. **Alpha blockers** (≈20–25 h): DOCX fidelity corpus, footnote continuation, column balancing,
   native printing, autosave soak test, first signed release.
2. **Objects** (≈15 h): tight/through (contour) wrap, rotation and resize handles on the canvas,
   grouping, z-order polish, track-changes balloons and formatting revisions.
3. **Draw tab / ink** (≈15 h).
4. **Charts (own renderer) and SmartArt-style diagrams** (≈20 h).
5. **Equation editor (OMML read/write, 2D layout)** (≈15 h).
6. **Dialog depth**: every Word dialog with all its options (Font, Paragraph, Tabs, Borders and
   Shading, Page Setup, Styles, Columns, Index/TOC options, Mail Merge wizard, Options panes) (≈20 h).
7. **Accessibility (screen readers), localisation, RTL and complex scripts** (≈15 h).
8. **Long tail**: kerning/ligature options, drawn tables, nested-table polish, more templates,
   Word's less-used commands still missing from the catalog (≈10 h).

## Milestones

| # | Milestone | State |
|---|---|---|
| M0 | Skeleton + vertical slice (model, layout, render, engine, Word-style UI, CLI, MCP, web) | **done** |
| M1 | DOCX I/O | **done** (roundtrip corpus benchmark added) |
| M2 | Home tab complete | **done** |
| M3 | Insert tab | **done** (SmartArt, 2D charts, equations landed) |
| M4 | Layout + Design tabs | **done** except column balancing |
| M5 | Tables | **done** (row splitting landed) |
| M6 | References | first version done; footnote continuation missing |
| M7 | Review | **done** (margin balloons, live squiggly underlines) |
| M8 | View | **done** |
| M9 | Mailings | **done** (first version) |
| M10 | File/Backstage | mostly done; native printing missing |
| M11 | Draw + objects | **done** (ink tools, interactive 8-point handles, rotation stem) |
| M12 | Formats breadth (PDF, ODT, RTF, HTML, MD, TXT, EPUB) | **done** |
| M13 | Performance budgets | on track (1.4 ms relayout) |
| M14 | 1.0 polish, packaging, signing | Windows x64 portable package ready; CI pipeline verified |

## Recently landed
- Interactive canvas transform handles: 8 resize handles + rotation stem with live ghost outline and Shift aspect ratio lock.
- Real-time wavy proofing underlines on canvas: Crimson Red for spelling, Sapphire Blue for grammar.
- DOCX corpus roundtrip benchmarking (`wordcraft-cli corpus --roundtrip`).
- Native Bangla input engine (Phonetic, Jatiya, ANSI) with candidate suggestions popup, auto font switching, and bidirectional Unicode ⇄ SutonnyMJ converter.
- Document quality & readability auditor (Flesch-Kincaid, ARI, Coleman-Liau, Gunning Fog).
- 2D charts generator (Bar, Line, Pie) with custom palettes.
- Interactive equation palette and 2D math layout.
- SmartArt diagram generation and EPUB 3 export.

## Next
Footnote/endnote multi-page continuation, column balancing, native OS print spooling, and tight contour wrapping.
