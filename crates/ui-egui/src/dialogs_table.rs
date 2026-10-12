//! Table dialogs: Sort, Formula, Convert to Text, Convert Text to Table, Split Cells, Delete Cells,
//! Borders and Shading for a table, and Table Positioning. Each ends by running one command
//! (`table.sort`, `table.formula`, …) with the settings it shows, so scripts and agents get the same
//! result without the dialog. Shown through [`crate::dialogs::Dialog::Table`].

use egui::{Ui, vec2};
use serde::Serialize;
use serde_json::{Value, json};

use crate::WordApp;
use crate::dialogs::buttons;
use crate::theme::semibold;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "form", rename_all = "camelCase")]
pub enum TableDialog {
    Sort(SortForm),
    Formula(FormulaForm),
    ToText(SeparatorForm),
    FromText(SeparatorForm),
    Split(SplitForm),
    DeleteCells(DeleteCellsForm),
    Borders(BordersForm),
    Position(PositionForm),
}

impl TableDialog {
    pub fn name(&self) -> &'static str {
        match self {
            TableDialog::Sort(_) => "tableSort",
            TableDialog::Formula(_) => "tableFormula",
            TableDialog::ToText(_) => "tableToText",
            TableDialog::FromText(_) => "tableFromText",
            TableDialog::Split(_) => "tableSplit",
            TableDialog::DeleteCells(_) => "tableDeleteCells",
            TableDialog::Borders(_) => "tableBorders",
            TableDialog::Position(_) => "tablePosition",
        }
    }

    /// The window title (English; the caller translates it).
    pub fn title(&self) -> &'static str {
        match self {
            TableDialog::Sort(_) => "Sort",
            TableDialog::Formula(_) => "Formula",
            TableDialog::ToText(_) => "Convert Table to Text",
            TableDialog::FromText(_) => "Convert Text to Table",
            TableDialog::Split(_) => "Split Cells",
            TableDialog::DeleteCells(_) => "Delete Cells",
            TableDialog::Borders(_) => "Borders and Shading",
            TableDialog::Position(_) => "Table Positioning",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SortForm {
    /// The sort key: 1-based grid column.
    pub column: u32,
    /// The header text of each column (or "Column n" when there is none).
    pub labels: Vec<String>,
    /// `number` or `text`.
    pub kind: String,
    pub descending: bool,
    pub header: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FormulaForm {
    pub formula: String,
    /// The function the paste list chose (kept between openings).
    pub function: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeparatorForm {
    /// 0 paragraph marks (Convert to Text) or one cell per paragraph (Convert Text to Table),
    /// 1 tabs, 2 commas, 3 other.
    pub choice: usize,
    pub other: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SplitForm {
    pub columns: u32,
    pub rows: u32,
    /// Whether the selection covers several cells (so merging first means something).
    pub several: bool,
    pub merge_first: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteCellsForm {
    /// 0 shift cells left, 1 shift cells up, 2 delete the row, 3 delete the column.
    pub choice: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BordersForm {
    /// 0 Borders, 1 Shading.
    pub tab: usize,
    /// none, outside, all, inside, top, bottom, left or right.
    pub setting: String,
    /// Line style (`single`, `double`, `dotted`, …).
    pub style: String,
    /// Line width, points.
    pub width: f32,
    /// `RRGGBB`, or empty for the automatic colour.
    pub color: String,
    /// The fill for the cells, `RRGGBB`, or empty for none.
    pub fill: String,
    pub borders_touched: bool,
    pub fill_touched: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PositionForm {
    pub floating: bool,
    /// 0 absolute position, 1 left, 2 centre, 3 right.
    pub h_mode: usize,
    /// Interface units.
    pub x: f32,
    /// column, margin or page.
    pub h_rel: String,
    /// 0 absolute position, 1 top, 2 centre, 3 bottom.
    pub v_mode: usize,
    pub y: f32,
    /// paragraph, margin or page.
    pub v_rel: String,
    /// Distances from text: left, top, right, bottom (interface units).
    pub dist: [f32; 4],
    pub overlap: bool,
}

const LINE_STYLES: [(&str, &str); 8] = [
    ("single", "Solid"),
    ("double", "Double"),
    ("dotted", "Dotted"),
    ("dashed", "Dashed"),
    ("thick", "Thick"),
    ("triple", "Triple"),
    ("dotDash", "Dot dash"),
    ("wave", "Wave"),
];

const WIDTHS: [f32; 9] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0];

const FUNCTIONS: [&str; 10] = ["SUM", "AVERAGE", "COUNT", "MAX", "MIN", "PRODUCT", "ABS", "INT", "ROUND", "MOD"];

/// Shading swatches: `RRGGBB` values offered in the Shading tab.
const SWATCHES: [&str; 8] = ["FFF2CC", "DEEBF7", "E2EFDA", "FCE4D6", "EDEDED", "D9E1F2", "F8CBAD", "C6E0B4"];

/// The caret's table, if the caret is in one.
fn caret_table(app: &WordApp) -> Option<(wordcraft_doc::Path, usize, usize, &wordcraft_doc::Table)> {
    let s = &app.session;
    let (tp, r, c) = s.sel.focus.path.cell()?;
    let t = s.doc.table(s.sel.focus.story, &tp)?;
    Some((tp, r, c, t))
}

fn cell_text(c: &wordcraft_doc::Cell) -> String {
    c.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join(" ")
}

/// The dialog `ui.dialog` opens by `name`, or `None` for another name.
pub fn open(name: &str, app: &WordApp) -> Option<TableDialog> {
    let caret = caret_table(app);
    Some(match name {
        "tableSort" => {
            let (grid, r, c, t) = match caret {
                Some((_, r, c, t)) => (t.grid.len(), r, c, Some(t)),
                None => (1, 0, 0, None),
            };
            let first_row = t.and_then(|t| t.rows.first());
            let header = first_row.is_some_and(|row| row.props.header);
            // The header row names the columns when there is one; otherwise they are numbered.
            let labels: Vec<String> = (0..grid)
                .map(|g| {
                    let named = if header { t.and_then(|t| t.rows.first()).and_then(|row| row_text_at(row, g)) } else { None };
                    named.filter(|x| !x.trim().is_empty()).unwrap_or_else(|| format!("Column {}", g + 1))
                })
                .collect();
            let column_g = t.map_or(0, |t| t.grid_col(r, c));
            // Numbers when the column reads as numbers throughout the body, text otherwise.
            let body: Vec<String> =
                t.map_or_else(Vec::new, |t| t.rows.iter().skip(usize::from(header)).filter_map(|row| row_text_at(row, column_g)).collect());
            let numeric =
                !body.is_empty() && body.iter().all(|x| x.trim().is_empty() || wordcraft_engine::cmd::table_formula::parse_number(x).is_some());
            SortForm { column: column_g as u32 + 1, labels, kind: if numeric { "number" } else { "text" }.into(), descending: false, header }
                .into_dialog()
        }
        "tableFormula" => FormulaForm { formula: "=SUM(ABOVE)".into(), function: 0 }.into_dialog(),
        "tableToText" => SeparatorForm { choice: 1, other: String::new() }.into_to_text(),
        "tableFromText" => SeparatorForm { choice: 1, other: String::new() }.into_from_text(),
        "tableSplit" => {
            let several = app.session.column_segments().is_some();
            SplitForm { columns: 2, rows: 1, several, merge_first: several }.into_dialog()
        }
        "tableDeleteCells" => DeleteCellsForm { choice: 0 }.into_dialog(),
        "tableBorders" => {
            let fill = caret
                .and_then(|(_, r, c, t)| t.rows.get(r).and_then(|row| row.cells.get(c)))
                .and_then(|cl| cl.props.shading)
                .map(|x| x.hex())
                .unwrap_or_default();
            BordersForm {
                tab: 0,
                setting: "all".into(),
                style: "single".into(),
                width: 0.5,
                color: String::new(),
                fill,
                borders_touched: false,
                fill_touched: false,
            }
            .into_dialog()
        }
        "tablePosition" => {
            let k = wordcraft_geom::Unit::default().pt_per_unit();
            let f = caret.as_ref().and_then(|(_, _, _, t)| t.props.float).unwrap_or_default();
            let mode = |a: Option<wordcraft_doc::para::FloatAlign>| match a {
                None => 0,
                Some(wordcraft_doc::para::FloatAlign::Start) => 1,
                Some(wordcraft_doc::para::FloatAlign::Center) => 2,
                _ => 3,
            };
            PositionForm {
                floating: caret.as_ref().is_some_and(|(_, _, _, t)| t.props.float.is_some()),
                h_mode: mode(f.h_align),
                x: f.x / k,
                h_rel: horizontal_relation(f.h_rel).into(),
                v_mode: mode(f.v_align),
                y: f.y / k,
                v_rel: vertical_relation(f.v_rel).into(),
                dist: f.dist.map(|d| d / k),
                overlap: f.overlap,
            }
            .into_dialog()
        }
        _ => return None,
    })
}

/// The name a horizontal anchor has in the dialog (column, margin or page).
fn horizontal_relation(a: wordcraft_doc::para::Anchor) -> &'static str {
    use wordcraft_doc::para::Anchor;
    match a {
        Anchor::Margin | Anchor::LeftMargin | Anchor::RightMargin => "margin",
        Anchor::Page => "page",
        _ => "column",
    }
}

/// The name a vertical anchor has in the dialog (paragraph, margin or page).
fn vertical_relation(a: wordcraft_doc::para::Anchor) -> &'static str {
    use wordcraft_doc::para::Anchor;
    match a {
        Anchor::Margin | Anchor::LeftMargin | Anchor::RightMargin => "margin",
        Anchor::Page => "page",
        _ => "paragraph",
    }
}

fn row_text_at(row: &wordcraft_doc::Row, g: usize) -> Option<String> {
    let mut x = 0usize;
    for c in &row.cells {
        if g < x + c.span() {
            return Some(cell_text(c));
        }
        x += c.span();
    }
    None
}

impl SortForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::Sort(self)
    }
}
impl FormulaForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::Formula(self)
    }
}
impl SeparatorForm {
    fn into_to_text(self) -> TableDialog {
        TableDialog::ToText(self)
    }
    fn into_from_text(self) -> TableDialog {
        TableDialog::FromText(self)
    }
}
impl SplitForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::Split(self)
    }
}
impl DeleteCellsForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::DeleteCells(self)
    }
}
impl BordersForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::Borders(self)
    }
}
impl PositionForm {
    fn into_dialog(self) -> TableDialog {
        TableDialog::Position(self)
    }
}

