# Table audit

**Date:** 2026-10-12 · **Scope:** table commands (`crates/engine/src/cmd/table*.rs`), the table model (`crates/doc/src/table.rs`), layout (`crates/layout/src/table.rs` and the table part of `layout/src/lib.rs`), OOXML and export round-trips, the Table Design and Table Layout ribbon, the table context menu, table dialogs, and the table pen and resize UI.

**Method.** Code reading across those files, plus headless probes with the release CLI (`target/release/wordcraft-cli.exe`, built 2026-10-12 10:47; HEAD commit 10:49). Each finding is tagged:

- **Reproduced**: I ran it and checked the output (text export, saved `.docx` or `.html` XML, or a PNG render).
- **Code-read**: the code path is clear from reading; I did not run it.

Nothing in the repository was changed by this audit. Probe outputs are in the session scratchpad.

**Working-tree state.** When the audit finished, `crates/engine/src/cmd/table.rs` and `crates/ui-egui/src/table_resize.rs` had uncommitted edits that were not made by this audit. They change how a cell selection maps to grid columns (`selected_cells`, `select_cell_range`), add `top`, `vcenter` and `bottom` to `table.cellAlign`, fix `set_column_width` for spanned cells, and skip vertically merged continuation cells when grabbing row borders. They do not touch the collapse in `crates/engine/src/cmd/column.rs:153`, so finding A still applies by reading. Probes ran against the release binary built at 10:47, not against the uncommitted source, and line numbers for `cmd/table.rs` and `table_resize.rs` refer to the committed versions. Re-run the probes once those edits land.

**Word reference.** Expected Word behaviour below comes from general knowledge of Word, not from observing Word in this audit. Items marked *(verify)* need checking against Word before fixing.

---

## Summary

| # | Sev | Finding | Status |
|---|---|---|---|
| A | Critical | Every mutating `table.*` command ignores a multi-cell selection. Merge Cells fails on a range; Shading, Insert/Delete Rows and Columns act on the caret cell only. | Reproduced |
| B | Critical | Insert Below and Delete Rows corrupt vertical merges, leaving orphaned continuation cells (blank cells with no top border). | Reproduced |
| C | High | Merge Cells (vertical) leaves an empty paragraph at the top of the merged cell. The Eraser's merge doesn't. | Reproduced |
| D | High | Split Cells grows the grid by a full column, so the table runs past the right margin. | Reproduced |
| E | High | Convert to Text drops character formatting. Convert Text to Table silently drops fields beyond 63 and deletes the source text. | Reproduced |
| F | High | Formula: unknown functions are silently summed; no cell references; F9 never recalculates formula fields. | Reproduced (MEDIAN) / Code-read (F9) |
| G | High | Sort uses a comparator that isn't a total order, so the output isn't sorted. Currency and percent values sort as text. | Reproduced |
| H | High | Backspace at the start of a paragraph after a nested table moves the caret into the outer table's last cell. | Reproduced |
| I | High | Border Painter applies all borders to the whole table at once. Borders and Shading from the table menu sets paragraph borders instead. | Reproduced / Code-read |
| J | Medium | Cell Margins button resets every margin to its default. A partial update resets the sides it doesn't name. | Reproduced |
| K | Medium | Repeat Header Rows on a row that isn't at the top is saved, but never repeats or shows any difference. | Reproduced (save) / Code-read (layout) |
| L | Medium | Sort, Formula, Convert to Text, Convert Text to Table, Split Cells, Delete Cells and Cell Margins run immediately with defaults instead of asking. | Code-read |
| M | Medium | Delete Cells always shifts left and ignores spans. Delete Columns deletes one column even with several selected. | Code-read / Reproduced (with A) |
| N | Medium | Selection commands given an invalid path do nothing and report no error. | Reproduced |
| O | Medium | Sizing: preferred cell width is never used for drawn columns; Distribute Rows grows the table; AutoFit Window discards proportions; the corner resize breaks its own width when the ratio is clamped. | Code-read |
| P | Medium | Merge Cells, Split Cells and the Eraser disagree about merged content and widths. | Reproduced (C) / Code-read |
| Q | Low | Missing UI and features: floating tables, text wrapping, table alt text, cell spacing (dropped on save), pen style and weight for borders, Select Column in the ribbon. | Code-read / grep |
| R | Process | Parity reports Table Design 8/8 and Table Layout 30/30, but only checks that a command id exists. The test suite doesn't run multi-cell commands, so A passed it. | Code-read |
| S | Safety | A comparator that isn't a total order can make `sort_by` panic on Rust 1.81+ (the project targets 1.90). Not reproduced. | Code-read |

