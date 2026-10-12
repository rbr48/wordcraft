//! Table commands on cell ranges and vertical merges: each command acts on the whole selection,
//! and every merge chain stays valid after an edit.

use serde_json::{Value, json};
use wordcraft_doc::Table;
use wordcraft_doc::props::VMerge;

use crate::Session;

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A 4 × 4 matrix: a header row ("", "Column A", "Column B", "Column C") and rows "Row 1" to
/// "Row 3". The table is block 1, after the empty paragraph `table.quick` leaves before it.
fn matrix(s: &mut Session) {
    run(s, "table.quick", json!({"kind": "matrix"}));
}

fn table(s: &Session) -> &Table {
    s.doc.body.iter().find_map(|b| b.as_table()).unwrap_or_else(|| panic!("no table"))
}

/// The text of cell `c` (by cell index) in row `r`.
fn cell_text(t: &Table, r: usize, c: usize) -> String {
    t.rows
        .get(r)
        .and_then(|row| row.cells.get(c))
        .map(|cl| cl.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Every continuation cell continues a restart with the same start and span in the row above.
fn assert_merges_valid(t: &Table) {
    let mut above: Vec<(usize, usize, VMerge)> = Vec::new();
    for (r, row) in t.rows.iter().enumerate() {
        let mut x = 0usize;
        let mut this: Vec<(usize, usize, VMerge)> = Vec::new();
        for cl in &row.cells {
            let span = cl.span();
            if cl.props.vmerge == VMerge::Continue {
                let ok = above.iter().any(|(start, sp, vm)| *start == x && *sp == span && *vm != VMerge::None);
                assert!(ok, "row {r}: continuation cell at grid column {x} has no restart above it");
            }
            this.push((x, span, cl.props.vmerge));
            x += span;
        }
        above = this;
    }
}

#[test]
fn insert_below_goes_after_the_last_selected_row() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 3}));
    run(&mut s, "table.insertRowBelow", json!({}));
    let t = table(&s);
    assert_eq!(t.rows.len(), 6);
    assert_eq!(cell_text(t, 1, 0), "Row 1");
    assert_eq!(cell_text(t, 2, 0), "Row 2");
    assert_eq!(cell_text(t, 3, 0), "");
    assert_eq!(cell_text(t, 4, 0), "");
    assert_eq!(cell_text(t, 5, 0), "Row 3");
}

#[test]
fn insert_above_adds_one_row_per_selected_row_above_the_first() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 3}));
    run(&mut s, "table.insertRowAbove", json!({}));
    let t = table(&s);
    assert_eq!(t.rows.len(), 6);
    assert_eq!(cell_text(t, 0, 1), "Column A");
    assert_eq!(cell_text(t, 1, 0), "");
    assert_eq!(cell_text(t, 2, 0), "");
    assert_eq!(cell_text(t, 3, 0), "Row 1");
}

#[test]
fn insert_right_adds_one_column_per_selected_column_and_keeps_the_width() {
    let mut s = s();
    matrix(&mut s);
    let before: f32 = table(&s).grid.iter().sum();
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 0, "c0": 1, "r1": 3, "c1": 2}));
    run(&mut s, "table.insertColumnRight", json!({}));
    let t = table(&s);
    assert_eq!(t.grid.len(), 6, "two columns inserted");
    assert_eq!(cell_text(t, 0, 1), "Column A");
    assert_eq!(cell_text(t, 0, 2), "Column B");
    assert_eq!(cell_text(t, 0, 3), "");
    assert_eq!(cell_text(t, 0, 5), "Column C");
    let after: f32 = t.grid.iter().sum();
    assert!((after - before).abs() < 0.01, "the table keeps its width ({before} → {after})");
    assert_merges_valid(t);
}

#[test]
fn insert_row_in_a_table_with_a_vertical_merge_is_refused_and_changes_nothing() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 2}));
    assert!(s.run("table.insertRowBelow", &json!({})).is_err(), "Word refuses row inserts over vertical merges");
    let t = table(&s);
    assert_eq!(t.rows.len(), 4);
    assert_merges_valid(t);
}

#[test]
fn delete_row_in_a_table_with_a_vertical_merge_is_refused_and_changes_nothing() {
    let mut s = s();
    matrix(&mut s);
    // Rows 1 to 3 of column 0 become one cell; Word refuses to delete any row of that table.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 3, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 1}));
    assert!(s.run("table.deleteRow", &json!({})).is_err());
    let t = table(&s);
    assert_eq!(t.rows.len(), 4);
    assert_merges_valid(t);
    assert!(cell_text(t, 1, 0).contains("Row 3"), "the merged text is untouched");
}

