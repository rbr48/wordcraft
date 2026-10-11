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
  - Windows x64 Inno Setup installer package at `packaging/windows/Output/wordcraft-0.6.0-windows-x64-setup.exe`.
  - Freshly updated portable release archive at `dist/wordcraft/wordcraft-0.6.0-windows-x64-portable.zip`.