/// The separator parameters a choice stands for.
fn separator_params(choice: usize, other: &str) -> Value {
    match choice {
        0 => json!({"separator": "paragraph"}),
        1 => json!({"separator": "tab"}),
        2 => json!({"separator": "comma"}),
        _ => match other.chars().next() {
            Some(c) => json!({"separator": "other", "char": c.to_string()}),
            None => json!({"separator": "tab"}),
        },
    }
}

/// Draws the dialog; returns true to close it.
pub fn body(app: &mut WordApp, ui: &mut Ui, d: &mut TableDialog) -> bool {
    match d {
        TableDialog::Sort(f) => sort_ui(app, ui, f),
        TableDialog::Formula(f) => formula_ui(app, ui, f),
        TableDialog::ToText(f) => separator_ui(app, ui, f, true),
        TableDialog::FromText(f) => separator_ui(app, ui, f, false),
        TableDialog::Split(f) => split_ui(app, ui, f),
        TableDialog::DeleteCells(f) => delete_cells_ui(app, ui, f),
        TableDialog::Borders(f) => borders_ui(app, ui, f),
        TableDialog::Position(f) => position_ui(app, ui, f),
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(tl!(text)).font(semibold(12.5)));
}

fn sort_ui(app: &mut WordApp, ui: &mut Ui, f: &mut SortForm) -> bool {
    heading(ui, "Sort by");
    let max = f.labels.len().max(1) as u32;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sort_col").selected_text(column_label(f)).show_ui(ui, |ui| {
            for (i, label) in f.labels.iter().enumerate() {
                ui.selectable_value(&mut f.column, i as u32 + 1, label.as_str());
            }
        });
    });
    f.column = f.column.clamp(1, max);
    heading(ui, "Type");
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.kind, "number".to_string(), tl!("Number"));
        ui.radio_value(&mut f.kind, "text".to_string(), tl!("Text"));
    });
    heading(ui, "Order");
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.descending, false, tl!("Ascending"));
        ui.radio_value(&mut f.descending, true, tl!("Descending"));
    });
    ui.checkbox(&mut f.header, tl!("Header row"));
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let _ = app.run("table.sort", json!({"column": f.column.saturating_sub(1), "type": f.kind, "descending": f.descending, "header": f.header}));
    }
    ok || cancel
}