#[test]
fn insert_row_above_a_header_row_is_a_header_too() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 0}));
    run(&mut s, "table.insertRowAbove", json!({}));
    let t = table(&s);
    assert!(t.rows[0].props.header, "the new top row is a header");
    assert!(t.rows[1].props.header, "the old header row stays a header");
    assert!(!t.rows[2].props.header, "the body row is not");
}

#[test]
fn insert_row_below_a_header_row_is_not_a_header() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 0}));
    run(&mut s, "table.insertRowBelow", json!({}));
    let t = table(&s);
    assert!(t.rows[0].props.header);
    assert!(!t.rows[1].props.header, "a row inserted below the header is body text");
}

/// The paragraph in the body whose plain text is `text`.
fn para_with_text<'a>(s: &'a Session, text: &str) -> &'a wordcraft_doc::Paragraph {
    s.doc.body.iter().filter_map(|b| b.as_para()).find(|p| p.plain_text() == text).unwrap_or_else(|| panic!("no paragraph {text:?}"))
}

fn is_bold_somewhere(p: &wordcraft_doc::Paragraph) -> bool {
    p.run_ranges().any(|(_, props)| props.bold == Some(true))
}

#[test]
fn convert_to_text_keeps_character_formatting() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 1}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "table.toText", json!({}));
    let p = para_with_text(&s, "Row 1\t1\t2\t3");
    assert!(is_bold_somewhere(p), "the bold '1' keeps its formatting");
}

#[test]
fn convert_to_text_with_paragraph_separator_gives_one_paragraph_per_cell() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 1, "c1": 1}));
    run(&mut s, "table.toText", json!({"separator": "paragraph"}));
    assert!(s.doc.body.iter().all(|b| b.as_table().is_none()));
    assert!(s.doc.body.iter().filter_map(|b| b.as_para()).any(|p| p.plain_text() == "Row 1"));
    assert!(s.doc.body.iter().filter_map(|b| b.as_para()).any(|p| p.plain_text() == "1"));
}

#[test]
fn convert_text_to_table_keeps_character_formatting() {
    let mut s = s();
    run(&mut s, "document.setText", json!({"text": "alpha\tbeta\tgamma"}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 6}}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 10}, "extend": true}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "table.fromText", json!({}));
    let t = table(&s);
    assert_eq!(t.grid.len(), 3);
    let beta = t.rows.first().and_then(|r| r.cells.get(1)).and_then(|c| c.blocks.first()).and_then(|b| b.as_para()).unwrap_or_else(|| panic!("cell"));
    assert_eq!(beta.plain_text(), "beta");
    assert!(is_bold_somewhere(beta), "the bold cell text keeps its formatting");
}

#[test]
fn convert_text_to_table_refuses_more_columns_than_a_table_has_and_changes_nothing() {
    let mut s = s();
    let fields: Vec<String> = (1..=70).map(|i| format!("f{i}")).collect();
    run(&mut s, "document.setText", json!({"text": fields.join("\t")}));
    run(&mut s, "select.all", json!({}));
    let err = s.run("table.fromText", &json!({})).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(err.contains("63"), "the error names the limit: {err}");
    assert!(s.doc.body.iter().all(|b| b.as_table().is_none()), "no table was made");
    assert!(s.doc.plain_text(wordcraft_doc::StoryRef::Body).contains("f70"), "the text is still there");
}

/// The result of the formula field in cell (`r`, `c`), if the cell holds one.
fn field_result(t: &Table, r: usize, c: usize) -> Option<String> {
    let cl = t.rows.get(r)?.cells.get(c)?;
    cl.blocks.iter().filter_map(|b| b.as_para()).flat_map(|p| p.objects.iter()).find_map(|o| match o {
        wordcraft_doc::para::InlineObject::Field { result, .. } => Some(result.clone()),
        _ => None,
    })
}

