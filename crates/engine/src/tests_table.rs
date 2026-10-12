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

#[test]
fn sort_puts_accented_letters_with_their_base_letter() {
    let mut s = s();
    text_table(&mut s, "Zed\nÉmile\nEva\nÅsa\nAnna\nebba");
    run(&mut s, "table.sort", json!({"column": 0, "type": "text"}));
    assert_eq!(first_column(table(&s)), vec!["Anna", "Åsa", "ebba", "Émile", "Eva", "Zed"]);
}

#[test]
fn a_header_row_set_in_table_properties_takes_the_rows_above_it_along() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 2, 0, 0], "off": 0}}));
    run(&mut s, "table.properties", json!({"headerRow": true}));
    let headers: Vec<bool> = table(&s).rows.iter().map(|r| r.props.header).collect();
    assert_eq!(headers, vec![true, true, true, false], "header rows repeat from the top, so rows 0-2 all are");
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 0, 0], "off": 0}}));
    run(&mut s, "table.properties", json!({"headerRow": false}));
    let headers: Vec<bool> = table(&s).rows.iter().map(|r| r.props.header).collect();
    assert_eq!(headers, vec![true, false, false, false]);
}

#[test]
fn a_tab_or_line_break_in_a_cell_does_not_split_its_clipboard_text() {
    let mut t = Table::new(1, 2, 200.0);
    t.rows[0].cells[0].blocks = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text("a\tb\u{b}c", Default::default()))];
    t.rows[0].cells[1].blocks = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text("d", Default::default()))];
    assert_eq!(crate::cmd::table::table_text(&t), "a b c\td");
}

#[test]
fn widening_a_column_against_its_neighbour_keeps_the_table_width() {
    let mut s = s();
    matrix(&mut s);
    let before = grid_sum(&s);
    run(&mut s, "table.columnWidth", json!({"path": "1", "row": 0, "col": 1, "width": 1000.0, "adjustNext": true}));
    let t = table(&s);
    assert!((grid_sum(&s) - before).abs() < 0.01, "the neighbour stops at its 6pt floor and the column stops there too");
    assert_eq!(t.grid[2], 6.0);
}

#[test]
fn pasting_over_a_merged_cell_keeps_every_copied_cells_text() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 3, "c0": 1, "r1": 3, "c1": 2}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 1, "c1": 2}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 3, "c0": 1, "r1": 3, "c1": 1}));
    run(&mut s, "edit.paste", json!({}));
    let text = cell_text(table(&s), 3, 1);
    assert!(text.contains('1') && text.contains('2'), "both copied cells land in the merged cell: {text:?}");
}

#[test]
fn distribute_rows_reads_the_height_of_rows_sized_by_their_text() {
    let mut s = s();
    matrix(&mut s);
    // Row 1 holds six lines, so it is far taller than 18pt with no height of its own.
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 0}}));
    for _ in 0..5 {
        run(&mut s, "text.insert", json!({"text": "x"}));
        run(&mut s, "text.newParagraph", json!({}));
    }
    run(&mut s, "table.distributeRows", json!({}));
    let hs: Vec<f32> = table(&s).rows.iter().map(|r| r.props.height.unwrap_or(0.0)).collect();
    assert!(hs.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{hs:?}");
    assert!(hs[0] > 25.0, "the average counts row 1's real height, not 18pt: {hs:?}");
}

#[test]
fn pasting_text_over_selected_cells_fills_every_one_of_them() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 2, "c1": 3}));
    run(&mut s, "edit.paste", json!({"text": "Done"}));
    let t = table(&s);
    for r in 1..=2 {
        for c in 1..=3 {
            assert_eq!(cell_text(t, r, c), "Done", "cell ({r}, {c})");
        }
    }
    assert_eq!(cell_text(t, 3, 1), "7", "cells outside the selection keep their text");
    assert_eq!(cell_text(t, 1, 0), "Row 1");
    assert!(s.column.as_ref().is_some_and(|c| c.segments.len() == 6), "the six cells stay selected");
    // One undo takes the whole paste back.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(cell_text(table(&s), 1, 1), "1");
    assert_eq!(cell_text(table(&s), 2, 3), "6");
}

