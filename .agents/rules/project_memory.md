---
trigger: always_on
description: Comprehensive project memory, architecture guide, and operational reference for WordCraft.
---

# WordCraft — Agent Memory & Instant Project Reference

## Overview
- **Repository**: WordCraft (`rbr48/wordcraft`, forked from `storytold/wordcraft`).
- **Purpose**: Clean-room, open-source Rust word processor targeting 100% Microsoft Word parity + high speed + agent control.
- **Platforms**: Windows x64, macOS, Linux, FreeBSD, WebAssembly (`wasm32-unknown-unknown`).
- **Toolchain on Windows**: MinGW GCC & Rust 2021:
  `$env:PATH = "C:\Users\USER\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.WinGet.Source_8wekyb3d8bbwe\mingw64\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"`

## Non-Negotiable Rules
1. **Never Crash**: Non-test code forbids `unsafe` (`#![forbid(unsafe_code)]`), rejects `unwrap()`, `expect()`, `panic!`, `unimplemented!`, `todo!`, `unreachable!`. Any hostile input is clamped/bounds-checked.
2. **Zero Proprietary Assets**: Never copy icons, fonts, or assets from Microsoft/Adobe. All UI icons are procedurally drawn vector code in `crates/ui-egui/src/icons.rs`. All third-party fonts/assets are documented in `ATTRIBUTION.md`.
3. **Strict Layering**: Downward-only dependencies across 17 crates:
   - **L0 Foundation**: `geom` (units, rects), `control-key` (auth token)
   - **L1 Core**: `doc` (AST, styles, undo), `fonts` (HarfBuzz, substitution), `proof` (spelling/grammar)
   - **L2 Document & Layout**: `layout` (pagination, lines, hit-test), `docx` (OOXML), `formats` (MD, HTML, RTF, EPUB)
   - **L3 Rasterization & Export**: `render` (vello_cpu), `pdf` (krilla)
   - **L4 Engine**: `engine` (Session, CommandRegistry, catalog parity, Bangla input engine, re-exports `wordcraft_proof as proof`)
   - **L5 Agents**: `mcp` (Model Context Protocol)
   - **L6 UI**: `ui-egui` (ribbon, canvas, handles, squiggly underlines)
   - **Apps**: `apps/wordcraft`, `apps/wordcraft-cli`, `apps/wordcraft-web`, `xtask`
4. **Everything is a Command**: Every action dispatches through `Session::run(cmd_id, params)`. Programmatic invocations never open modal dialogs.
5. **Quality Gates**: Every commit must pass all 6 CI gates: `cargo run -p xtask -- ci` (`fmt`, `clippy -D warnings`, `test`, `assets`, `layers`, `wasm`).