#[test]
fn update_field_recalculates_a_formula_from_the_cells_it_reads() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 3, 1, 0], "off": 0}}));
    run(&mut s, "table.insertRowBelow", json!({}));
    run(&mut s, "table.formula", json!({"formula": "=SUM(ABOVE)"}));
    assert_eq!(field_result(table(&s), 4, 1).as_deref(), Some("12"));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 1}}));
    run(&mut s, "text.insert", json!({"text": "00"}));
    run(&mut s, "references.updateFields", json!({}));
    assert_eq!(field_result(table(&s), 4, 1).as_deref(), Some("111"), "the result follows the edited cell");
}

#[test]
fn a_formula_with_cell_references_is_evaluated_against_them() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 3, 1, 0], "off": 0}}));
    run(&mut s, "table.insertRowBelow", json!({}));
    run(&mut s, "table.formula", json!({"formula": "=SUM(B2:B4)"}));
    assert_eq!(field_result(table(&s), 4, 1).as_deref(), Some("12"));
}

#[test]
fn an_unknown_formula_function_inserts_nothing() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 3, 1, 0], "off": 0}}));
    run(&mut s, "table.insertRowBelow", json!({}));
    let err = s.run("table.formula", &json!({"formula": "=MEDIAN(ABOVE)"})).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(err.contains("MEDIAN"), "{err}");
    assert_eq!(field_result(table(&s), 4, 1), None, "no field was inserted");
}

/// The first column's text, row by row.
fn first_column(t: &Table) -> Vec<String> {
    (0..t.rows.len()).map(|r| cell_text(t, r, 0)).collect()
}

fn text_table(s: &mut Session, text: &str) {
    run(s, "document.setText", json!({"text": text}));
    run(s, "select.all", json!({}));
    run(s, "table.fromText", json!({}));
}

#[test]
fn sort_by_number_uses_the_value_and_reads_currency() {
    let mut s = s();
    text_table(&mut s, "a\t$10\nb\t$9\nc\t$100\nd\t1,200\ne\t9\nf\tabc");
    run(&mut s, "table.sort", json!({"column": 1}));
    let t = table(&s);
    let second: Vec<String> = (0..t.rows.len()).map(|r| cell_text(t, r, 1)).collect();
    assert_eq!(second, vec!["$9", "9", "$10", "$100", "1,200", "abc"]);
}

#[test]
fn sort_is_a_total_order_numbers_first_then_text() {
    let mut s = s();
    text_table(&mut s, "10\n9\n1a\n2b\nx9\n3\nb\nA");
    run(&mut s, "table.sort", json!({"column": 0}));
    assert_eq!(first_column(table(&s)), vec!["3", "9", "10", "1a", "2b", "A", "b", "x9"]);
}

#[test]
fn sort_descending_reverses_the_order() {
    let mut s = s();
    text_table(&mut s, "2\n10\n1");
    run(&mut s, "table.sort", json!({"column": 0, "descending": true}));
    assert_eq!(first_column(table(&s)), vec!["10", "2", "1"]);
}

#[test]
fn sort_text_type_compares_characters() {
    let mut s = s();
    text_table(&mut s, "10\n9\n100");
    run(&mut s, "table.sort", json!({"column": 0, "type": "text"}));
    assert_eq!(first_column(table(&s)), vec!["10", "100", "9"]);
}

#[test]
fn sort_keeps_the_header_row_in_place() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.sort", json!({"column": 3, "descending": true}));
    let t = table(&s);
    assert_eq!(cell_text(t, 0, 1), "Column A", "the header stays on top");
    assert_eq!(cell_text(t, 1, 0), "Row 3", "rows sort by column C, largest first");
}

#[test]
fn sort_refuses_rows_with_a_vertical_merge_and_changes_nothing() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    let before = table(&s).clone();
    assert!(s.run("table.sort", &json!({"column": 1})).is_err());
    assert_eq!(table(&s), &before);
}

#[test]
fn sort_refuses_a_table_with_a_horizontal_merge_and_changes_nothing() {
    let mut s = s();
    matrix(&mut s);
    // A merge across two cells of one body row: Word refuses to sort any table with merged cells.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 1, "r1": 2, "c1": 2}));
    run(&mut s, "table.merge", json!({}));
    let before = table(&s).clone();
    assert!(s.run("table.sort", &json!({"column": 0})).is_err());
    assert_eq!(table(&s), &before);
}

fn border_at(cell: &wordcraft_doc::Cell, side: &str) -> Option<wordcraft_doc::props::Border> {
    let b = cell.props.borders?;
    match side {
        "top" => b.top,
        "bottom" => b.bottom,
        "left" => b.left,
        _ => b.right,
    }
}