#[test]
fn pasting_a_copied_sentence_over_selected_cells_fills_every_one_of_them() {
    let mut s = s();
    matrix(&mut s);
    // Copy a sentence (WordCraft's own clipboard) from a cell, then paste it over two rows.
    let at = |off: usize| json!({"story": "body", "path": [1, 0, 1, 0], "off": off});
    run(&mut s, "select.range", json!({"anchor": at(0), "focus": at(8)}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 1, "r1": 3, "c1": 1}));
    run(&mut s, "edit.paste", json!({}));
    let t = table(&s);
    assert_eq!(cell_text(t, 2, 1), "Column A");
    assert_eq!(cell_text(t, 3, 1), "Column A");
}

#[test]
fn a_cell_range_grows_to_take_merged_cells_whole() {
    let mut s = s();
    matrix(&mut s);
    // Column 0, rows 1-3 become one cell.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 3, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    // A range from the middle of that cell to its right neighbour takes all three rows.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 2, "c0": 0, "r1": 2, "c1": 1}));
    let col = s.column.as_ref().unwrap_or_else(|| panic!("no cell range"));
    let rows: std::collections::BTreeSet<usize> = col.segments.iter().filter_map(|(a, _)| a.path.cell().map(|(_, r, _)| r)).collect();
    assert_eq!(rows.into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    // Shading that range colours column 1 in all three rows, not row 2 alone.
    run(&mut s, "table.shading", json!({"color": "FFCC00"}));
    let t = table(&s);
    assert!((1..=3).all(|r| t.rows[r].cells[1].props.shading.is_some()), "rows 1-3 of column 1 are shaded");
    assert!(t.rows[0].cells[1].props.shading.is_none());
}

#[test]
fn a_drag_selection_into_a_merged_cell_takes_it_whole() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 3, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    // An ordinary selection from row 2's second cell into the merged cell, then Merge Cells.
    let at = |r: u32, c: u32| json!({"story": "body", "path": [1, r, c, 0], "off": 0});
    run(&mut s, "select.range", json!({"anchor": at(2, 1), "focus": at(1, 0)}));
    run(&mut s, "table.merge", json!({}));
    let t = table(&s);
    assert_merges_valid(t);
    // The merged cell (rows 1-3) and column 1 of those rows became one cell.
    assert_eq!(t.rows[1].cells[0].span(), 2);
    assert_eq!(t.rows[3].cells[0].props.vmerge, VMerge::Continue);
}

#[test]
fn update_field_settles_formulas_that_read_other_formulas_below_them() {
    let mut s = s();
    matrix(&mut s);
    let select = |s: &mut Session, r: u32, c: u32| {
        let at = |off: usize| json!({"story": "body", "path": [1, r, c, 0], "off": off});
        run(s, "select.range", json!({"anchor": at(0), "focus": at(1)}));
    };
    // D2 doubles D3; D3 is D4 plus one; D4 is a plain number.
    select(&mut s, 1, 3);
    run(&mut s, "table.formula", json!({"formula": "=D3*2"}));
    select(&mut s, 2, 3);
    run(&mut s, "table.formula", json!({"formula": "=D4+1"}));
    assert_eq!(field_result(table(&s), 2, 3).as_deref(), Some("10"));
    select(&mut s, 3, 3);
    run(&mut s, "text.insert", json!({"text": "90"}));
    run(&mut s, "references.updateFields", json!({}));
    let t = table(&s);
    assert_eq!(field_result(t, 2, 3).as_deref(), Some("91"));
    assert_eq!(field_result(t, 1, 3).as_deref(), Some("182"), "D2 reads D3's new result, not the old one");
}

#[test]
fn formulas_that_read_each_other_in_a_circle_stop_updating() {
    let mut s = s();
    matrix(&mut s);
    let select = |s: &mut Session, r: u32, c: u32| {
        let at = |off: usize| json!({"story": "body", "path": [1, r, c, 0], "off": off});
        run(s, "select.range", json!({"anchor": at(0), "focus": at(1)}));
    };
    select(&mut s, 1, 1);
    run(&mut s, "table.formula", json!({"formula": "=B3+1"}));
    select(&mut s, 2, 1);
    run(&mut s, "table.formula", json!({"formula": "=B2+1"}));
    // Each pass adds one to both; the passes are bounded, so this returns.
    run(&mut s, "references.updateFields", json!({}));
    assert!(field_result(table(&s), 1, 1).is_some());
}