---

## A. Multi-cell selection is ignored by table commands (root cause)

**Where.** `crates/engine/src/cmd/column.rs:153-158`: for any mutating command that isn't `text.*`, `format.*`, `para.*` or `edit.paste*`, the column block is collapsed to its first cell (`s.sel = caret; s.column = None`) before the command runs. `Session::run` sends every command through `column::dispatch` (`crates/engine/src/session.rs:687`). The table commands then call `selected_cells()` (`cmd/table.rs:463`), which finds no column block and returns only the caret cell.

**Same path in the GUI.** `app.run` calls `Session::run` (`crates/ui-egui/src/lib.rs:386`). The ribbon, context menu and keyboard therefore behave identically.

**Reproduced.** A `table.quick` matrix (4×4) with a 2×2 range selected through `table.selectRange`:

| Command | Expected (Word) | Result |
|---|---|---|
| `table.merge` | merges the 2×2 block | `select two or more cells to merge` (error) |
| `table.shading` (red) | shades all four cells | only the caret cell is shaded (HTML shows one `background-color`) |
| `table.deleteRow` (rows 1–2) | removes both rows | only row 1 is removed |
| `table.insertRowBelow` (rows 1–2) | inserts after row 2 | a blank row lands between row 1 and row 2 |
| `table.insertRowAbove` (rows 1–2) | inserts two rows above row 1 | one blank row |
| `table.deleteColumn` (columns A–B) | removes both columns | only the first column is removed |

**Not affected** (these commands handle ranges themselves): `format.bold` (four cells bolded), `text.delete` (four cells emptied), copy and cut.

**Merge works only in one case.** A linear selection (Shift+arrows, or `caret.set` with `extend`) leaves `column_segments()` empty, so `dispatch` doesn't collapse it. A mouse drag across cells always creates the block (`crates/ui-egui/src/canvas.rs:1477` sends `table.selectRange`), so the drag path, which is the usual way to select cells, is the broken one.

**Tests miss it.** `crates/engine/src/tests.rs` `table_selection_range_and_headers` (about line 690) checks the selection's segments. No test runs a command on a range.