#[test]
fn borders_on_a_cell_range_change_only_those_cells() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 2, "c1": 2}));
    run(&mut s, "table.borders", json!({"kind": "all", "width": 1.5}));
    let t = table(&s);
    let inner = t.rows[1].cells[1].props.borders;
    assert!(inner.is_some(), "the selected cells get borders");
    assert!(border_at(&t.rows[2].cells[2], "bottom").is_some_and(|b| (b.width - 1.5).abs() < 0.01));
    assert!(
        t.rows[3].cells[1].props.borders.is_none() || border_at(&t.rows[3].cells[1], "top").is_some(),
        "the shared edge is set in the cell below too"
    );
    assert!(t.rows[0].cells[0].props.borders.is_none(), "an unselected corner keeps its borders");
    assert_eq!(t.props.borders, None, "the table's own borders are untouched");
}

#[test]
fn border_painter_paints_the_edge_nearest_the_click() {
    let mut s = s();
    matrix(&mut s);
    // A click in the middle of the first cell's bottom edge: its layout rectangle is in the page.
    let l = s.layout();
    let boxes: Vec<_> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            wordcraft_layout::Placed::Cell { rect, row, cell, .. } => Some((*rect, *row, *cell)),
            _ => None,
        })
        .collect();
    let (rect, row, cell) = *boxes.iter().find(|(_, r, c)| *r == 1 && *c == 1).unwrap_or_else(|| panic!("cell"));
    let (x, y) = (rect.x + rect.w / 2.0, rect.bottom());
    run(&mut s, "table.borderPainter", json!({"page": 0, "x": x, "y": y, "width": 2.0}));
    let t = table(&s);
    assert!(border_at(&t.rows[row].cells[cell], "bottom").is_some_and(|b| (b.width - 2.0).abs() < 0.01));
    assert!(border_at(&t.rows[row + 1].cells[cell], "top").is_some(), "the line is shared with the cell below");
}

fn grid_sum(s: &Session) -> f32 {
    table(s).grid.iter().sum()
}

#[test]
fn delete_cells_shift_up_moves_the_cells_below_up_and_removes_the_bottom_one() {
    let mut s = s();
    matrix(&mut s);
    // Column C (the last column) ends every row, so its bottom cell can go.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 3, "r1": 1, "c1": 3}));
    run(&mut s, "table.deleteCells", json!({"shift": "up"}));
    let t = table(&s);
    assert_eq!(cell_text(t, 1, 3), "6");
    assert_eq!(cell_text(t, 2, 3), "9");
    assert_eq!(t.rows.last().map(|r| r.cells.len()), Some(3), "the bottom cell of that column is removed");
    assert_merges_valid(t);
}

#[test]
fn delete_cells_shift_up_refuses_a_bottom_cell_in_the_middle_of_its_row() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 1}));
    let before = table(&s).clone();
    assert!(s.run("table.deleteCells", &json!({"shift": "up"})).is_err());
    assert_eq!(table(&s), &before, "nothing changes");
}

#[test]
fn delete_cells_shift_left_removes_the_cell_and_moves_the_row_left() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 1}));
    run(&mut s, "table.deleteCells", json!({}));
    let t = table(&s);
    assert_eq!(t.rows[1].cells.len(), 3);
    assert_eq!(cell_text(t, 1, 1), "2");
}

#[test]
fn split_cells_into_rows_and_columns_keeps_the_width_and_the_merges_valid() {
    let mut s = s();
    matrix(&mut s);
    let width = grid_sum(&s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 0}}));
    run(&mut s, "table.split", json!({"columns": 2, "rows": 2}));
    let t = table(&s);
    assert_eq!(t.rows.len(), 5, "one row goes in below");
    assert!((grid_sum(&s) - width).abs() < 0.5, "the table keeps its width");
    assert_merges_valid(t);
    assert_eq!(t.rows[1].cells.first().map(|c| c.props.vmerge), Some(VMerge::Restart), "the row's other cells run down");
}

#[test]
fn split_cells_merge_first_merges_the_selection_and_splits_it() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 2}));
    run(&mut s, "table.split", json!({"columns": 2, "mergeFirst": true}));
    let t = table(&s);
    assert_eq!(t.rows[1].cells.len(), 4, "Row 1, the two halves, and 3");
}