fn column_label(f: &SortForm) -> String {
    let i = (f.column.max(1) - 1) as usize;
    f.labels.get(i).cloned().unwrap_or_else(|| format!("Column {}", f.column))
}

fn formula_ui(app: &mut WordApp, ui: &mut Ui, f: &mut FormulaForm) -> bool {
    heading(ui, "Formula");
    ui.add(egui::TextEdit::singleline(&mut f.formula).desired_width(280.0));
    ui.horizontal(|ui| {
        ui.label(tl!("Paste function:"));
        egui::ComboBox::from_id_salt("formula_fn").selected_text(FUNCTIONS.get(f.function).copied().unwrap_or("SUM")).show_ui(ui, |ui| {
            for (i, name) in FUNCTIONS.iter().enumerate() {
                if ui.selectable_value(&mut f.function, i, *name).changed() {
                    f.formula = format!("={name}(ABOVE)");
                }
            }
        });
    });
    ui.label(
        egui::RichText::new(tl!(
            "Cell references (A1, B2:B4), + - * / ^ and parentheses. ABOVE, LEFT, RIGHT and BELOW sum the numbers next to the cell."
        ))
        .weak(),
    );
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && !f.formula.trim().is_empty() {
        let _ = app.run("table.formula", json!({"formula": f.formula}));
    }
    ok || cancel
}