/// The page rectangles of row `r`'s cells, by cell index.
fn row_cell_rects(s: &mut Session, r: usize) -> Vec<(usize, wordcraft_geom::Rect)> {
    let l = s.layout();
    let mut out: Vec<(usize, wordcraft_geom::Rect)> = l
        .pages
        .iter()
        .flat_map(|pg| pg.items.iter())
        .filter_map(|it| match it {
            wordcraft_layout::Placed::Cell { rect, row, cell, table, .. } if *row == r && table.0.len() == 1 => Some((*cell, *rect)),
            _ => None,
        })
        .collect();
    out.sort_by_key(|x| x.0);
    out
}

#[test]
fn a_right_to_left_table_puts_its_first_column_on_the_right() {
    let mut s = s();
    matrix(&mut s);
    let ltr = row_cell_rects(&mut s, 1);
    assert!(ltr.windows(2).all(|w| w[0].1.x < w[1].1.x), "left to right first");
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 0}}));
    run(&mut s, "table.properties", json!({"rightToLeft": true}));
    assert!(table(&s).props.rtl);
    let rtl = row_cell_rects(&mut s, 1);
    assert_eq!(rtl.len(), 4);
    assert!(rtl.windows(2).all(|w| w[0].1.x > w[1].1.x), "cell 0 is the rightmost: {rtl:?}");
    // The same columns, mirrored: the table's extent doesn't change for a table that fills the
    // text column.
    let span = |v: &[(usize, wordcraft_geom::Rect)]| {
        let lo = v.iter().map(|x| x.1.x).fold(f32::MAX, f32::min);
        let hi = v.iter().map(|x| x.1.right()).fold(f32::MIN, f32::max);
        hi - lo
    };
    assert!((span(&ltr) - span(&rtl)).abs() < 0.5);
    assert!((ltr[0].1.w - rtl[0].1.w).abs() < 0.01, "each column keeps its width");
}

#[test]
fn a_right_to_left_table_survives_a_docx_round_trip() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 1, 0], "off": 0}}));
    run(&mut s, "table.properties", json!({"rightToLeft": true}));
    let bytes = wordcraft_docx::write(&s.doc).unwrap_or_else(|e| panic!("{e}"));
    let back = wordcraft_docx::read(&bytes).unwrap_or_else(|e| panic!("{e}"));
    let t = back.body.iter().find_map(|b| b.as_table()).unwrap_or_else(|| panic!("no table"));
    assert!(t.props.rtl);
    // Turning it off writes no bidiVisual.
    run(&mut s, "table.properties", json!({"rightToLeft": false}));
    let back = wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{e}"));
    assert!(!back.body.iter().find_map(|b| b.as_table()).is_some_and(|t| t.props.rtl));
}

#[test]
fn sort_by_several_keys_breaks_ties_with_the_next_key() {
    let mut s = s();
    text_table(&mut s, "Sales\tAnn\t3\nIT\tBob\t9\nSales\tCyd\t7\nIT\tDee\t2");
    run(&mut s, "table.sort", json!({"keys": [{"column": 0, "type": "text"}, {"column": 2, "descending": true}]}));
    let t = table(&s);
    let names: Vec<String> = (0..t.rows.len()).map(|r| cell_text(t, r, 1)).collect();
    assert_eq!(names, vec!["Bob", "Dee", "Cyd", "Ann"], "IT first, then by number, largest first");
    assert!(s.run("table.sort", &json!({"keys": [{}, {}, {}, {}]})).is_err(), "Word's Sort takes three keys");
    assert!(s.run("table.sort", &json!({"keys": []})).is_err());
}

#[test]
fn sort_by_date_reads_the_dates_word_reads() {
    let mut s = s();
    text_table(&mut s, "12 October 2026\nnot a date\n2026-01-05\n3/4/2025\nOct 1, 2026");
    run(&mut s, "table.sort", json!({"column": 0, "type": "date"}));
    assert_eq!(first_column(table(&s)), vec!["3/4/2025", "2026-01-05", "Oct 1, 2026", "12 October 2026", "not a date"]);
}

#[test]
fn sorting_with_rows_selected_moves_only_those_rows() {
    let mut s = s();
    text_table(&mut s, "5\n4\n3\n2\n1");
    run(&mut s, "table.selectRange", json!({"path": "0", "r0": 1, "c0": 0, "r1": 3, "c1": 0}));
    run(&mut s, "table.sort", json!({"column": 0}));
    assert_eq!(first_column(table(&s)), vec!["5", "2", "3", "4", "1"]);
}