#[test]
fn distribute_columns_levels_only_the_selected_columns() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 0, 1, 0], "off": 0}}));
    run(&mut s, "table.columnWidth", json!({"width": 200.0}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 0, "c0": 2, "r1": 0, "c1": 3}));
    run(&mut s, "table.distributeColumns", json!({}));
    let g = &table(&s).grid;
    assert!((g[2] - g[3]).abs() < 0.01, "the selected columns are equal");
    assert!((g[1] - 200.0).abs() < 0.01, "the column outside the selection keeps its width: {g:?}");
}

#[test]
fn resize_table_keeps_the_grid_and_the_stated_width_equal() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.resizeTable", json!({"width": 300.0}));
    let t = table(&s);
    let sum: f32 = t.grid.iter().sum();
    assert!((sum - 300.0).abs() < 0.5, "grid total {sum}");
    assert!(t.props.width.is_some_and(|w| (w - sum).abs() < 0.5));
}

#[test]
fn autofit_window_keeps_the_proportions_of_the_columns() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 0, 1, 0], "off": 0}}));
    run(&mut s, "table.columnWidth", json!({"width": 200.0}));
    run(&mut s, "table.autofit", json!({"mode": "window"}));
    // The caret's cell is in column 1, widened to 200 pt: it stays about twice as wide as the others.
    let g = &table(&s).grid;
    assert!(g[1] > 1.5 * g[0], "the wide column stays wide: {g:?}");
}

#[test]
fn shading_refuses_a_colour_that_is_not_hex() {
    let mut s = s();
    matrix(&mut s);
    assert!(s.run("table.shading", &json!({"color": "not-a-colour"})).is_err());
}

#[test]
fn cell_margins_keep_the_sides_they_do_not_name() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.cellMargins", json!({"top": 10, "left": 10, "bottom": 10, "right": 10}));
    run(&mut s, "table.cellMargins", json!({"top": 4}));
    assert_eq!(table(&s).props.cell_margins, Some([4.0, 10.0, 10.0, 10.0]));
}

#[test]
fn selection_commands_refuse_a_path_that_is_not_a_table_or_an_out_of_range_cell() {
    let mut s = s();
    matrix(&mut s);
    assert!(s.run("table.selectRange", &json!({"path": "0", "r0": 0, "c0": 0, "r1": 1, "c1": 1})).is_err());
    assert!(s.run("table.selectRange", &json!({"path": "1", "r0": 0, "c0": 0, "r1": 1, "c1": 99})).is_err());
    assert!(s.run("table.selectColumn", &json!({"path": "1", "col": 99})).is_err());
    assert!(s.run("table.selectRow", &json!({"path": "1", "row": 99})).is_err());
}

#[test]
fn the_calendar_puts_the_first_of_the_month_on_its_weekday() {
    // 1 January 2024 was a Monday, so the first week starts with a blank Sunday.
    let rows = super::cmd::table::calendar_rows(super::cmd::table::days_from_civil(2024, 1, 1));
    assert_eq!(rows[0], vec!["January 2024".to_string()]);
    assert_eq!(rows[1][0], "Sun");
    assert_eq!(rows[2], vec!["".to_string(), "1".into(), "2".into(), "3".into(), "4".into(), "5".into(), "6".into()]);
    assert!(rows.iter().any(|r| r.contains(&"31".to_string())), "January has 31 days");
    // February 2024 is a leap year.
    let feb = super::cmd::table::calendar_rows(super::cmd::table::days_from_civil(2024, 2, 10));
    assert!(feb.iter().any(|r| r.contains(&"29".to_string())) && !feb.iter().any(|r| r.contains(&"30".to_string())));
    let (y, m, d) = super::cmd::table::civil_from_days(super::cmd::table::days_from_civil(2026, 10, 12));
    assert_eq!((y, m, d), (2026, 10, 12), "the day counts round-trip");
}

#[test]
fn the_calendar_is_a_table_with_the_month_across_the_top() {
    let mut s = s();
    run(&mut s, "table.quick", json!({"kind": "calendar"}));
    let t = table(&s);
    assert_eq!(t.grid.len(), 7);
    assert_eq!(t.rows[0].cells.len(), 1, "the title spans the week");
    assert_eq!(t.rows[0].cells[0].span(), 7);
    assert!(cell_text(t, 0, 0).contains(' '), "the month and year: {}", cell_text(t, 0, 0));
}