fn separator_ui(app: &mut WordApp, ui: &mut Ui, f: &mut SeparatorForm, to_text: bool) -> bool {
    heading(ui, "Separate text with");
    let first = if to_text { tl!("Paragraph marks") } else { tl!("Paragraphs") };
    ui.radio_value(&mut f.choice, 0, first);
    ui.radio_value(&mut f.choice, 1, tl!("Tabs"));
    ui.radio_value(&mut f.choice, 2, tl!("Commas"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.choice, 3, tl!("Other:"));
        ui.add_enabled(f.choice == 3, egui::TextEdit::singleline(&mut f.other).desired_width(40.0));
    });
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && (f.choice != 3 || !f.other.is_empty()) {
        let id = if to_text { "table.toText" } else { "table.fromText" };
        let _ = app.run(id, separator_params(f.choice, &f.other));
    }
    ok || cancel
}

fn split_ui(app: &mut WordApp, ui: &mut Ui, f: &mut SplitForm) -> bool {
    egui::Grid::new("tsplit").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Number of columns:"));
        ui.add(egui::DragValue::new(&mut f.columns).range(1..=63));
        ui.end_row();
        ui.label(tl!("Number of rows:"));
        ui.add(egui::DragValue::new(&mut f.rows).range(1..=100));
        ui.end_row();
    });
    ui.add_enabled(f.several, egui::Checkbox::new(&mut f.merge_first, tl!("Merge cells before split")));
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let _ = app.run("table.split", json!({"columns": f.columns, "rows": f.rows, "mergeFirst": f.merge_first && f.several}));
    }
    ok || cancel
}

fn delete_cells_ui(app: &mut WordApp, ui: &mut Ui, f: &mut DeleteCellsForm) -> bool {
    ui.radio_value(&mut f.choice, 0, tl!("Shift cells left"));
    ui.radio_value(&mut f.choice, 1, tl!("Shift cells up"));
    ui.radio_value(&mut f.choice, 2, tl!("Delete entire row"));
    ui.radio_value(&mut f.choice, 3, tl!("Delete entire column"));
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let _ = match f.choice {
            0 => app.run("table.deleteCells", json!({"shift": "left"})),
            1 => app.run("table.deleteCells", json!({"shift": "up"})),
            2 => app.run("table.deleteRow", json!({})),
            _ => app.run("table.deleteColumn", json!({})),
        };
    }
    ok || cancel
}

