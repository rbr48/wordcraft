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