/// The left edge of the first cell of the first row on page 0.
fn first_cell_x(s: &mut Session) -> Option<f32> {
    s.layout().pages.first()?.items.iter().find_map(|it| match it {
        wordcraft_layout::Placed::Cell { rect, cell: 0, row: 0, .. } => Some(rect.x),
        _ => None,
    })
}

#[test]
fn cell_spacing_moves_the_cells_in_by_the_gap_and_survives_a_save() {
    let mut s = s();
    matrix(&mut s);
    let plain = first_cell_x(&mut s).unwrap_or(f32::NAN);
    run(&mut s, "table.properties", json!({"cellSpacing": 4.0}));
    let spaced = first_cell_x(&mut s).unwrap_or(f32::NAN);
    assert!((spaced - plain - 4.0).abs() < 0.01, "the table's edge gap is the spacing: {plain} → {spaced}");
    let back = wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap_or_default()).unwrap_or_default();
    let t = back.body.iter().find_map(|b| b.as_table());
    assert_eq!(t.and_then(|t| t.props.cell_spacing), Some(4.0));
}

#[test]
fn table_positioning_floats_the_table_and_puts_it_back_in_the_text() {
    use wordcraft_doc::para::{Anchor, FloatAlign};
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.position", json!({"floating": true, "hAlign": "center", "hRel": "margin", "vRel": "page", "y": 36.0, "overlap": false}));
    let f = table(&s).props.float.unwrap_or_else(|| panic!("the table floats"));
    assert_eq!((f.h_align, f.h_rel, f.v_rel), (Some(FloatAlign::Center), Anchor::Margin, Anchor::Page));
    assert!((f.y - 36.0).abs() < 0.01 && !f.overlap);
    run(&mut s, "table.position", json!({"floating": false}));
    assert!(table(&s).props.float.is_none(), "back in the text");
    assert!(s.run("table.position", &json!({"floating": true, "hRel": "nowhere"})).is_err());
}

#[test]
fn alternative_text_is_set_and_cleared_through_table_properties() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.properties", json!({"altText": "Sales by region"}));
    assert_eq!(table(&s).props.caption.as_deref(), Some("Sales by region"));
    run(&mut s, "table.properties", json!({"altText": "  "}));
    assert_eq!(table(&s).props.caption, None, "blank text clears the description");
}

#[test]
fn convert_to_text_with_another_character_joins_cells_with_it() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 1, "c1": 1}));
    run(&mut s, "table.toText", json!({"separator": "other", "char": "|"}));
    assert!(s.doc.body.iter().filter_map(|b| b.as_para()).any(|p| p.plain_text() == "Row 1|1|2|3"));
}

/// The tables in the body, in order.
fn tables(s: &Session) -> Vec<&Table> {
    s.doc.body.iter().filter_map(|b| b.as_table()).collect()
}

#[test]
fn copying_a_whole_table_pastes_a_table_with_the_same_cells() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectTable", json!({"path": "1"}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 0}}));
    run(&mut s, "edit.paste", json!({}));
    let all = tables(&s);
    assert_eq!(all.len(), 2, "the copy is a second table");
    let copy = all[0];
    assert_eq!((copy.rows.len(), copy.cols()), (4, 4));
    assert_eq!(cell_text(copy, 1, 0), "Row 1");
    assert_eq!(cell_text(copy, 3, 3), "9");
    assert_merges_valid(copy);
}

#[test]
fn copying_a_cell_range_pastes_a_table_of_that_shape() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 2, "c1": 2}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 0}}));
    run(&mut s, "edit.paste", json!({}));
    let copy = tables(&s)[0];
    assert_eq!((copy.rows.len(), copy.cols()), (2, 2));
    assert_eq!(
        (cell_text(copy, 0, 0), cell_text(copy, 0, 1), cell_text(copy, 1, 0), cell_text(copy, 1, 1)),
        ("1".into(), "2".into(), "4".into(), "5".into())
    );
}

#[test]
fn copying_a_row_pastes_it_as_a_row_above_the_caret() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 1}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 3, 0, 0], "off": 0}}));
    run(&mut s, "edit.paste", json!({}));
    let t = table(&s);
    assert_eq!(t.rows.len(), 5, "one row is added");
    assert_eq!(cell_text(t, 3, 0), "Row 1", "the copy lands above the caret's row");
    assert_eq!(cell_text(t, 4, 0), "Row 3");
    assert_eq!(cell_text(t, 3, 3), "3");
}