fn borders_ui(app: &mut WordApp, ui: &mut Ui, f: &mut BordersForm) -> bool {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut f.tab, 0, tl!("Borders"));
        ui.selectable_value(&mut f.tab, 1, tl!("Shading"));
    });
    ui.separator();
    if f.tab == 0 {
        heading(ui, "Setting");
        egui::Grid::new("tborders").num_columns(2).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
            for (value, label) in [("none", "None"), ("outside", "Box"), ("all", "All"), ("inside", "Grid inside")] {
                if ui.radio_value(&mut f.setting, value.to_string(), tl!(label)).changed() {
                    f.borders_touched = true;
                }
            }
            ui.end_row();
            for (value, label) in [("top", "Top"), ("bottom", "Bottom"), ("left", "Left"), ("right", "Right")] {
                if ui.radio_value(&mut f.setting, value.to_string(), tl!(label)).changed() {
                    f.borders_touched = true;
                }
            }
            ui.end_row();
        });
        ui.add_space(4.0);
        egui::Grid::new("tpen").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label(tl!("Style:"));
            let shown = LINE_STYLES.iter().find(|(v, _)| *v == f.style).map_or("Solid", |(_, l)| l);
            egui::ComboBox::from_id_salt("tb_style").selected_text(tl!(shown)).show_ui(ui, |ui| {
                for (value, label) in LINE_STYLES {
                    if ui.selectable_value(&mut f.style, value.to_string(), tl!(label)).changed() {
                        f.borders_touched = true;
                    }
                }
            });
            ui.end_row();
            ui.label(tl!("Width:"));
            let width_text = format!("{} pt", f.width);
            egui::ComboBox::from_id_salt("tb_width").selected_text(width_text).show_ui(ui, |ui| {
                for w in WIDTHS {
                    if ui.selectable_value(&mut f.width, w, format!("{w} pt")).changed() {
                        f.borders_touched = true;
                    }
                }
            });
            ui.end_row();
            ui.label(tl!("Colour:"));
            let shown = if f.color.is_empty() { tl!("Automatic").to_string() } else { format!("#{}", f.color) };
            let colour = ui.add(egui::TextEdit::singleline(&mut f.color).hint_text(tl!("Automatic")).desired_width(90.0)).on_hover_text(shown);
            if colour.changed() {
                f.borders_touched = true;
            }
            ui.end_row();
        });
    } else {
        heading(ui, "Fill");
        ui.horizontal_wrapped(|ui| {
            if ui.button(tl!("No Colour")).clicked() {
                f.fill.clear();
                f.fill_touched = true;
            }
            for hex in SWATCHES {
                let color = wordcraft_doc::props::Rgb::parse(hex).map_or(egui::Color32::GRAY, |c| egui::Color32::from_rgb(c.0, c.1, c.2));
                let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::click());
                ui.painter().rect_filled(rect, 3.0, color);
                if resp.clicked() {
                    f.fill = hex.to_string();
                    f.fill_touched = true;
                }
            }
        });
        ui.label(egui::RichText::new(if f.fill.is_empty() { tl!("No colour").to_string() } else { format!("#{}", f.fill) }).weak());
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        if f.borders_touched {
            let mut params = json!({"kind": f.setting, "style": f.style, "width": f.width});
            if !f.color.is_empty() {
                params["color"] = json!(f.color);
            }
            let _ = app.run("table.borders", params);
        }
        if f.fill_touched {
            let color = if f.fill.is_empty() { Value::Null } else { json!(f.fill) };
            let _ = app.run("table.shading", json!({"color": color}));
        }
    }
    ok || cancel
}

