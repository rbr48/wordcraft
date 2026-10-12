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
fn insert_inside_a_merge_splits_it_without_orphans() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.insertRowBelow", json!({}));
    assert_merges_valid(table(&s));
}

#[test]
fn delete_the_top_row_of_a_merge_keeps_its_text_in_the_row_below() {
    let mut s = s();
    matrix(&mut s);
    // Rows 1 to 3 of column 0 become one cell; deleting row 1 leaves a merge over rows 2 and 3.
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 3, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 1}));
    run(&mut s, "table.deleteRow", json!({}));
    let t = table(&s);
    assert_merges_valid(t);
    let merged = cell_text(t, 1, 0);
    assert!(merged.contains("Row 1") && merged.contains("Row 2") && merged.contains("Row 3"), "merged text kept: {merged:?}");
    assert_eq!(t.rows.get(1).and_then(|row| row.cells.first()).map(|c| c.props.vmerge), Some(VMerge::Restart));
    assert_eq!(t.rows.get(2).and_then(|row| row.cells.first()).map(|c| c.props.vmerge), Some(VMerge::Continue));
}

#[test]
fn deleting_the_only_row_of_a_merge_leaves_an_ordinary_cell() {
    let mut s = s();
    matrix(&mut s);
    run(&mut s, "table.selectRange", json!({"path": "1", "r0": 1, "c0": 0, "r1": 2, "c1": 0}));
    run(&mut s, "table.merge", json!({}));
    run(&mut s, "table.selectRow", json!({"path": "1", "row": 1}));
    run(&mut s, "table.deleteRow", json!({}));
    let t = table(&s);
    assert_merges_valid(t);
    assert!(t.rows.iter().all(|row| row.cells.iter().all(|c| c.props.vmerge == VMerge::None)));
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