#[test]
fn a_copied_range_pasted_over_selected_cells_replaces_them_and_adds_rows() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 2, "c1": 2}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 3, "c0": 3, "r1": 3, "c1": 3}));
    run(&mut s, "edit.paste", json!({}));
    let t = table(&s);
    assert_eq!(cell_text(t, 3, 3), "1", "the top-left cell takes the copy's first cell");
    assert_eq!(cell_text(t, 4, 3), "4", "a taller copy adds rows at the foot");
    assert_eq!(t.rows.len(), 5);
    assert_merges_valid(t);
}

#[test]
fn cutting_a_cell_range_empties_every_paragraph_of_its_cells() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 2}));
    run(&mut s, "edit.cut", json!({}));
    let t = table(&s);
    assert_eq!(cell_text(t, 1, 1), "");
    assert_eq!(cell_text(t, 1, 2), "");
    assert_eq!(cell_text(t, 1, 3), "3", "cells outside the range keep their text");
    assert_eq!(t.rows.len(), 4);
}

#[test]
fn a_copied_cell_pastes_its_text_into_the_caret_cell() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectCell", json!({}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 0}}));
    run(&mut s, "table.selectCell", json!({}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 3, 3, 0], "off": 1}}));
    run(&mut s, "edit.paste", json!({}));
    let t = table(&s);
    assert_eq!(tables(&s).len(), 1, "no nested table: the cell's text is pasted");
    assert_eq!(cell_text(t, 3, 3), "91", "the copied text lands at the caret");
}

/// A `rows` × `cols` table of numbers after the first paragraph, the caret in its first cell.
fn large_table(rows: usize, cols: usize) -> Session {
    let mut s = s();
    let mut t = Table::new(rows, cols, 468.0);
    for (r, row) in t.rows.iter_mut().enumerate() {
        for (c, cell) in row.cells.iter_mut().enumerate() {
            let text = ((r * 7 + c * 13) % 997).to_string();
            cell.blocks = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(&text, Default::default()))];
        }
    }
    s.doc.insert_block(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(1), wordcraft_doc::Block::Table(t)).unwrap();
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 0, 0, 0], "off": 0}}));
    s
}

/// A large table is sorted, summed and laid out in moderate time. The bound is generous (tests run
/// unoptimised, on a shared machine); it catches a quadratic step, not a slow one.
#[test]
fn a_large_table_is_sorted_summed_and_laid_out_in_moderate_time() {
    let start = std::time::Instant::now();
    let mut s = large_table(1500, 12);
    run(&mut s, "table.sort", json!({"column": 0}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1499, 0, 0], "off": 0}}));
    run(&mut s, "table.formula", json!({"formula": "=SUM(ABOVE)"}));
    let pages = s.layout().pages.len();
    let took = start.elapsed();
    assert_eq!(table(&s).rows.len(), 1500);
    assert!(pages > 10, "a long table runs over many pages: {pages}");
    assert!(took.as_secs() < 60, "a 1500 × 12 table took {took:?}");
}

#[test]
fn a_wide_large_table_grows_by_a_column_without_losing_rows() {
    let mut s = large_table(1000, 20);
    run(&mut s, "table.insertColumnRight", json!({}));
    let t = table(&s);
    assert_eq!(t.rows.len(), 1000);
    assert_eq!(t.grid.len(), 21);
    assert_merges_valid(t);
}

/// Every table command with junk parameters, the caret in a table: none panics, and the table keeps
/// its merges valid. (The registry-wide test runs them with the caret outside any table.)
#[test]
fn hostile_table_params_never_panic_inside_a_table() {
    let reg = crate::cmd::registry();
    let junk = [
        json!({"formula": "=IF(", "columns": 1e9, "rows": -5, "column": 99, "row": 4096, "r0": 99, "c0": 99, "r1": -1, "c1": 1e9, "path": "1", "shift": "sideways", "kind": "nope", "text": "\\#\"", "descending": "maybe"}),
        json!({"formula": "=SUM(ABOVE) \\# \"#,##0.00;(\"", "width": 1e308, "widthPct": -1, "cellSpacing": -1e308, "indent": 1e308, "rowHeight": 0, "columnWidth": 1e9, "cellWidth": -4}),
        json!({"formula": "=DEFINED(ZZ99)*0", "separator": "\u{0}", "other": "", "choice": 9, "style": "nope", "color": "zz", "fill": "#", "alt": "x"}),
    ];
    for spec in reg.all().iter().filter(|spec| spec.id.starts_with("table.")) {
        for j in &junk {
            let mut s = s();
            matrix(&mut s);
            let _ = s.run(spec.id, j);
            s.clamp_selection();
            let _ = s.layout();
            // A command may take the table away (Delete Table); then there is nothing to check.
            if let Some(t) = s.doc.body.iter().find_map(|b| b.as_table()) {
                assert_merges_valid(t);
            }
        }
    }
}