## Current State & Capabilities
- **Feature Parity**: 406 / 406 Word catalog features mapped to live commands (100.0% catalog parity in `docs/parity.md`).
- **Advanced Features Added**:
  - Full vector Draw tab (ballpoint, highlighter, pencil, eraser, lasso).
  - SmartArt diagrams (Process, Cycle, Hierarchy, Relationship, Matrix, Pyramid) & 2D charts (Bar, Line, Pie).
  - 2D Math layout & Equation palette.
  - Native Bangla input engine (Phonetic/Avro, Jatiya, ANSI/SutonnyMJ) with real-time suggestion popup & auto font-switching.
  - Document quality & readability auditor (Flesch-Kincaid, ARI, Coleman-Liau, Gunning Fog).
  - Interactive canvas transform handles (8 resize handles + rotate stalk handle with Shift aspect lock and live ghost preview).
  - Real-time wavy squiggly underlines on canvas (Crimson Red for spelling, Sapphire Blue for grammar).
  - DOCX corpus roundtrip benchmarking via `wordcraft-cli corpus --roundtrip`.
  - Geometric contour tight text wrapping for non-rectangular floating shapes (Ellipses, Diamonds, Triangles).
  - Portable Windows x64 release package at `dist/wordcraft/wordcraft-0.1.0-windows-x64-portable.zip`.
  - Lead Developer & Top Contributor attribution configured: M S Rana (`@rbr48` / Izhaan Intellect).
  - Discord links removed across chrome title bar, Help ribbon, backstage navigation, and About window.
  - Interactive Track Changes revision markup balloons in side margin with `[✓ Accept]` and `[✗ Reject]` buttons.
  - Full DOCX Watermark roundtrip fidelity (read and write via ECMA-376 / ISO 29500 VML header shapes).
  - Native AI Document Intelligence Suite: `ai.rewrite` (5 tone presets), `ai.generateTable` (natural language prompt to native tables), `ai.summarize` (bullets, executive, abstract), `ai.ask` (document structure, telemetry, queries), `ai.autocomplete` (ghost suggestions), and dedicated Copilot pane with "Table" & "Ask" tabs.
  - OpenType typographic features (`liga`, `kern`, `smcp`) in layout text shaping.
  - PDF/A archival compliance & metadata tagging in `wordcraft-pdf`.
  - Native OS printing command (`file.printNative`) and Windows Inno Setup installer script (`packaging/windows/wordcraft.iss`).
  - Full Bijoy 52 conjunct parity (যুক্তবর্ণ), 'g' linker key, independent vowels, and Avro phonetic grapheme cluster boundary handling.
  - Complete Dari (দাঁড়ি / '।'), Bisarga (': ' / 'ঃ'), and middle-of-word independent vowels ('আ', 'ই', etc.) precision support across Jatiya, Phonetic (Avro), and ANSI (Bijoy Classic/SutonnyMJ).
  - Fixed ANSI typing `Shift + g` (`G`) outputting Bangla Dari (`।`, U+0964) instead of English period (`.`) or `জ্ঞ`.
  - Rebuilt Windows x64 Inno Setup installer package at `packaging/windows/Output/wordcraft-0.6.0-windows-x64-setup.exe`.
  - Rebuilt portable release archive at `dist/wordcraft/wordcraft-0.6.0-windows-x64-portable.zip`.
  - Isolated English proofing (spelling and grammar squiggles) from Bengali text: runs formatted in SutonnyMJ, Kalpurush, or any Bengali font (`is_bengali_family`), or tagged with Bengali language (`bn`), or containing Bengali Unicode / SutonnyMJ glyphs, now skip English proofing without generating false-positive red/blue squiggly lines.
  - Fixed SutonnyMJ conjunct mapping for `ল্গ` (`find_bijoy_conjunct('ল', 'গ')` corrected from `ë` [ল্ট] to `î` [ল্গ]), added missing Ra-fala conjuncts (`ক্র`, `ত্র`, `প্র`, `গ্র`, `ব্র`, `ভ্র`, `শ্র`, `স্র`, `ধ্র`, `ম্র`, `হ্র`, `হৃ`), and added Bijoy `z` (Ra-fala) key ligature replacement in ANSI mode.
  - Implemented dynamic Medial E-kar (`‡`, `\u{2021}`) vs. Word-Initial E-kar (`†`, `\u{2020}`) selection in SutonnyMJ ANSI typing (`c` / `X`) and `unicode_to_ansi` converter: eliminates the white gap before medial Pre-kar E-kar/O-kar in words like `ভালোবাসি` (`fv‡jvevwm`) while retaining word-initial spacing in words like `তোমায়` (`†Zvgvq`).
  - Added support for 2-character conjunct clusters (`cÖ`, `¯Í`, etc.) receiving preceding pre-kar (`†cÖg` for `প্রেম`, `wcÖwZ` for `প্রীতি`).
  - Rebuilt Windows x64 Inno Setup installer package at `packaging/windows/Output/wordcraft-0.6.0-windows-x64-setup.exe` and portable release archive at `dist/wordcraft/wordcraft-0.6.0-windows-x64-portable.zip`.
  - Fixed Jatiya (Unicode) mode pre-kar (`c` [ে], `d` [ি], `C` [ৈ]) ordering: removed erroneous `prev_is_cons` check that was attaching pre-kars to preceding consonants instead of the upcoming consonant, fixing multi-syllable typing like `বসে` (was `বেস`), `করে` (was `কের`), `পড়ে` (was `পের`).
  - Fixed Jatiya mode independent vowel linker handling: when typing independent vowels via `g` + vowel key (e.g. `c` + `j` [`কে`] then `g` + `s` [`উ`]), stashed pre-kar (`ে`) is restored back to the preceding consonant (`কে`), correctly yielding `কেউ` instead of leaking into `কউে`.
  - Replaced small static dictionary with a comprehensive Avro phonetic vocabulary database (`crates/engine/src/cmd/bangla_vocab.rs`) containing 660+ high-frequency Bengali words and inflections across verbs, agriculture, banking, careers, and grammar.
  - Prioritized exact dictionary matches as Candidate #1 in `phonetic_candidates` and added direct vocabulary lookup in `phonetic_word`: words like `krishi` (`কৃষি`), `banke` (`ব্যাংকে`), `chakri` (`চাকরি`), and `kori` (`করি`) now transliterate and auto-commit accurately (e.g., `ami bangladesh krishi banke chakri kori` -> `আমি বাংলাদেশ কৃষি ব্যাংকে চাকরি করি`).
  - Added length-based prefix candidate prioritization so common, shorter words appear first in candidate suggestion popups (e.g., `am` suggests `আমি`, `আমার`, `আমরা` first).
  - Fixed Dari (`।`) in Avro Phonetic mode: mapped `|` (Shift + `\`) to insert `।` (was previously bugged inserting `ঃ` [Bisarga]), enabled `..` (double period) automatic conversion into `।`, and allowed non-digit period `.` to insert `।`.
  - Conducted deep table audit (`docs/table-audit.md`) covering commands, data model, layout, OOXML round-trips, ribbon UI, and context dialogs across 19 critical-to-low findings (A–S).
  - Unblocked multi-cell selection for mutating table commands: `crates/engine/src/cmd/column.rs` now allows all `table.*` commands to retain column block selections across cells rather than prematurely collapsing to a single caret.
  - Implemented grid-based multi-cell and column selection: added `select_grid_range` and updated `select_column_cells` so column selection uniformly selects grid column ranges even across complex horizontally merged rows.
  - Fixed merged cell column resizing (`boundary_g = g.saturating_add(span).saturating_sub(1)`) in `crates/engine/src/cmd/table.rs`.
  - Decoupled vertical cell alignment (`table.cellAlign`) to set `cl.props.valign` (`top`, `vcenter`, `bottom`) directly on cells without clobbering paragraph text alignment.
  - Fixed table resize hit testing in `crates/ui-egui/src/table_resize.rs` to skip `VMerge::Continue` cells on row borders.
  - Implemented dedicated `Dialog::CellMargins` dialog with top, left, bottom, right inputs and wired it to the Table Layout ribbon button.
  - Verified all 6 CI quality gates (`fmt`, `clippy -D warnings`, `test`, `assets`, `layers`, `wasm`) pass cleanly.
  - **Full Table Audit Resolution (commits b0fe7c8, 458a4ff, sort/formula commit, 947d78e)**:
    - **B** — Added `Table::repair_vmerge()` in `crates/doc/src/table.rs`: scans grid columns, promotes orphan `Continue` cells to `None`, demotes single-row `Restart` to `None`; called at end of `insert_row`, `delete_row`, `insert_col`, `delete_col`, `merge`.
    - **C** — `Table::merge` (vertical) now trims the leading empty paragraph from the restart cell after moving blocks in.
    - **D** — `Table::split_cell` no longer calls `insert_col` (which widened the table). When `span >= n` it re-spans in place; when `span < n` it subdivides the last grid column in-width, preserving `total_width`.
    - **E** — (noted limitation) `table.toText`/`table.fromText` preserve plain text only; documented in audit.
    - **F** — `formula` command: unknown functions now return `Err(CmdError::Failed("unknown formula function `X`"))`. `MAX`/`MIN` empty-set returns 0; `PRODUCT` empty-set returns 1. `parse_num` strips `৳`, `¥`, `%` in addition to `$`, `€`, `£`.
    - **G/S** — `sort_table` uses a strict 3-tier total-order comparator: (1) both numeric via `f64::total_cmp`, (2) numeric before text, (3) case-insensitive lexicographic. Eliminates non-total-order panic risk.
    - **H** — Backspace nested-table navigation in `crates/engine/src/cmd/text.rs`: path comparison changed from `q.0.first() == prev_path.0.first()` to `q.0.starts_with(&prev_path.0)` so caret stays in the parent cell, not the outer table.
    - **K** — `table.repeatHeader` now enforces contiguous headers from row 0: toggling ON marks rows 0..=r as header; toggling OFF clears headers from r onward.
    - **M** — `del_col` collects all distinct grid columns from `selected_cells`, deletes them in descending order; falls back to caret column when no multi-cell selection.
    - **O** — `table.distributeRows` now averages total row height (`total / count`) instead of taking the max height.
    - **S** — `split_table` has a bounds guard (`if r <= t.rows.len()`) preventing panic on out-of-range row index.
    - Regression tests added for: `vmerge_repair_removes_orphan_continues`, `split_cell_does_not_widen_table`, `distribute_rows_averages_height`, `table_sort_total_order_and_currency`, `table_formula_unknown_function_errors`, `del_col_multi_column_selection`, `backspace_after_nested_table_scoped_to_parent_cell`.
  - **Version 0.6.2 Upgrade & Release**:
    - Bumped workspace version to `0.6.2` in `Cargo.toml`.
    - Updated Windows Inno Setup script (`packaging/windows/wordcraft.iss`), `build-installer.ps1`, `packaging/android/AndroidManifest.xml`, and `packaging/android/build-apk.ps1`.
    - Recompiled optimized release binaries `wordcraft.exe` and `wordcraft-cli.exe`.
    - Rebuilt Windows x64 Inno Setup installer at `packaging/windows/Output/wordcraft-0.6.2-windows-x64-setup.exe`.
    - Rebuilt portable release archive at `dist/wordcraft/wordcraft-0.6.2-windows-x64-portable.zip`.
  - **Upstream Feature Integration Sprint (10 commits merged & verified)**:
    - **#626**: DOCX theme colors and theme fonts take precedence over fixed fallback values.
    - **#543, #553**: Find and Replace special characters (`^p`, `^t`, `^^`) and wildcard pattern compilation with capture replacements.
    - **#40, #796**: Custom dictionary persistence across sessions (`user_dictionary.txt` atomic bounded file I/O).
    - **#544, #545**: AutoCorrect TWo INitial CApitals, cAPS LOCK accidental correction, and table-cell first-letter capitalization.
    - **#745**: Large inline picture layout Word parity (pictures keep their dimensions rather than shrinking, imported/pasted pictures fitted to text width).
    - **#810, #829**: ODT import reads page width, height, print orientation, and margins from `styles.xml` with defensive bounds clamping.
    - **#713**: Bidi auto-detection for RTL paragraph direction when typing Arabic, Kurdish, or Hebrew into an empty paragraph.
    - **#759**: Resizable Navigation pane truncates long headings with ellipses and provides hover tooltips.
    - **#907**: Hardened table formula engine per upstream maintainer review (table boundary clamping, 100k cell evaluation budget, power/sign recursion depth guards).
    - Cleanly integrated upstream fork PRs #1 and #2 without regressions to custom Bangla/AI capabilities.
  - **Full Document Agent & Advanced .doc Tables (PRs #4 and #5)**:
    - **M9.1–M9.3**: Full document AI agent in `crates/engine/src/agent.rs` running multi-round goal execution, compact AST outline representation, command execution with defensive guards, and single-stroke atomic undo.
    - **Copilot Chat Pane**: Dedicated non-blocking Copilot UI (`crates/ui-egui/src/copilot.rs`) running provider requests off the main UI thread via `std::sync::mpsc`, slash command palette (`/edit`, `/check-doc`, `/proofread`, `/summarize`, `/rewrite`, `/draft`, `/table`, `/ask`), and model presets.
    - **M3.4–M3.6**: `.doc` files automatically convert to `.docx` on save/edit; `.doc` binary tables support per-row column grids, proper row-end marks, and explicit cell width retention matching Microsoft Word behavior.
  - **Version 0.6.3 Upgrade & Native Spooler Printing (PR #9)**:
    - **M10**: Native OS spooler printing integrated via new crate `wordcraft-print` (Windows Print Spooler, macOS CUPS, Linux CUPS, with PDF intermediate rendering and PowerShell script driver).
    - Bumped workspace version to `0.6.3` in `Cargo.toml`.
    - Updated Windows Inno Setup script (`packaging/windows/wordcraft.iss`), `build-installer.ps1`, `packaging/android/AndroidManifest.xml`, and `packaging/android/build-apk.ps1`.
  - **Upstream UI Polish & Bug Fixes Import (Commits up to 41d37e6)**:
    - Safely imported 18 upstream polish and bug fix commits while preserving 100% of custom features (Full Document AI Agent, Copilot pane, Bangla typing engine & 77k vocabulary, table audit repairs):
      1. Mini toolbar layout metrics and proper box-aligned dropdown menus (`#535`).
      2. Space before suppression at the top of a page matching Word behavior (`#932`).
      3. Multiple line spacing leading added below text runs (`3731898`).
      4. Adjacent paragraph spacing collapses to larger of before/after margins (`be74745`).
      5. Strict Word-parity kerning application (`681a57f`).
      6. Ribbon tab Keytips matching Word for Windows: File (F), Design (G), References (S), Review (R), View (W), Help (Y) (`#922`).
      7. Drawing view position preserved at ink point rather than jumping back to caret (`#859`).
      8. Save confirmation prompt defaults focus to Save button for fast keyboard confirmation (`#861`).
      9. AutoCorrect language scoping: English corrections restricted to English BCP-47 runs, insulating Bangla/non-English words (`#835`).
      10. Track Changes Accept/Reject at caret covers changes split by comments (`#809`).
      11. Comment cards display both date and time stamps (`#812`).
      12. `Ctrl+F` auto-focuses Find input on opening (`#772`).
      13. Paragraph alignment and ribbon clicks return focus immediately to canvas (`#690`).
      14. Ellipses exempted from false-positive sentence capitalization proofing (`#742`).
      15. WMF drawing aspect rendering fills frame like Word (`#739`).
      16. Save As dialog preserves user-typed filenames without reset (`#689`).
      17. Zotero citation note and picture positioning fixes (`#469`, `#470`).
      18. Documentation, brand licensing, and AUR packaging notes (`#509`, `#664`, `#865`).
    - All test suites passing (`wordcraft-layout` 154/154, `wordcraft-ui-egui` 183/183, `bangla` 17/17, `agent` 6/6, `hostile_params_never_panic` 1/1).
    - Recompiled production release binaries (`wordcraft.exe`, `wordcraft-cli.exe`).
    - Rebuilt Inno Setup Windows installer at `packaging/windows/Output/wordcraft-0.6.3-windows-x64-setup.exe` (20.0 MB).
    - Rebuilt portable release archive at `dist/wordcraft/wordcraft-0.6.3-windows-x64-portable.zip` (41.0 MB).