**Fix direction.** Let `table.*` commands read the column block (exclude them from the collapse, or collapse only for commands that don't read `selected_cells`). Add one regression test per range command.

Reproduce:

```bash
target/release/wordcraft-cli.exe run --cmd 'table.quick={"kind":"matrix"}' --cmd 'table.selectRange={"path":"1","r0":1,"c0":1,"r1":2,"c1":2}' --cmd table.merge
```

---

## B. Vertical merges break on row edits

- **Insert Below on the first row of a merge** (Reproduced): the saved `.docx` has a `<w:vMerge/>` continuation below a plain new row, so the continuation has no restart above it. `Table::insert_row` (`crates/doc/src/table.rs:95-111`) copies the source row's cells and only turns a `Restart` into `None` (line 105). It never repairs the chain below.
- **Delete Rows on the restart row** (Reproduced, PNG render): the merged text disappears and the surviving continuation cell renders as a blank cell with no top border. `Table::delete_row` (`crates/doc/src/table.rs:113-117`) doesn't touch merges.
- **Insert Below on a continuation row** (Code-read): `insert_row` copies `Continue` cells as they are (line 105 handles only `Restart`), so the merge silently extends into the new row.
- **Delete Columns across a merge** (Code-read): `Table::delete_col` (`crates/doc/src/table.rs:166-190`) removes restart cells without repairing the continuation cells in the other rows.

**Fix direction.** One normaliser (for example `Table::repair_vmerge`) that re-derives `Restart` and `Continue` from the grid after any row, column, merge or split change. Add tests for each path.

---

## C. Merge Cells leaves an empty paragraph

- **Reproduced** in the saved `.docx`: the merged cell begins with an empty `<w:p></w:p>` before its text.
- `Table::merge` (`crates/doc/src/table.rs:226-240`) appends the continuation blocks without dropping the restart cell's leading empty paragraph. `merge_right` (line 205) does drop it. The Eraser's vertical merge does too (`crates/engine/src/cmd/table_draw.rs:473-478`). The two merge paths disagree.

---

## D. Split Cells widens the table

- **Reproduced:** splitting a cell in a 4-column table grows the grid from 468 pt to 585 pt. The PNG render shows the table running past the right margin, with a blank column in the header row and in the other rows.
- `Table::split_cell` (`crates/doc/src/table.rs:244-285`) adds columns through `insert_col`, which adds a column's width. It doesn't scale the others. The Insert Columns command does scale (`crates/engine/src/cmd/table.rs:562-568`).

---

## E. Conversions lose formatting and text

- **Convert to Text** (`crates/engine/src/cmd/table.rs:710-730`, helper `cell_text` at line 687) rebuilds each line with `plain_text()`. **Reproduced:** a bold cell (`<strong>1</strong>`) comes out as plain text. Paragraph styles and alignment are lost too (Code-read).
- **Convert Text to Table** (`cmd/table.rs:841-867`) rebuilds cells from `plain_text()` as well (Code-read).
- **Silent truncation.** `cols.clamp(1, 63)` (line 849) and `row.cells.get_mut(c)` drop fields past 63. **Reproduced:** a 70-field line keeps fields 1–63. The source paragraphs are then deleted (lines 859-861), so fields 64–70 are gone for good.
- Lines beyond `MAX_ROWS` (32,767) are dropped the same way (Code-read).

---

## F. Formula

- **Unknown functions are summed.** `formula` ends with `else { nums.iter().sum() }` (`cmd/table.rs:768-770`). **Reproduced:** `=MEDIAN(ABOVE)` returns `12`, the sum of the column. Word doesn't have MEDIAN, so it would show an error. Silently summing is wrong either way.
- **No cell references.** Only the keywords ABOVE, LEFT, RIGHT and BELOW are parsed. `=SUM(B2:B4)` and `=A1+B1` are not supported (Code-read).
- **F9 doesn't recalculate.** The result is stored as a static field result. `update_fields` (`crates/engine/src/cmd/references.rs:269` onward) handles DATE, TIME, SEQ, REF, PAGEREF, TITLE, AUTHOR and NUMWORDS only, so a formula result goes stale after its cells change (Code-read).
- **ABOVE scope** *(verify)*. `formula` sums every numeric cell above, including past text rows and earlier totals. Word's ABOVE may stop at the first empty or non-numeric cell.
- **Ribbon** (`crates/ui-egui/src/ribbon.rs:1870`) inserts `=SUM(ABOVE)` at once. There is no Formula dialog.

---

## G. Sort

- **Not a total order.** The comparator (`cmd/table.rs:698-706`) compares numerically when both keys parse and as text otherwise. **Reproduced:** a 16-row column of `10, 9, 1a, 2b, 100, …` came out as `9 10 10 1a 1a 2b 9 9 10 100 1a 1a 3 9 10 x9`, which isn't sorted at all.
- **Currency and percent values sort as text.** **Reproduced:** `$10, $9, $100` come out as `$10, $100, $9`. Word's numeric sort is expected to read these as numbers *(verify)*.
- **Panic risk.** Rust 1.81 and later may panic in `sort_by` when the comparator isn't a total order, and the project targets Rust 1.90. I did not reproduce a panic (a 400-row run exited 0). This is still a never-crash issue (see S).
- **Wrong cell in merged rows** (Code-read). The key is `a.cells.get(col)` (line 699): the cell index, not the grid column. In a row with a merged cell, the comparison reads the wrong cell.
- **No dialog.** Ribbon "Sort" (`ribbon.rs:1866`) sorts by column 1 at once. Word's Sort dialog asks for the key, type, order and header row.

---

## H. Backspace after a nested table

- **Reproduced.** Put a nested table in a cell, move the caret to the paragraph after it, and press Backspace. The caret lands at `[1,3,3,0]`, the outer table's last cell.
- Cause: the search in `crates/engine/src/cmd/text.rs` (about lines 347-353) matches `q.0.first()`, the outermost index. Inside a nested table that picks the wrong table. It should match the table's own path.

---

## I. Borders

- **Border Painter isn't a painter.** The button runs `table.borders` with `kind: "all"` on the whole table at once (`ribbon.rs:1743`; `crates/engine/src/cmd/tools.rs:289-294`). **Reproduced:** `tblBorders` is written with all six edges single.
- **`table.borders` ignores the selection** and clears every cell's own border override (`cmd/table.rs:643-685`, clearing at lines 679-683). A "Top Border" click wipes per-cell borders across the table (Code-read).
- **Borders and Shading from the table menu sets paragraph borders.** The menu item (`crates/ui-egui/src/canvas.rs:1971`) opens the paragraph Borders dialog (`dialogs.rs:750`). Its OK runs `para.borders` and `para.shading` (`dialogs_para.rs:388-397`). That changes the caret paragraph's border and shading, not table borders or cell shading (Code-read).
- **No pen controls.** The Borders group (`ribbon.rs:1730-1745`) has a menu and Border Painter, but no line style, weight or colour. The `width` and `color` params of `table.borders` have no UI.

---

## J. Cell Margins

- **Reproduced.** The ribbon button (`ribbon.rs:1863`) runs `table.cellMargins` with no parameters, which sets top 0, left and right 5.4, bottom 0 (`crates/engine/src/cmd/tools.rs:272-287`). It's a one-click reset, not an options dialog.
- **Reproduced.** Partial updates reset the other sides. Setting all four to 10, then `{"top":4}`, leaves left, bottom and right at 5.4, 0 and 5.4.

---

## K. Repeat Header Rows

- `table.repeatHeader` toggles the caret row (`cmd/table.rs:207-215`) and accepts any row. Layout repeats only the leading header rows (`crates/layout/src/lib.rs:2008`, `take_while`).
- **Reproduced (save):** a non-leading header row is written as `w:tblHeader`. **Code-read (layout):** it never repeats, and nothing on screen changes. The Table Properties "Header row" checkbox has the same problem (`cmd/table.rs:435-437`).

---

## L. Commands that should ask run blind

- `input_dialog` (`crates/ui-egui/src/lib.rs:1185-1214`) covers only Table Properties among the table commands. Sort, Formula, Convert to Text, Convert Text to Table, Split Cells, Delete Cells and Cell Margins run at once with defaults.
- Menu labels promise dialogs that don't open: "Delete Cells…" (`canvas.rs:1951`), "Split Cells…" (`canvas.rs:1957`), "Convert Text to Table…" (`ribbon.rs:711`).

---

## M. Delete commands

- **Delete Cells** (`cmd/table.rs:29-40`): always shifts left (no shift-up option), removes one cell even when it spans several columns (removing its whole span), and silently does nothing when the row has one cell. Word opens a dialog here (Code-read).
- **Delete Columns** (`cmd/table.rs:586-600`) deletes only the caret's column (see A). `Table::delete_col` (`doc/src/table.rs:189`) also removes any row that ends up with no cells, so that row's content is lost without a message (Code-read).

---

## N. Selection commands

- **Reproduced.** `table.selectRange` (and `selectRow`, `selectColumn`) with a path that isn't a table does nothing and reports no error. In the probe, path `"0"` pointed at a paragraph; the caret stayed put.
- **Code-read.** Out-of-range columns clamp to the last column instead of failing (`select_cell_range`, `cmd/table.rs:779-823`).

---

## O. Sizing and widths

- **Preferred cell width is never used for columns** (Code-read). Layout reads a cell's width only in `is_automatic` (`crates/layout/src/table.rs:972-974`), which switches content autofit on or off. Setting "Cell → Preferred width" in Table Properties changes the saved `tcW` and the autofit switch, but not the drawn column.
- **Table preferred width leaves the grid unscaled** (Code-read). `properties` sets `tblW` (`cmd/table.rs:416-419`) and leaves the grid alone. Table Properties shows column width from the grid, so the dialog's numbers drift away from what's drawn.
- **Corner resize breaks its own width when clamped** (Code-read). `resizeTable` clamps the scale ratio to [0.1, 10] (`cmd/table.rs:175`), but still sets `props.width` to the requested width, so the grid and the stated width disagree whenever the requested change is more than tenfold.
- **Distribute Rows** (`cmd/table.rs:113-120`) gives every row the tallest row's height, so the table grows, and it ignores the selection (Code-read). Word's rule is expected to keep the total height *(verify)*.
- **AutoFit Window** (`cmd/table.rs:306-312`) makes every column equal, discarding the user's proportions (Code-read).
- **AutoFit Contents** (`crates/layout/src/table.rs:91, 153-170`) stops measuring after 4,000 paragraphs and silently leaves later rows out of the fit (Code-read).

---

## P. Merge, split and erase disagree

- Merged content: Merge Cells keeps the leading empty paragraph (C), while the Eraser and `merge_right` strip it.
- Widths: Merge keeps the grid; Split Cells widens it (D); Insert Columns keeps the total by scaling (`cmd/table.rs:562-568`). Three different rules for the same thing.

---

## Q. Missing features

- **Floating tables.** Layout renders floating tables from `w:tblpPr` (`crates/layout/src/lib.rs`, `place_floating_table`), but no command creates or edits table position or text wrapping. A search of `crates/engine/src/cmd/` finds nothing.
- **Table alt text.** Round-trips through `w:tblCaption` and `w:tblDescription` (`crates/docx/src/read/props.rs`, `write/props.rs`), but there's no UI or command.
- **Cell spacing.** `w:tblCellSpacing` is not modelled anywhere in the codebase, so it's dropped on save (grep finds nothing).
- **Table Options dialog** (default cell margins, spacing, alt text) doesn't exist. The Cell Margins button stands in for it (see J).
- **Select Column** is in the catalog (`catalog.rs:341`) but not in the ribbon Select menu (`ribbon.rs:1800-1803`).
- **Calendar quick table** is a fixed grid with no month name (`cmd/table.rs:873-882`).
- **Dead code:** `let _ = StoryRef::Body;` (`cmd/table.rs:920`) and `let _ = row;` (`ribbon.rs:1660`).

---

## R. Parity and tests

- `docs/parity-checklist.md` reports Table Design 8/8 and Table Layout 30/30. The parity function (`crates/engine/src/catalog.rs:461-470`) only checks whether a command id is registered, so the gaps above don't show. The Borders button, Cell Margins and the Sort button all count as done.
- The existing table tests check structure after single-cell commands, and that a range becomes a selection (`tests.rs` about lines 660-720 and 2780-2800). None runs a multi-cell command, so A passed the suite.

---

## S. Never-crash

- **Sort comparator.** See G. A total-order comparator (parse each key once, then compare in fixed tiers) would fix both the wrong order and the panic risk.
- **`split_table`** calls `t.rows.split_off(r)` (`cmd/table.rs:634`), which panics when `r > len`. `r` comes from the caret path, so it's an invariant rather than input, but a checked split would meet the standard.
- **`from_text`** truncates silently (E). That's data loss rather than a panic.

---

## What works

- Draw Table and Eraser geometry (`cmd/table_draw.rs`): coordinates are validated and errors come back as messages.
- Header repeat across pages for leading header rows (`crates/layout/src/lib.rs:2008-2054`).
- The Table Properties dialog sends only the fields that changed (`dialogs.rs`, `TableForm::changes`).
- OOXML round-trip of `gridSpan`, `vMerge`, `tblHeader`, `tblGrid`, `tcW` and `tblW` (`crates/docx/src/read/story.rs:967-1028`, `write/story.rs:830-858`).
- Range format and range delete honour the selection (see A).
- Drag resizing joins its steps into one undo entry (`crates/ui-egui/src/table_resize.rs`).
- Table style management (new, modify, delete and rebase) looks sound.

---

## Not reproduced or to verify

- **Hit-testing in vertical merges.** `Placed::Cell` uses the row height only (`crates/layout/src/table.rs:645`), so a click in the lower rows of a merge may land in the hidden continuation cell. Needs a click test.
- **Sort panic** (see G and S).
- **Resize clamp mismatch** (see O).
- **Insert Above on the header row.** The new top row isn't a header, so header repeat stops. I haven't checked what Word does.
- **Formula ABOVE scope and Distribute Rows semantics** (see F and O). Both need checking against Word.

---

## Suggested order of work

1. **A.** Let `table.*` commands see the range. One change, and it unblocks Merge, Shading, Insert and Delete Rows and Columns, and Cell alignment. Add a regression test for each.
2. **B and C.** A single vertical-merge normaliser, used by row, column, merge and split, with tests.
3. **E, F and G.** Stop the data loss and the wrong results: format-preserving conversions, a visible error for the 63-column cap, formula errors for unknown functions, and a total-order sort.
4. **D, H, I and J.** Split width, the nested Backspace, Border Painter, Borders and Shading from the table menu, and Cell Margins as an options dialog.
5. **L and Q.** Wire the dialogs through `input_dialog`, then the missing features.
6. **R.** Make parity behavioural: a test that runs each command on a range and checks the result.