#[test]
fn delete_columns_on_a_merged_cell_removes_every_column_it_spans() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 2}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 1}));
    run(&mut s, "table.deleteColumn", json!({}));
    let t = table(&s);
    assert_eq!(t.grid.len(), 2, "the merged cell spanned two columns; both go");
    assert!(t.rows.iter().all(|r| r.cells.iter().map(|c| c.span()).sum::<usize>() == 2));
}

#[test]
fn a_range_from_a_wide_cell_covers_all_of_its_columns() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 2, "r1": 2, "c1": 3}));
    run(&mut s, "table.merge", json!({}));
    // From the merged cell (grid columns 2-3) back to the first cell of the row below.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 2, "r1": 3, "c1": 0}));
    let col = s.column.as_ref().unwrap_or_else(|| panic!("no cell range"));
    let row3 = col.segments.iter().filter(|(a, _)| a.path.cell().is_some_and(|(_, r, _)| r == 3)).count();
    assert_eq!(row3, 4, "row 3's cells under grid columns 0-3 are all selected");
}

#[test]
fn merge_across_rows_with_different_cells_merges_the_rectangle() {
    let mut s = s();
    matrix(&mut s);
    // Row 0 gets a merged cell, so its cell indices no longer line up with row 1's.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 0, "c0": 1, "r1": 0, "c1": 2}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 0, "c0": 2, "r1": 1, "c1": 1}));
    run(&mut s, "table.merge", json!({}));
    let t = table(&s);
    assert_eq!(t.rows[0].cells.len(), 2, "grid columns 1-3 of row 0 are one cell");
    assert_eq!(t.rows[0].cells[1].span(), 3);
    assert_eq!(t.rows[0].cells[1].props.vmerge, VMerge::Restart);
    assert_eq!(t.rows[1].cells[1].props.vmerge, VMerge::Continue);
    assert_merges_valid(t);
}

#[test]
fn split_table_through_a_vertical_merge_leaves_both_tables_valid() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 1, "r1": 2, "c1": 1}));
    run(&mut s, "table.splitTable", json!({}));
    let ts = tables(&s);
    assert_eq!(ts.len(), 2);
    for t in ts {
        assert_merges_valid(t);
        assert!(t.rows.iter().all(|r| r.cells.iter().all(|c| c.props.vmerge != VMerge::Restart)), "a one-row merge is no merge");
    }
}

#[test]
fn a_border_on_one_side_of_the_whole_table_keeps_the_cells_other_borders() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 1}));
    run(&mut s, "table.borders", json!({"kind": "bottom", "style": "single", "width": 1.0}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 0, "c0": 0, "r1": 3, "c1": 3}));
    run(&mut s, "table.borders", json!({"kind": "top", "style": "double", "width": 1.0}));
    let t = table(&s);
    assert!(border_at(&t.rows[1].cells[1], "bottom").is_some(), "the cell's own bottom border stays");
    assert_eq!(t.props.borders.and_then(|b| b.top).map(|b| b.style), Some(wordcraft_doc::props::BorderStyle::Double));
    // All borders on the whole table still replace the cells' own.
    run(&mut s, "table.borders", json!({"kind": "all"}));
    assert!(table(&s).rows.iter().all(|r| r.cells.iter().all(|c| c.props.borders.is_none())));
}

#[test]
fn delete_cells_shift_up_takes_bottom_cells_that_end_their_row_together() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 2, "r1": 3, "c1": 3}));
    run(&mut s, "table.deleteCells", json!({"shift": "up"}));
    let t = table(&s);
    assert_eq!(cell_text(t, 2, 2), "8");
    assert_eq!(cell_text(t, 2, 3), "9");
    assert_eq!(t.rows[3].cells.len(), 2, "the bottom row loses its last two cells");
    assert_eq!(cell_text(t, 3, 1), "7");
}