/// A session holding `n` floating tables (3 × 2, 200pt wide) at the same place on the page, each
/// after a paragraph of text, with `overlap` as each table's Allow overlap.
fn floating_tables(overlaps: &[bool]) -> Session {
    use wordcraft_doc::para::Anchor;
    let mut doc = wordcraft_doc::Document::new();
    doc.body.clear();
    for &overlap in overlaps {
        doc.body.push(std::sync::Arc::new(wordcraft_doc::Block::Para(wordcraft_doc::Paragraph::with_text("Text before.", Default::default()))));
        let mut t = Table::new(3, 2, 200.0);
        t.props.float =
            Some(wordcraft_doc::props::TableFloat { h_rel: Anchor::Margin, v_rel: Anchor::Page, x: 0.0, y: 144.0, overlap, ..Default::default() });
        doc.body.push(std::sync::Arc::new(wordcraft_doc::Block::Table(t)));
    }
    let words = "Text that wraps beside the floating tables and must keep clear of them. ".repeat(40);
    doc.body.push(std::sync::Arc::new(wordcraft_doc::Block::Para(wordcraft_doc::Paragraph::with_text(&words, Default::default()))));
    Session::new(doc)
}

/// The page area each table's cells cover, by the table's block index.
fn table_areas(s: &mut Session) -> std::collections::BTreeMap<u32, wordcraft_geom::Rect> {
    let l = s.layout();
    let mut out: std::collections::BTreeMap<u32, wordcraft_geom::Rect> = Default::default();
    for it in l.pages.first().map(|p| p.items.as_slice()).unwrap_or(&[]) {
        if let wordcraft_layout::Placed::Cell { rect, table, .. } = it
            && let [b] = table.0.as_slice()
        {
            let e = out.entry(*b).or_insert(*rect);
            let (x0, y0) = (e.x.min(rect.x), e.y.min(rect.y));
            let (x1, y1) = (e.right().max(rect.right()), e.bottom().max(rect.bottom()));
            *e = wordcraft_geom::Rect::new(x0, y0, x1 - x0, y1 - y0);
        }
    }
    out
}

#[test]
fn a_floating_table_that_may_not_overlap_moves_below_the_one_in_its_way() {
    let mut s = floating_tables(&[false, false]);
    let areas = table_areas(&mut s);
    let (a, b) = (areas[&1], areas[&3]);
    assert!(!a.intersects(&b.inset(0.01, 0.01, 0.01, 0.01)), "{a:?} and {b:?} must not overlap");
    assert!(b.y >= a.bottom() - 0.01, "the second goes below the first");
    assert!((a.x - b.x).abs() < 0.5, "and keeps its horizontal place");
}

#[test]
fn a_floating_table_that_may_overlap_stays_where_it_is_placed() {
    let mut s = floating_tables(&[true, true]);
    let areas = table_areas(&mut s);
    let (a, b) = (areas[&1], areas[&3]);
    assert!((a.y - b.y).abs() < 0.5 && (a.x - b.x).abs() < 0.5, "both at the place they ask for: {a:?} {b:?}");
}

#[test]
fn text_keeps_clear_of_floating_tables() {
    let mut s = floating_tables(&[false, false]);
    let areas = table_areas(&mut s);
    let l = s.layout();
    let mut lines = 0;
    for it in l.pages.first().map(|p| p.items.as_slice()).unwrap_or(&[]) {
        let wordcraft_layout::Placed::Lines { para, l0, l1, x, y, path, .. } = it else { continue };
        if path.0.len() != 1 {
            continue;
        }
        let base = para.lines.get(*l0).map_or(0.0, |l| l.top);
        for line in para.lines.get(*l0..*l1).unwrap_or(&[]) {
            let top = y + line.top - base;
            for v in &line.vis {
                let r = wordcraft_geom::Rect::new(x + v.x, top, v.w.max(0.1), line.height);
                for a in areas.values() {
                    assert!(!r.inset(0.5, 0.5, 0.5, 0.5).intersects(a), "text at {r:?} runs into the table at {a:?}");
                }
            }
            lines += 1;
        }
    }
    assert!(lines > 10, "the text was laid out ({lines} lines)");
}