fn position_ui(app: &mut WordApp, ui: &mut Ui, f: &mut PositionForm) -> bool {
    let unit = wordcraft_geom::Unit::default();
    let k = unit.pt_per_unit();
    ui.checkbox(&mut f.floating, tl!("Float the table: text wraps around it"));
    ui.add_enabled_ui(f.floating, |ui| {
        heading(ui, "Horizontal");
        egui::Grid::new("tpos_h").num_columns(3).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.radio_value(&mut f.h_mode, 0, tl!("Position:"));
            ui.add_enabled(f.h_mode == 0, egui::DragValue::new(&mut f.x).speed(0.01).suffix(unit.suffix()).max_decimals(2));
            relation(ui, "tpos_hr", &mut f.h_rel, &[("column", "Column"), ("margin", "Margin"), ("page", "Page")]);
            ui.end_row();
            for (mode, label) in [(1, "Left"), (2, "Centre"), (3, "Right")] {
                ui.radio_value(&mut f.h_mode, mode, tl!(label));
                ui.label("");
                ui.label("");
                ui.end_row();
            }
        });
        heading(ui, "Vertical");
        egui::Grid::new("tpos_v").num_columns(3).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.radio_value(&mut f.v_mode, 0, tl!("Position:"));
            ui.add_enabled(f.v_mode == 0, egui::DragValue::new(&mut f.y).speed(0.01).suffix(unit.suffix()).max_decimals(2));
            relation(ui, "tpos_vr", &mut f.v_rel, &[("paragraph", "Paragraph"), ("margin", "Margin"), ("page", "Page")]);
            ui.end_row();
            for (mode, label) in [(1, "Top"), (2, "Centre"), (3, "Bottom")] {
                ui.radio_value(&mut f.v_mode, mode, tl!(label));
                ui.label("");
                ui.label("");
                ui.end_row();
            }
        });
        heading(ui, "Distance from surrounding text");
        egui::Grid::new("tpos_d").num_columns(4).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            for (i, label) in [(0, "Left:"), (1, "Top:"), (2, "Right:"), (3, "Bottom:")] {
                ui.label(tl!(label));
                ui.add(egui::DragValue::new(&mut f.dist[i]).speed(0.01).range(0.0..=5.0).suffix(unit.suffix()).max_decimals(2));
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.checkbox(&mut f.overlap, tl!("Allow overlap with other floating tables"));
    });
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let params = if f.floating {
            let mut p = json!({
                "floating": true,
                "hRel": f.h_rel,
                "vRel": f.v_rel,
                "dist": f.dist.map(|d| d * k),
                "overlap": f.overlap,
            });
            match f.h_mode {
                0 => p["x"] = json!(f.x * k),
                1 => p["hAlign"] = json!("left"),
                2 => p["hAlign"] = json!("center"),
                _ => p["hAlign"] = json!("right"),
            }
            match f.v_mode {
                0 => p["y"] = json!(f.y * k),
                1 => p["vAlign"] = json!("top"),
                2 => p["vAlign"] = json!("center"),
                _ => p["vAlign"] = json!("bottom"),
            }
            p
        } else {
            json!({"floating": false})
        };
        let _ = app.run("table.position", params);
    }
    ok || cancel
}

fn relation(ui: &mut Ui, id: &str, value: &mut String, options: &[(&str, &str)]) {
    let shown = options.iter().find(|(v, _)| v == value).map_or("", |(_, l)| l);
    egui::ComboBox::from_id_salt(id).selected_text(tl!(shown)).show_ui(ui, |ui| {
        for (v, l) in options {
            ui.selectable_value(value, v.to_string(), tl!(l));
        }
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    use crate::WordApp;

    /// A new document with a tabular quick table (a header row, then six body rows), the caret in it.
    fn app_with_table() -> WordApp {
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        a.run("table.quick", json!({"kind": "tabular"})).unwrap();
        a
    }

    /// The plain text of cell `c` in row `r` of the first table.
    fn cell_text(a: &WordApp, r: usize, c: usize) -> String {
        let t = a.session.doc.body.iter().find_map(|b| b.as_table()).expect("a table");
        t.rows[r].cells[c].blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn the_sort_dialog_sorts_the_body_rows_and_keeps_the_header() {
        let mut a = app_with_table();
        a.run("ui.dialog", json!({"name": "tableSort"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        for _ in 0..4 {
            h.step();
        }
        assert!(h.query_by_label("Sort by").is_some(), "the dialog asks for its key");
        assert!(h.query_by_label("Header row").is_some(), "and whether the first row is a header");
        h.get_by_label("Descending").click();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "OK").click();
        for _ in 0..3 {
            h.step();
        }
        let a = h.state();
        assert_eq!(cell_text(a, 0, 0), "Item", "the header stays on top");
        assert_eq!(cell_text(a, 1, 0), "Pens", "the body is sorted descending");
        assert_eq!(cell_text(a, 6, 0), "Books");
    }

    #[test]
    fn the_formula_dialog_asks_for_a_function_and_a_formula() {
        let mut a = app_with_table();
        a.run("ui.dialog", json!({"name": "tableFormula"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        for _ in 0..4 {
            h.step();
        }
        assert!(h.query_by_label("Paste function:").is_some(), "the dialog offers the functions");
    }
}