#[test]
fn autofit_to_contents_keeps_fitting_as_you_type() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 3}));
    run(&mut s, "text.insert", json!({"text": "A"}));
    run(&mut s, "table.autofit", json!({"mode": "contents"}));
    let first = row_cell_rects(&mut s, 0);
    // Typing a long line in the first cell after the fit widens its column, as Word's does.
    run(&mut s, "text.insert", json!({"text": " much longer text in the first column"}));
    let after = row_cell_rects(&mut s, 0);
    assert!(after[0].1.w > first[0].1.w + 20.0, "the column grows with its text: {} -> {}", first[0].1.w, after[0].1.w);
    // Fixed column width turns the live fit off again.
    run(&mut s, "table.autofit", json!({"mode": "fixed"}));
    let fixed = row_cell_rects(&mut s, 0);
    run(&mut s, "text.insert", json!({"text": " and even more words that would widen it"}));
    let still = row_cell_rects(&mut s, 0);
    assert!((still[0].1.w - fixed[0].1.w).abs() < 0.5, "a fixed table keeps its widths");
}

/// Every cell box of the tables nested in the first table, and that table's first cell.
fn nested_cells(s: &mut Session) -> (wordcraft_geom::Rect, Vec<wordcraft_geom::Rect>) {
    let l = s.layout();
    let (mut outer, mut inner) = (None, Vec::new());
    for it in l.pages.iter().flat_map(|p| p.items.iter()) {
        if let wordcraft_layout::Placed::Cell { rect, table, row, cell, .. } = it {
            if table.0.len() == 1 && (*row, *cell) == (0, 0) {
                outer = Some(*rect);
            } else if table.0.len() > 1 {
                inner.push(*rect);
            }
        }
    }
    (outer.unwrap_or_else(|| panic!("no outer cell")), inner)
}

#[test]
fn a_table_inserted_in_a_cell_fits_the_cell() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    let inner_tp = s.sel.focus.path.cell().unwrap_or_else(|| panic!("caret in the nested table")).0;
    let w: f32 = s.doc.table(s.sel.focus.story, &inner_tp).map(|t| t.grid.iter().sum()).unwrap_or(0.0);
    assert!(w < 234.0, "the nested table is narrower than its 234pt cell, not the page's 468pt: {w}");
    let (outer, inner) = nested_cells(&mut s);
    assert_eq!(inner.len(), 4);
    assert!(inner.iter().all(|r| r.x >= outer.x - 0.5 && r.right() <= outer.right() + 0.5), "inside {outer:?}: {inner:?}");
}

#[test]
fn autofit_to_window_in_a_nested_table_fills_its_cell_not_the_page() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "table.autofit", json!({"mode": "window"}));
    let (outer, inner) = nested_cells(&mut s);
    assert!(inner.iter().all(|r| r.right() <= outer.right() + 0.5), "inside {outer:?}: {inner:?}");
    let span = inner.iter().map(|r| r.right()).fold(f32::MIN, f32::max) - inner.iter().map(|r| r.x).fold(f32::MAX, f32::min);
    assert!(span > outer.w - 20.0, "and fills it: {span} of {}", outer.w);
}

/// Every cell box on every page: (page, table path, row, cell, rect).
fn cell_boxes(s: &mut Session) -> Vec<(usize, wordcraft_doc::Path, usize, usize, wordcraft_geom::Rect)> {
    let l = s.layout();
    let mut out = Vec::new();
    for (pi, pg) in l.pages.iter().enumerate() {
        for it in &pg.items {
            if let wordcraft_layout::Placed::Cell { rect, table, row, cell, .. } = it {
                out.push((pi, table.clone(), *row, *cell, *rect));
            }
        }
    }
    out
}

/// Where a click at the middle of `r` (low part when `low`) on page `pi` puts the caret.
fn click(s: &mut Session, pi: usize, r: wordcraft_geom::Rect, low: bool) -> wordcraft_doc::Pos {
    let y = if low { r.bottom() - 3.0 } else { r.y + r.h / 2.0 };
    let l = s.layout();
    l.hit(pi, r.x + r.w / 2.0, y, wordcraft_doc::StoryRef::Body).unwrap_or_else(|| panic!("no hit at {r:?}"))
}

#[test]
fn a_click_low_in_a_vertically_merged_cell_lands_in_it() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 1, "r1": 3, "c1": 1}));
    run(&mut s, "table.merge", json!({}));
    let merged = cell_boxes(&mut s).into_iter().find(|b| b.1.0 == [1] && (b.2, b.3) == (1, 1)).unwrap_or_else(|| panic!("no merged cell"));
    // Low in the merged cell is beside row 3, yet the caret goes into the merged cell's text.
    let pos = click(&mut s, merged.0, merged.4, true);
    let (_, r, c) = pos.path.cell().unwrap_or_else(|| panic!("not in a cell: {pos:?}"));
    assert_eq!((r, c), (1, 1), "the merged cell (row 1), not a continuation: {pos:?}");
}

#[test]
fn a_click_in_a_nested_table_lands_in_its_cell() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    let inner = cell_boxes(&mut s).into_iter().find(|b| b.1.0.len() > 1 && (b.2, b.3) == (1, 1)).unwrap_or_else(|| panic!("no nested cell"));
    let pos = click(&mut s, inner.0, inner.4, false);
    let (tp, r, c) = pos.path.cell().unwrap_or_else(|| panic!("not in a cell"));
    assert_eq!((tp, r, c), (inner.1.clone(), 1, 1), "{pos:?}");
}

#[test]
fn a_click_on_the_part_of_a_row_carried_to_the_next_page_lands_in_that_row() {
    let mut s = s();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    // The second row holds more lines than a page: it splits across pages.
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0, 1, 0, 0], "off": 0}}));
    for _ in 0..80 {
        run(&mut s, "text.insert", json!({"text": "line"}));
        run(&mut s, "text.newParagraph", json!({}));
    }
    let boxes = cell_boxes(&mut s);
    let later =
        boxes.iter().find(|b| b.0 == 1 && b.1.0 == [0] && (b.2, b.3) == (1, 0)).cloned().unwrap_or_else(|| panic!("row 1 doesn't reach page 2"));
    let pos = click(&mut s, later.0, later.4, false);
    let (_, r, c) = pos.path.cell().unwrap_or_else(|| panic!("not in a cell: {pos:?}"));
    assert_eq!((r, c), (1, 0), "{pos:?}");
    let block = pos.path.0.get(3).copied().unwrap_or(0);
    assert!(block > 20, "a paragraph from the part on page 2, not the row's start: {pos:?}");
}

/// Typing in a large table relays it out quickly. Measured 2026-10-12: about 70ms a keystroke
/// with its layout in a 1000 × 6 table (60ms when the table is fixed), against about 200ms for
/// the first layout. The bound is generous (a shared machine); it catches a quadratic step, such
/// as a keystroke that walks every cell of every row once per cell.
#[test]
fn typing_in_a_large_table_relays_it_out_in_moderate_time() {
    let mut s = large_table(1000, 6);
    let pages = s.layout().pages.len();
    assert!(pages > 10, "the table runs over many pages: {pages}");
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 500, 2, 0], "off": 0}}));
    let start = std::time::Instant::now();
    for _ in 0..10 {
        run(&mut s, "text.insert", json!({"text": "x"}));
        let _ = s.layout();
    }
    let per = start.elapsed() / 10;
    assert!(per.as_millis() < 1500, "a keystroke with its layout took {per:?} in a 1000 × 6 table");
    let start = std::time::Instant::now();
    run(&mut s, "table.insertRowBelow", json!({}));
    run(&mut s, "table.selectColumn", json!({}));
    let _ = s.layout();
    assert!(start.elapsed().as_millis() < 3000, "a row insert and a column selection took {:?}", start.elapsed());
}

/// Dragging text inside one cell is an ordinary text selection: the canvas shades the text, not
/// the whole cell. Only a table cell range shades whole cells.
#[test]
fn text_selected_inside_a_cell_is_not_a_cell_range() {
    let mut s = s();
    matrix(&mut s);
    // Text inside the cell "Row 1" (row 1, column 0): select the first word by hand.
    let cell_path = json!([1, 1, 0, 0, 0]);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": cell_path, "off": 0}}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": cell_path, "off": 3}, "extend": true}));
    assert!(!s.is_cell_range(), "a text selection inside a cell must not shade the whole cell");
    assert!(s.column_segments().is_none());
    // A table cell range does shade its cells whole.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 1, "c1": 1}));
    assert!(s.is_cell_range());
}
