//! Dialogs: Font, Paragraph, Find & Replace, Go To, Insert Table, Page Setup, Link, Bookmark,
//! Word Count, Zoom, Watermark, New/Modify Style, Manage Styles, New/Modify Table Style, Table
//! Properties, Command search, Paste Special, About, Save Changes, the password to open a
//! document and Encrypt with Password, and the mail-merge Recipient List, Insert Merge Field, Find
//! Recipient, merge rules, Match Fields and Check for Errors. Every dialog ends by running a
//! command (or shows one's result), so agents get the same result without the dialog.

use egui::{Sense, Stroke, Ui, vec2};
use serde::Serialize;
use serde_json::{Value, json};

use wordcraft_engine::Password;

use crate::WordApp;
use crate::theme::{Tokens, semibold};

/// The Find and Replace dialog's search options beyond the first row (#543).
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindExtra {
    pub wildcards: bool,
    pub prefix: bool,
    pub suffix: bool,
    pub ignore_punct: bool,
    pub ignore_space: bool,
    /// The search options are shown.
    pub more: bool,
    /// Special ▾ inserts into Replace with (it last had the focus), not Find what.
    pub to_replace: bool,
}

/// The Special ▾ menu: (name, code) for Find what, without and with wildcards.
const SPECIAL_FIND: &[(&str, &str)] = &[
    ("Paragraph Mark", "^p"),
    ("Tab Character", "^t"),
    ("Any Character", "^?"),
    ("Any Digit", "^#"),
    ("Any Letter", "^$"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Graphic", "^g"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
    ("Section Break", "^b"),
    ("White Space", "^w"),
];
const SPECIAL_FIND_WILD: &[(&str, &str)] = &[
    ("Any Character", "?"),
    ("Character in Range", "[-]"),
    ("Beginning of Word", "<"),
    ("End of Word", ">"),
    ("Expression", "()"),
    ("Not", "[!]"),
    ("Num Occurrences", "{,}"),
    ("Previous 1 or More", "@"),
    ("0 or More Characters", "*"),
    ("Paragraph Mark", "^13"),
    ("Tab Character", "^t"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Graphic", "^g"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
];
/// Replace with; with wildcards, "Find What Expression" (`\1`) comes first.
const SPECIAL_REPLACE: &[(&str, &str)] = &[
    ("Paragraph Mark", "^p"),
    ("Tab Character", "^t"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Clipboard Contents", "^c"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Find What Text", "^&"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
];

/// The Special ▾ menu button; returns the code picked.
fn special_menu(ui: &mut Ui, extra: &FindExtra) -> Option<&'static str> {
    let mut picked = None;
    ui.menu_button(format!("{} ▾", tl!("Special")), |ui| {
        let wild_replace: &[(&str, &str)] = &[("Find What Expression", "\\1")];
        let lists: [&[(&str, &str)]; 2] = match (extra.to_replace, extra.wildcards) {
            (false, false) => [SPECIAL_FIND, &[]],
            (false, true) => [SPECIAL_FIND_WILD, &[]],
            (true, false) => [SPECIAL_REPLACE, &[]],
            (true, true) => [wild_replace, SPECIAL_REPLACE],
        };
        for (name, code) in lists.into_iter().flatten() {
            if ui.button(format!("{}   {code}", tl!(name))).clicked() {
                picked = Some(*code);
                ui.close();
            }
        }
    });
    picked
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "dialog", rename_all = "camelCase")]
pub enum Dialog {
    Font {
        font: String,
        size: String,
        bold: bool,
        italic: bool,
        underline_style: String,
        strike: bool,
        double_strike: bool,
        sup: bool,
        sub: bool,
        small_caps: bool,
        caps: bool,
        hidden: bool,
        color: String,
        spacing: f32,
        position: f32,
    },
    Paragraph {
        /// Right-to-left reading order (Direction in Word's Paragraph dialog).
        rtl: bool,
        /// Alignment and indents as seen on the page: `left` is the left side whatever the
        /// direction (the model stores them from the paragraph's start edge).
        align: String,
        left: f32,
        right: f32,
        first: f32,
        before: f32,
        after: f32,
        line: f32,
        keep_next: bool,
        keep_lines: bool,
        page_break: bool,
        widow: bool,
        /// Page: 0 Indents and Spacing, 1 Line and Page Breaks, 2 Asian Typography.
        tab: u8,
        /// Asian Typography as the model has it (`para.asianTypography`): kinsoku, word wrap,
        /// hanging punctuation, compress punctuation at line start, space between Asian and
        /// Latin text, space between Asian text and numbers.
        asian: [bool; 6],
        /// `asian` when the dialog opened: OK sets only the flags that changed.
        #[serde(skip)]
        asian_was: [bool; 6],
    },
    /// Tabs (#320).
    Tabs {
        form: Box<crate::dialogs_para::TabsForm>,
    },
    /// Borders and Shading (#320).
    Borders {
        form: Box<crate::dialogs_para::BordersForm>,
    },
    Find {
        query: String,
        replace: String,
        match_case: bool,
        whole_word: bool,
        regex: bool,
        /// Search options under "Search Options" (#543).
        extra: FindExtra,
        replace_mode: bool,
        message: String,
    },
    Goto {
        page: String,
    },
    InsertTable {
        rows: u32,
        cols: u32,
    },
    PageSetup {
        top: f32,
        bottom: f32,
        left: f32,
        right: f32,
        landscape: bool,
    },
    Link {
        url: String,
        text: String,
    },
    Bookmark {
        name: String,
    },
    WordCount {
        stats: Value,
    },
    Zoom {
        percent: f32,
    },
    Watermark {
        text: String,
        diagonal: bool,
    },
    NewStyle {
        name: String,
        based_on: String,
        /// Opened from Manage Styles, which comes back when this closes.
        #[serde(skip)]
        back: bool,
    },
    ModifyStyle {
        #[serde(skip)]
        back: bool,
        id: String,
        name: String,
        font: String,
        size: f32,
        bold: bool,
        italic: bool,
        color: String,
        before: f32,
        after: f32,
    },
    /// Manage Styles: every style with its preview and description; modify, create, delete and
    /// show or hide them.
    ManageStyles {
        alphabetical: bool,
        selected: String,
    },
    /// Table Design › New / Modify Table Style: one formatting region at a time.
    TableStyle {
        /// The style being modified; `None` creates one.
        id: Option<String>,
        name: String,
        /// Style id.
        based_on: String,
        /// Index into `regions` (see [`REGION_LABELS`]).
        region: usize,
        regions: Box<[TableRegion; 7]>,
        /// What the regions showed when the dialog opened (or the base style changed): only
        /// changes are sent, so everything else stays inherited.
        #[serde(skip)]
        basis: Box<[TableRegion; 7]>,
    },
    Commands {
        query: String,
    },
    /// Columns, Symbol and Field ([`crate::dialogs_insert`], #321).
    Insert(Box<crate::dialogs_insert::InsertDialog>),
    /// Table dialogs: Sort, Formula, Convert to Text and Text to Table, Split and Delete Cells,
    /// Borders and Shading for a table, Table Positioning ([`crate::dialogs_table`]).
    Table(Box<crate::dialogs_table::TableDialog>),
    /// Paste Special: the clipboard's formats and the chosen one. The clipboard payload stays out
    /// of the serialized dialog state (it can be megabytes).
    PasteSpecial {
        formats: Vec<String>,
        choice: usize,
        #[serde(skip)]
        payload: Value,
    },
    /// Tabs: 0 About, 1 Contributors, 2 Models.
    About {
        tab: u8,
    },
    Accessibility {
        issues: Vec<Value>,
    },
    CustomizeKeyboard {
        category: String,
        query: String,
        selected_cmd: String,
        new_key: String,
    },
    BanglaConverter {
        source_text: String,
        result_text: String,
    },
    CheckUpdates {
        checking: bool,
        latest_version: String,
        release_notes: String,
        download_url: String,
        status_msg: String,
    },
    /// "Do you want to save changes?" before a user's New, Open, Close, Envelopes, Labels or
    /// Finish & Merge replaces the document (see [`WordApp::run`]); `then` runs once it is
    /// answered (`ui.saveChanges`), and only while the document it asked about
    /// (`Session::document_id`) is still the one open.
    SaveChanges {
        name: String,
        then: String,
        #[serde(skip)]
        params: Value,
        #[serde(skip)]
        document: u64,
    },
    /// Mailings › Select Recipients / Edit Recipient List: a table of recipients to type or
    /// edit (field names on top), or a file to load instead (#240).
    RecipientList {
        fields: Vec<String>,
        rows: Vec<Vec<String>>,
        message: String,
    },
    /// Mailings › Insert Merge Field: one of the recipient list's fields (or a typed name).
    InsertMergeField {
        fields: Vec<String>,
        field: String,
    },
    /// Mailings › Find Recipient.
    FindRecipient {
        text: String,
        message: String,
    },
    /// Mailings › Rules › If…Then…Else… (`IF`) or Skip Record If… (`SKIPIF`): the condition, and
    /// for If the text to insert either way.
    MergeRule {
        rule: String,
        fields: Vec<String>,
        field: String,
        value: String,
        then: String,
        els: String,
    },
    /// Mailings › Match Fields: which recipient-list field fills each address part.
    MatchFields {
        fields: Vec<String>,
        address: Value,
    },
    /// Mailings › Check for Errors: merge fields the recipient list doesn't have.
    CheckErrors {
        unknown: Vec<String>,
        records: u64,
    },
    /// Table Layout › Properties: the table, the caret's row, column and cell. Only what changed
    /// from `basis` (the values when it opened) is applied, as one `table.properties` step.
    TableProperties {
        form: Box<TableForm>,
        #[serde(skip)]
        basis: Box<TableForm>,
    },
    /// Table Layout › Alignment › Cell Margins: top, left, bottom, right in interface units.
    CellMargins {
        top: f32,
        left: f32,
        bottom: f32,
        right: f32,
    },
    /// Opening a password-protected document (#55): the password, then `file.open` with
    /// `params` again. The password is never serialised or printed.
    Password {
        name: String,
        #[serde(skip)]
        params: Value,
        #[serde(skip)]
        password: Password,
        message: String,
    },
    /// File › Info › Protect Document › Encrypt with Password: a password typed twice
    /// (`file.encrypt`); left empty, it removes the password.
    EncryptPassword {
        #[serde(skip)]
        password: Password,
        #[serde(skip)]
        confirm: Password,
        message: String,
    },
    /// Home › Paragraph › Multilevel List › Define New Multilevel List (#328).
    DefineList {
        form: Box<crate::dialogs_lists::ListForm>,
    },
    /// Review › Tracking › Track Changes Options (#328).
    TrackOptions {
        form: Box<crate::dialogs_lists::TrackForm>,
    },
}

/// The Table Properties dialog's fields. Lengths are in the interface unit
/// ([`wordcraft_geom::Unit`], inches by default).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableForm {
    pub cell_spacing_on: bool,
    pub cell_spacing: f32,
    pub alt_text: String,
    pub width_on: bool,
    pub width: f32,
    /// `left`, `center` or `right`.
    pub align: String,
    /// Table direction: right to left (the first column on the right).
    pub rtl: bool,
    pub indent: f32,
    pub row_height_on: bool,
    pub row_height: f32,
    pub row_exact: bool,
    pub allow_break: bool,
    pub header_row: bool,
    pub column_width: f32,
    pub cell_width_on: bool,
    pub cell_width: f32,
    /// `top`, `center` or `bottom`.
    pub valign: String,
}

impl TableForm {
    /// The caret's table, row, column and cell as the dialog shows them.
    pub fn read(app: &WordApp) -> Option<TableForm> {
        use wordcraft_doc::props::{Align, HeightRule, VAlign};
        let s = &app.session;
        let (tp, r, c) = s.sel.focus.path.cell()?;
        let t = s.doc.table(s.sel.focus.story, &tp)?;
        let k = wordcraft_geom::Unit::default().pt_per_unit();
        let row = t.rows.get(r).map(|x| x.props.clone()).unwrap_or_default();
        let cell = t.rows.get(r).and_then(|x| x.cells.get(c)).map(|x| x.props.clone()).unwrap_or_default();
        let tw = wordcraft_engine::cmd::page::sect(s).text_width();
        let width = t.props.width_pct.filter(|p| *p > 0.0).map(|p| tw * p.min(100.0) / 100.0).or(t.props.width.filter(|w| *w > 0.0));
        let fin = |v: f32| if v.is_finite() { v } else { 0.0 };
        Some(TableForm {
            cell_spacing_on: t.props.cell_spacing.is_some_and(|x| x > 0.0),
            cell_spacing: fin(t.props.cell_spacing.unwrap_or(0.0)) / k,
            alt_text: t.props.caption.clone().unwrap_or_default(),
            rtl: t.props.rtl,
            width_on: width.is_some(),
            width: fin(width.unwrap_or(tw)) / k,
            align: match t.props.align {
                Some(Align::Center) => "center",
                Some(Align::Right) => "right",
                _ => "left",
            }
            .into(),
            indent: fin(t.props.indent.unwrap_or(0.0)) / k,
            row_height_on: row.height.is_some() && row.height_rule != HeightRule::Auto,
            row_height: fin(row.height.unwrap_or(18.0)) / k,
            row_exact: row.height_rule == HeightRule::Exact,
            allow_break: !row.cant_split,
            header_row: row.header,
            column_width: fin(t.grid.get(t.grid_col(r, c)).copied().unwrap_or(72.0)) / k,
            cell_width_on: cell.width.is_some(),
            cell_width: fin(cell.width.unwrap_or(72.0)) / k,
            valign: match cell.valign {
                VAlign::Center => "center",
                VAlign::Bottom => "bottom",
                _ => "top",
            }
            .into(),
        })
    }

    /// `table.properties` parameters for what differs from `basis` (empty when nothing does).
    pub fn changes(&self, basis: &TableForm) -> Value {
        let k = wordcraft_geom::Unit::default().pt_per_unit();
        let moved = |a: f32, b: f32| (a - b).abs() > 1e-4;
        let mut v = serde_json::Map::new();
        if self.cell_spacing_on != basis.cell_spacing_on || (self.cell_spacing_on && moved(self.cell_spacing, basis.cell_spacing)) {
            v.insert("cellSpacing".into(), if self.cell_spacing_on { json!(self.cell_spacing * k) } else { Value::Null });
        }
        if self.alt_text != basis.alt_text {
            v.insert("altText".into(), json!(self.alt_text));
        }
        if self.width_on != basis.width_on || (self.width_on && moved(self.width, basis.width)) {
            v.insert("width".into(), if self.width_on { json!(self.width * k) } else { Value::Null });
        }
        if self.align != basis.align {
            v.insert("align".into(), json!(self.align));
        }
        if self.rtl != basis.rtl {
            v.insert("rightToLeft".into(), json!(self.rtl));
        }
        if moved(self.indent, basis.indent) {
            v.insert("indent".into(), json!(self.indent * k));
        }
        if self.row_height_on != basis.row_height_on
            || (self.row_height_on && (moved(self.row_height, basis.row_height) || self.row_exact != basis.row_exact))
        {
            v.insert("rowHeight".into(), if self.row_height_on { json!(self.row_height * k) } else { Value::Null });
            v.insert("rowHeightRule".into(), json!(if self.row_exact { "exact" } else { "atLeast" }));
        }
        if self.allow_break != basis.allow_break {
            v.insert("allowBreak".into(), json!(self.allow_break));
        }
        if self.header_row != basis.header_row {
            v.insert("headerRow".into(), json!(self.header_row));
        }
        if moved(self.column_width, basis.column_width) {
            v.insert("columnWidth".into(), json!(self.column_width * k));
        }
        if self.cell_width_on != basis.cell_width_on || (self.cell_width_on && moved(self.cell_width, basis.cell_width)) {
            v.insert("cellWidth".into(), if self.cell_width_on { json!(self.cell_width * k) } else { Value::Null });
        }
        if self.valign != basis.valign {
            v.insert("valign".into(), json!(self.valign));
        }
        Value::Object(v)
    }
}

/// The address parts Match Fields lists, with their keys in `mailings.matchFields`' `address`.
pub const ADDRESS_PARTS: [(&str, &str); 9] = [
    ("Courtesy Title", "title"),
    ("First Name", "firstName"),
    ("Last Name", "lastName"),
    ("Company", "company"),
    ("Address", "address"),
    ("City", "city"),
    ("State", "state"),
    ("Postal Code", "postal"),
    ("Country", "country"),
];

/// The `mailings.rules` parameters a Merge Rule dialog stands for, once it names a field.
pub fn rule_params(d: &Dialog) -> Option<Value> {
    let Dialog::MergeRule { rule, field, value, then, els, .. } = d else { return None };
    let field = field.trim();
    if field.is_empty() {
        return None;
    }
    Some(if rule == "SKIPIF" {
        json!({"rule": "SKIPIF", "field": field, "value": value})
    } else {
        json!({"rule": "IF", "field": field, "value": value, "then": then, "else": els})
    })
}

/// The fields a new recipient list starts with.
pub const NEW_LIST_FIELDS: [&str; 5] = ["First Name", "Last Name", "Address", "City", "Postal Code"];

/// A typed recipient list ready for `mailings.recipients`: field names trimmed, blank rows
/// dropped. An error says what to fix.
pub fn recipient_table(fields: &[String], rows: &[Vec<String>]) -> Result<Value, &'static str> {
    let fields: Vec<&str> = fields.iter().map(|f| f.trim()).collect();
    if fields.is_empty() {
        return Err("Add at least one field.");
    }
    if fields.iter().any(|f| f.is_empty()) {
        return Err("Every field needs a name.");
    }
    for (i, f) in fields.iter().enumerate() {
        if fields.iter().skip(i + 1).any(|g| g.eq_ignore_ascii_case(f)) {
            return Err("Field names must be different.");
        }
    }
    let rows: Vec<&Vec<String>> = rows.iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).collect();
    if rows.is_empty() {
        return Err("Type at least one recipient.");
    }
    Ok(json!({"fields": fields, "rows": rows}))
}

/// One table style region in the Table Style dialog: colours are hex, empty for none.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRegion {
    pub borders: bool,
    pub fill: String,
    pub bold: bool,
    pub color: String,
}

/// The Table Style dialog's regions, in the order of `table_style::REGIONS` (the param names).
const REGION_LABELS: [&str; 7] = ["Whole Table", "Header Row", "Total Row", "First Column", "Last Column", "Banded Rows", "Banded Columns"];

/// The regions of table style `id` as resolved through its based-on chain.
fn table_regions(app: &WordApp, id: &str) -> Box<[TableRegion; 7]> {
    let Some(st) = app.session.doc.styles.table_style(id) else { return Default::default() };
    let p = &st.parts;
    let hex = |c: Option<wordcraft_doc::Rgb>| c.map(|c| c.hex()).unwrap_or_default();
    let text = |c: &wordcraft_doc::CharProps| match c.color {
        Some(wordcraft_doc::TextColor::Rgb(c)) => c.hex(),
        _ => String::new(),
    };
    let lines = |b: Option<wordcraft_doc::props::Borders>| b.is_some_and(|b| b.any_visible());
    // A region's text: the whole table's formatting with the region's over it.
    let region = |borders: Option<wordcraft_doc::props::Borders>, fill: Option<wordcraft_doc::Rgb>, chr: &wordcraft_doc::CharProps| {
        let mut c = st.chr.clone();
        c.overlay(chr);
        TableRegion { borders: lines(borders), fill: hex(fill), bold: c.bold.unwrap_or(false), color: text(&c) }
    };
    let none = wordcraft_doc::CharProps::default();
    Box::new([
        region(p.borders, p.fill, &none),
        region(p.header_borders, p.header_fill, &p.header_chr),
        region(p.total_borders, p.total_fill, &p.total_chr),
        region(p.first_col_borders, p.first_col_fill, &p.first_col_chr),
        region(p.last_col_borders, p.last_col_fill, &p.last_col_chr),
        region(p.band_borders, p.band_fill, &p.band_chr),
        region(p.col_band_borders, p.col_band_fill, &p.col_band_chr),
    ])
}

/// The params `table.newStyle` / `table.modifyStyle` take for the Table Style dialog's state:
/// only what changed from `basis`, so the rest stays inherited.
fn table_style_params(id: Option<&str>, name: &str, based_on: &str, regions: &[TableRegion; 7], basis: &[TableRegion; 7]) -> Value {
    let mut v = json!({"name": name.trim(), "basedOn": based_on});
    for ((key, r), b) in wordcraft_engine::cmd::table_style::REGIONS.into_iter().zip(regions.iter()).zip(basis.iter()) {
        let ch = region_changes(r, b);
        if ch.as_object().is_some_and(|o| !o.is_empty()) {
            v[key] = ch;
        }
    }
    if let Some(id) = id {
        v["style"] = json!(id);
    }
    v
}

/// A small sample table drawn with `style` (5 columns, a header, three body rows and a total
/// row), showing the regions `look` turns on: the Table Style dialog's live preview.
fn table_style_preview(ui: &mut Ui, style: Option<&wordcraft_doc::styles::TableStyleProps>, look: wordcraft_doc::props::TableLook) {
    use wordcraft_doc::props::{Border, Borders};
    const COLS: usize = 5;
    const ROWS: usize = 5;
    const TEXT: [[&str; COLS]; ROWS] = [
        ["", "Mon", "Tue", "Wed", "Sum"],
        ["North", "4", "7", "2", "13"],
        ["South", "6", "1", "5", "12"],
        ["West", "3", "8", "4", "15"],
        ["Total", "13", "16", "11", "40"],
    ];
    let (rect, _) = ui.allocate_exact_size(vec2(300.0, 120.0), Sense::hover());
    let painter = ui.painter_at(rect);
    // The page is white whatever the interface theme.
    painter.rect_filled(rect, 2.0, egui::Color32::WHITE);
    let t = Tokens::get(ui.ctx());
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let Some(st) = style else { return };
    let p = &st.parts;
    let table = rect.shrink2(vec2(14.0, 10.0));
    let (cw, rh) = (table.width() / COLS as f32, table.height() / ROWS as f32);
    let rgb = |c: wordcraft_doc::Rgb| egui::Color32::from_rgb(c.0, c.1, c.2);
    let tb = p.borders.unwrap_or_default();
    for (ri, row) in TEXT.iter().enumerate() {
        let header = look.header_row && ri == 0;
        let total = look.total_row && ri + 1 == ROWS;
        let band = look.banded_rows
            && !header
            && (ri.saturating_sub(usize::from(look.header_row)) / p.band_size.unwrap_or(1).clamp(1, 1000) as usize).is_multiple_of(2);
        for (ci, txt) in row.iter().enumerate() {
            let first = look.first_column && ci == 0;
            let last = look.last_column && ci + 1 == COLS;
            let col_band = look.banded_columns && !first && ci.saturating_sub(usize::from(look.first_column)).is_multiple_of(2);
            let cell = egui::Rect::from_min_size(table.min + vec2(ci as f32 * cw, ri as f32 * rh), vec2(cw, rh));
            // Regions from lowest to highest priority, as layout applies them.
            let regions = [
                (col_band, p.col_band_fill, &p.col_band_chr, p.col_band_borders),
                (band, p.band_fill, &p.band_chr, p.band_borders),
                (first, p.first_col_fill, &p.first_col_chr, p.first_col_borders),
                (last, p.last_col_fill, &p.last_col_chr, p.last_col_borders),
                (header, p.header_fill, &p.header_chr, p.header_borders),
                (total, p.total_fill, &p.total_chr, p.total_borders),
            ];
            let mut fill = p.fill;
            let mut chr = st.chr.clone();
            let mut b = Borders {
                top: if ri == 0 { tb.top } else { tb.between },
                bottom: if ri + 1 == ROWS { tb.bottom } else { tb.between },
                left: if ci == 0 { tb.left } else { tb.inside_v },
                right: if ci + 1 == COLS { tb.right } else { tb.inside_v },
                between: None,
                inside_v: None,
            };
            for (on, f, c, rb) in regions {
                if !on {
                    continue;
                }
                fill = f.or(fill);
                chr.overlay(c);
                if let Some(rb) = rb {
                    b.overlay(&Borders { between: None, inside_v: None, ..rb });
                }
            }
            if total && p.total_borders.is_none_or(|x| x.top.is_none()) {
                b.top = p.total_border_top.or(b.top);
            }
            if let Some(f) = fill {
                painter.rect_filled(cell, 0.0, rgb(f));
            }
            let edge = |from: egui::Pos2, to: egui::Pos2, e: Option<Border>| {
                if let Some(e) = e.filter(Border::is_visible) {
                    let color = e.color.map_or(egui::Color32::BLACK, rgb);
                    painter.line_segment([from, to], egui::Stroke::new(e.width.clamp(0.5, 3.0) * 1.3, color));
                }
            };
            edge(cell.left_top(), cell.right_top(), b.top);
            edge(cell.left_bottom(), cell.right_bottom(), b.bottom);
            edge(cell.left_top(), cell.left_bottom(), b.left);
            edge(cell.right_top(), cell.right_bottom(), b.right);
            let color = match chr.color {
                Some(wordcraft_doc::TextColor::Rgb(c)) => rgb(c),
                _ => egui::Color32::BLACK,
            };
            let font = egui::FontId::proportional(11.0);
            let pos = cell.left_center() + vec2(4.0, 0.0);
            painter.text(pos, egui::Align2::LEFT_CENTER, tl!(*txt), font.clone(), color);
            if chr.bold.unwrap_or(false) {
                // A second pass a hair to the right reads as bold at this size.
                painter.text(pos + vec2(0.6, 0.0), egui::Align2::LEFT_CENTER, tl!(*txt), font, color);
            }
        }
    }
}

/// The style of the table at the caret, when it is a table style.
fn current_table_style(app: &WordApp) -> Option<String> {
    let s = &app.session;
    let (tp, _, _) = s.sel.focus.path.cell()?;
    let id = s.doc.table(s.sel.focus.story, &tp)?.props.style.clone()?;
    s.doc.styles.get(&id).filter(|st| st.kind == wordcraft_doc::StyleKind::Table).map(|st| st.id.clone())
}

/// The params `table.newStyle` / `table.modifyStyle` need for what changed in one region.
fn region_changes(r: &TableRegion, basis: &TableRegion) -> Value {
    let color = |c: &str| if c.is_empty() { Value::Null } else { json!(c) };
    let mut v = json!({});
    if r.borders != basis.borders {
        v["borders"] = json!(r.borders);
    }
    if r.fill != basis.fill {
        v["fill"] = color(&r.fill);
    }
    if r.bold != basis.bold {
        v["bold"] = json!(r.bold);
    }
    if r.color != basis.color {
        v["color"] = color(&r.color);
    }
    v
}

/// A colour menu: a swatch with the colour grid and No Color. `value` is hex, empty for none.
pub(crate) fn color_menu(ui: &mut Ui, theme: &[wordcraft_doc::Rgb], value: &mut String) {
    let c = wordcraft_doc::Rgb::parse(value);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        let t = Tokens::get(ui.ctx());
        match c {
            Some(c) => {
                ui.painter().rect_filled(r, 2.0, crate::theme::c32(c));
            }
            None => {
                ui.painter().line_segment([r.left_bottom(), r.right_top()], egui::Stroke::new(1.0, t.text_dim));
            }
        }
        ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
        let label = if c.is_some() { value.clone() } else { tl!("No Color").to_string() };
        ui.menu_button(label, |ui| {
            if ui.button(tl!("No Color")).clicked() {
                value.clear();
                ui.close();
            }
            if let Some(hex) = crate::widgets::color_grid(ui, theme) {
                *value = hex;
                ui.close();
            }
        });
    });
}

impl Dialog {
    pub fn name(&self) -> &'static str {
        match self {
            Dialog::Font { .. } => "font",
            Dialog::Paragraph { .. } => "paragraph",
            Dialog::Tabs { .. } => "tabs",
            Dialog::Borders { .. } => "borders",
            Dialog::Find { replace_mode: false, .. } => "find",
            Dialog::Find { .. } => "replace",
            Dialog::Goto { .. } => "goto",
            Dialog::InsertTable { .. } => "insertTable",
            Dialog::PageSetup { .. } => "pageSetup",
            Dialog::Link { .. } => "link",
            Dialog::Bookmark { .. } => "bookmark",
            Dialog::WordCount { .. } => "wordCount",
            Dialog::Zoom { .. } => "zoom",
            Dialog::Watermark { .. } => "watermark",
            Dialog::NewStyle { .. } => "newStyle",
            Dialog::ModifyStyle { .. } => "modifyStyle",
            Dialog::ManageStyles { .. } => "manageStyles",
            Dialog::TableStyle { id: None, .. } => "newTableStyle",
            Dialog::TableStyle { .. } => "modifyTableStyle",
            Dialog::Commands { .. } => "commands",
            Dialog::Insert(d) => d.name(),
            Dialog::Table(d) => d.name(),
            Dialog::PasteSpecial { .. } => "pasteSpecial",
            Dialog::About { .. } => "about",
            Dialog::Accessibility { .. } => "accessibility",
            Dialog::CustomizeKeyboard { .. } => "customizeKeyboard",
            Dialog::BanglaConverter { .. } => "banglaConverter",
            Dialog::CheckUpdates { .. } => "checkUpdates",
            Dialog::SaveChanges { .. } => "saveChanges",
            Dialog::RecipientList { .. } => "recipientList",
            Dialog::InsertMergeField { .. } => "insertMergeField",
            Dialog::FindRecipient { .. } => "findRecipient",
            Dialog::MergeRule { rule, .. } if rule == "SKIPIF" => "ruleSkipIf",
            Dialog::MergeRule { .. } => "ruleIf",
            Dialog::MatchFields { .. } => "matchFields",
            Dialog::CheckErrors { .. } => "checkErrors",
            Dialog::TableProperties { .. } => "tableProperties",
            Dialog::CellMargins { .. } => "cellMargins",
            Dialog::Password { .. } => "password",
            Dialog::EncryptPassword { .. } => "encryptPassword",
            Dialog::DefineList { .. } => "defineList",
            Dialog::TrackOptions { .. } => "trackChangesOptions",
        }
    }

    /// The dialog that shows a report command's result: Match Fields, Check for Errors.
    pub fn report(id: &str, v: &Value) -> Option<Dialog> {
        let strings = |k: &str| -> Vec<String> {
            v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
        };
        match id {
            "mailings.matchFields" => {
                Some(Dialog::MatchFields { fields: strings("fields"), address: v.get("address").cloned().unwrap_or(Value::Null) })
            }
            "mailings.checkErrors" => {
                Some(Dialog::CheckErrors { unknown: strings("unknownFields"), records: v.get("records").and_then(Value::as_u64).unwrap_or(0) })
            }
            _ => None,
        }
    }

    pub fn open(name: &str, app: &mut WordApp) -> Option<Dialog> {
        if let Some(d) = crate::dialogs_insert::open(name, app) {
            return Some(Dialog::Insert(Box::new(d)));
        }
        if let Some(d) = crate::dialogs_table::open(name, app) {
            return Some(Dialog::Table(Box::new(d)));
        }
        let st = app.session.run("format.state", &json!({})).unwrap_or_default();
        let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
        Some(match name {
            "font" => Dialog::Font {
                font: s("font"),
                size: st.get("size").and_then(Value::as_f64).map(|v| v.to_string()).unwrap_or_default(),
                bold: b("bold"),
                italic: b("italic"),
                underline_style: if b("underline") { "single".into() } else { "none".into() },
                strike: b("strike"),
                double_strike: b("doubleStrike"),
                sup: b("superscript"),
                sub: b("subscript"),
                small_caps: false,
                caps: false,
                hidden: false,
                color: s("color"),
                spacing: 0.0,
                position: 0.0,
            },
            "paragraph" | "asianTypography" => {
                let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
                let rp = rp?;
                let line = match rp.line_spacing {
                    wordcraft_doc::props::LineSpacing::Multiple(m) => m,
                    _ => 1.0,
                };
                let (vl, vr) = rp.visual_indents();
                Dialog::Paragraph {
                    rtl: rp.bidi,
                    align: format!("{:?}", rp.align.visual(rp.bidi)).to_lowercase(),
                    left: vl / 72.0,
                    right: vr / 72.0,
                    first: rp.indent_first / 72.0,
                    before: rp.space_before,
                    after: rp.space_after,
                    line,
                    keep_next: rp.keep_next,
                    keep_lines: rp.keep_lines,
                    page_break: rp.page_break_before,
                    widow: rp.widow_control,
                    tab: if name == "asianTypography" { 2 } else { 0 },
                    asian: [rp.kinsoku, rp.word_wrap, rp.overflow_punct, rp.top_line_punct, rp.auto_space_de, rp.auto_space_dn],
                    asian_was: [rp.kinsoku, rp.word_wrap, rp.overflow_punct, rp.top_line_punct, rp.auto_space_de, rp.auto_space_dn],
                }
            }
            "tabs" => Dialog::Tabs { form: Box::new(crate::dialogs_para::TabsForm::read(app)?) },
            "borders" | "pageBorders" | "shading" => {
                let tab = match name {
                    "pageBorders" => 1,
                    "shading" => 2,
                    _ => 0,
                };
                Dialog::Borders { form: Box::new(crate::dialogs_para::BordersForm::read(app, tab)?) }
            }
            "find" | "replace" => Dialog::Find {
                query: if app.session.sel.is_collapsed() { app.session.find.query.clone() } else { app.session.selected_text() },
                replace: app.session.find.replace.clone(),
                match_case: app.session.find.match_case,
                whole_word: app.session.find.whole_word,
                regex: app.session.find.regex,
                extra: FindExtra {
                    wildcards: app.session.find.wildcards,
                    prefix: app.session.find.match_prefix,
                    suffix: app.session.find.match_suffix,
                    ignore_punct: app.session.find.ignore_punct,
                    ignore_space: app.session.find.ignore_space,
                    more: app.session.find.wildcards
                        || app.session.find.match_prefix
                        || app.session.find.match_suffix
                        || app.session.find.ignore_punct
                        || app.session.find.ignore_space,
                    to_replace: false,
                },
                replace_mode: name == "replace",
                message: String::new(),
            },
            "goto" => Dialog::Goto { page: String::new() },
            "insertTable" => Dialog::InsertTable { rows: 2, cols: 5 },
            "pageSetup" => {
                let sp = wordcraft_engine::cmd::page::sect(&app.session);
                Dialog::PageSetup {
                    top: sp.margin_top / 72.0,
                    bottom: sp.margin_bottom / 72.0,
                    left: sp.margin_left / 72.0,
                    right: sp.margin_right / 72.0,
                    landscape: sp.landscape,
                }
            }
            "link" => Dialog::Link { url: "https://".into(), text: app.session.selected_text() },
            "bookmark" => Dialog::Bookmark { name: String::new() },
            "wordCount" => Dialog::WordCount { stats: app.session.run("review.wordCount", &json!({})).unwrap_or_default() },
            "zoom" => Dialog::Zoom { percent: (app.session.view.zoom * 100.0).round() },
            "watermark" => Dialog::Watermark { text: "CONFIDENTIAL".into(), diagonal: true },
            "newStyle" => Dialog::NewStyle { name: "Style1".into(), based_on: "Normal".into(), back: false },
            "manageStyles" => Dialog::ManageStyles { alphabetical: false, selected: s("style") },
            "newTableStyle" => {
                let based_on = current_table_style(app).unwrap_or_else(|| "TableGrid".into());
                let styles = &app.session.doc.styles;
                let name = (1..1000).map(|i| format!("Table Style {i}")).find(|n| styles.find(n).is_none()).unwrap_or_default();
                let regions = table_regions(app, &based_on);
                Dialog::TableStyle { id: None, name, based_on, region: 0, basis: regions.clone(), regions }
            }
            "modifyTableStyle" => {
                let id = current_table_style(app)?;
                let st = app.session.doc.styles.get(&id)?;
                let (name, based_on) = (st.name.clone(), st.based_on.clone().unwrap_or_default());
                let regions = table_regions(app, &id);
                Dialog::TableStyle { id: Some(id), name, based_on, region: 0, basis: regions.clone(), regions }
            }
            "commands" => Dialog::Commands { query: String::new() },
            "tableProperties" => {
                let form = Box::new(TableForm::read(app)?);
                Dialog::TableProperties { basis: form.clone(), form }
            }
            "cellMargins" => {
                let s = &app.session;
                let (tp, _, _) = s.sel.focus.path.cell()?;
                let t = s.doc.table(s.sel.focus.story, &tp)?;
                let k = wordcraft_geom::Unit::default().pt_per_unit();
                let m = t.props.cell_margins.unwrap_or([0.0, 5.4, 0.0, 5.4]);
                Dialog::CellMargins { top: m[0] / k, left: m[1] / k, bottom: m[2] / k, right: m[3] / k }
            }
            "pasteSpecial" => Dialog::paste_special(app, &json!({})),
            "about" => Dialog::About { tab: 0 },
            "contributors" => Dialog::About { tab: 1 },
            "models" => Dialog::About { tab: 2 },
            "accessibility" => {
                let r = app.session.run("file.accessibility", &json!({})).unwrap_or_default();
                let issues = r.get("issues").and_then(Value::as_array).cloned().unwrap_or_default();
                Dialog::Accessibility { issues }
            }
            "customizeKeyboard" => Dialog::CustomizeKeyboard {
                category: "All Commands".into(),
                query: String::new(),
                selected_cmd: String::new(),
                new_key: String::new(),
            },
            "banglaConverter" => Dialog::BanglaConverter {
                source_text: if app.session.sel.is_collapsed() { String::new() } else { app.session.selected_text() },
                result_text: String::new(),
            },
            "checkUpdates" => Dialog::CheckUpdates {
                checking: false,
                latest_version: env!("CARGO_PKG_VERSION").to_string(),
                release_notes: format!("You are currently running WordCraft v{} (Latest Stable Release).", env!("CARGO_PKG_VERSION")),
                download_url: "https://github.com/rbr48/wordcraft/releases/latest".into(),
                status_msg: tl!("Checking for updates from GitHub...").to_string(),
            },
            // The current list to edit, or a new one to type.
            "recipientList" | "newRecipientList" => {
                let m = &app.session.merge;
                if name == "recipientList" && !m.headers.is_empty() {
                    Dialog::RecipientList { fields: m.headers.clone(), rows: m.rows.clone(), message: String::new() }
                } else {
                    let fields: Vec<String> = NEW_LIST_FIELDS.iter().map(|f| f.to_string()).collect();
                    let rows = vec![vec![String::new(); fields.len()]];
                    Dialog::RecipientList { fields, rows, message: String::new() }
                }
            }
            "insertMergeField" => {
                let fields = app.session.merge.headers.clone();
                Dialog::InsertMergeField { field: fields.first().cloned().unwrap_or_default(), fields }
            }
            "findRecipient" => Dialog::FindRecipient { text: String::new(), message: String::new() },
            "encryptPassword" => Dialog::EncryptPassword { password: Password::default(), confirm: Password::default(), message: String::new() },
            "ruleIf" | "ruleSkipIf" => {
                let fields = app.session.merge.headers.clone();
                Dialog::MergeRule {
                    rule: if name == "ruleSkipIf" { "SKIPIF" } else { "IF" }.into(),
                    field: fields.first().cloned().unwrap_or_default(),
                    fields,
                    value: String::new(),
                    then: String::new(),
                    els: String::new(),
                }
            }
            "matchFields" | "checkErrors" => {
                let id = if name == "matchFields" { "mailings.matchFields" } else { "mailings.checkErrors" };
                return Self::report(id, &app.session.run(id, &json!({})).unwrap_or_default());
            }
            "defineList" => Dialog::DefineList { form: Box::new(crate::dialogs_lists::ListForm::read(app)) },
            "trackChangesOptions" => Dialog::TrackOptions { form: Box::new(crate::dialogs_lists::TrackForm::read(app)) },
            _ => return None,
        })
    }

    /// Paste Special for a clipboard payload (`text`, `html`, `rtf`; empty = WordCraft's own copy).
    pub fn paste_special(app: &WordApp, payload: &Value) -> Dialog {
        let formats: Vec<String> = wordcraft_engine::cmd::paste::available(&app.session, payload).into_iter().map(str::to_string).collect();
        let mut kept = json!({});
        for k in ["text", "html", "rtf"] {
            if let Some(t) = payload.get(k).and_then(Value::as_str) {
                kept[k] = json!(t);
            }
        }
        Dialog::PasteSpecial { formats, choice: 0, payload: kept }
    }

    pub fn modify_style(app: &WordApp, id: &str) -> Option<Dialog> {
        let st = app.session.doc.styles.get(id)?;
        let rc = app.session.doc.styles.resolve_char(
            if st.kind == wordcraft_doc::StyleKind::Paragraph { Some(id) } else { None },
            &wordcraft_doc::CharProps {
                style: if st.kind == wordcraft_doc::StyleKind::Character { Some(id.into()) } else { None },
                ..Default::default()
            },
        );
        let rp = app.session.doc.styles.resolve_para(&wordcraft_doc::ParaProps { style: Some(id.into()), ..Default::default() });
        Some(Dialog::ModifyStyle {
            back: false,
            id: id.into(),
            name: st.name.clone(),
            font: rc.font,
            size: rc.size,
            bold: rc.bold,
            italic: rc.italic,
            color: match rc.color {
                wordcraft_doc::TextColor::Rgb(c) => c.hex(),
                wordcraft_doc::TextColor::Auto => "auto".into(),
            },
            before: rp.space_before,
            after: rp.space_after,
        })
    }
}

/// Grid picker for Insert › Table (hover to size, click to insert).
pub fn table_grid_picker(ui: &mut Ui, app: &mut WordApp) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new("table_picker_hover");
    let hover: (usize, usize) = ui.data(|d| d.get_temp(id)).unwrap_or((0, 0));
    ui.label(if hover.0 > 0 {
        crate::i18n::fmt(tl!("{cols}×{rows} Table"), &[("cols", &hover.1.to_string()), ("rows", &hover.0.to_string())])
    } else {
        tl!("Insert Table").to_string()
    });
    let mut new_hover = (0, 0);
    let mut clicked = None;
    egui::Grid::new("tgp").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for r in 1..=8 {
            for c in 1..=10 {
                let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                let on = r <= hover.0 && c <= hover.1;
                ui.painter().rect(
                    rect,
                    1.0,
                    if on { t.checked } else { t.input },
                    egui::Stroke::new(1.0, if on { t.accent } else { t.border_strong }),
                    egui::StrokeKind::Inside,
                );
                if resp.hovered() {
                    new_hover = (r, c);
                }
                if resp.clicked() {
                    clicked = Some((r, c));
                }
            }
            ui.end_row();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, new_hover));
    if let Some((r, c)) = clicked {
        let _ = app.run("insert.table", json!({"rows": r, "cols": c}));
        ui.close();
    }
}

pub fn show(app: &mut WordApp, ctx: &egui::Context) {
    let Some(mut d) = app.dialog.take() else { return };
    let mut open = true;
    let mut close = false;
    let title = match &d {
        Dialog::Font { .. } => "Font",
        Dialog::Paragraph { .. } => "Paragraph",
        Dialog::Tabs { .. } => "Tabs",
        Dialog::Borders { .. } => "Borders and Shading",
        Dialog::Find { replace_mode: false, .. } => "Find",
        Dialog::Find { .. } => "Find and Replace",
        Dialog::Goto { .. } => "Go To",
        Dialog::InsertTable { .. } => "Insert Table",
        Dialog::PageSetup { .. } => "Page Setup",
        Dialog::Link { .. } => "Insert Hyperlink",
        Dialog::Bookmark { .. } => "Bookmark",
        Dialog::WordCount { .. } => "Word Count",
        Dialog::Zoom { .. } => "Zoom",
        Dialog::Watermark { .. } => "Custom Watermark",
        Dialog::NewStyle { .. } => "Create New Style",
        Dialog::ModifyStyle { .. } => "Modify Style",
        Dialog::ManageStyles { .. } => "Manage Styles",
        Dialog::TableStyle { id: None, .. } => "New Table Style",
        Dialog::TableStyle { .. } => "Modify Table Style",
        Dialog::Commands { .. } => "Search Commands",
        Dialog::Insert(d) => d.title(),
        Dialog::Table(d) => d.title(),
        Dialog::PasteSpecial { .. } => "Paste Special",
        Dialog::About { .. } => "About WordCraft",
        Dialog::Accessibility { .. } => "Accessibility Checker",
        Dialog::CustomizeKeyboard { .. } => "Customize Keyboard",
        Dialog::BanglaConverter { .. } => "Bangla Converter",
        Dialog::CheckUpdates { .. } => "Check for Updates",
        Dialog::SaveChanges { .. } => "WordCraft",
        Dialog::RecipientList { .. } => "Recipient List",
        Dialog::InsertMergeField { .. } => "Insert Merge Field",
        Dialog::FindRecipient { .. } => "Find Recipient",
        Dialog::MergeRule { rule, .. } if rule == "SKIPIF" => "Skip Record If",
        Dialog::MergeRule { .. } => "If…Then…Else",
        Dialog::MatchFields { .. } => "Match Fields",
        Dialog::CheckErrors { .. } => "Check for Errors",
        Dialog::TableProperties { .. } => "Table Properties",
        Dialog::CellMargins { .. } => "Cell Margins",
        Dialog::Password { .. } => "Password",
        Dialog::EncryptPassword { .. } => "Encrypt with Password",
        Dialog::DefineList { .. } => "Define New Multilevel List",
        Dialog::TrackOptions { .. } => "Track Changes Options",
    };
    egui::Window::new(tl!(title))
        .id(egui::Id::new(("dialog", title)))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            close = body(app, ui, &mut d);
        });
    if open && !close {
        app.dialog = Some(d);
    } else {
        app.canvas.want_focus = true;
    }
}

pub(crate) fn buttons(ui: &mut Ui, ok: &str) -> (bool, bool) {
    let mut r = (false, false);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                r.1 = true;
            }
            let okb = ui.add(egui::Button::new(egui::RichText::new(ok).color(egui::Color32::WHITE)).fill(crate::theme::APP_COLOR));
            if okb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                r.0 = true;
            }
        });
    });
    r
}

/// Returns true to close.
fn body(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    match d {
        Dialog::Font {
            font,
            size,
            bold,
            italic,
            underline_style,
            strike,
            double_strike,
            sup,
            sub,
            small_caps,
            caps,
            hidden,
            color,
            spacing,
            position,
        } => {
            // Build the sample text format for the preview pane below.
            let preview_size = size.trim().parse::<f32>().unwrap_or(12.0).clamp(6.0, 72.0);
            let preview_color = wordcraft_doc::Rgb::parse(color).map(crate::theme::c32).unwrap_or_else(|| ui.visuals().text_color());
            let mut fmt = egui::TextFormat {
                font_id: egui::FontId::new(preview_size, egui::FontFamily::Proportional),
                color: preview_color,
                ..Default::default()
            };
            if *bold {
                fmt.font_id = crate::theme::semibold(preview_size);
            }
            if *italic {
                fmt.italics = true;
            }
            if underline_style != "none" {
                fmt.underline = egui::Stroke::new(1.0, preview_color);
            }
            if *strike || *double_strike {
                fmt.strikethrough = egui::Stroke::new(1.0, preview_color);
            }
            egui::Grid::new("fontdlg").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.text_edit_singleline(size);
                ui.end_row();
                ui.label(tl!("Font color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Underline style:"));
                egui::ComboBox::from_id_salt("u_style").selected_text(underline_style.as_str()).show_ui(ui, |ui| {
                    for u in ["none", "single", "double", "thick", "dotted", "dash", "wave"] {
                        ui.selectable_value(underline_style, u.to_string(), u);
                    }
                });
                ui.end_row();
                ui.label(tl!("Spacing (pt):"));
                ui.add(egui::DragValue::new(spacing).speed(0.1).range(-20.0..=20.0));
                ui.end_row();
                ui.label(tl!("Position (pt):"));
                ui.add(egui::DragValue::new(position).speed(0.1).range(-100.0..=100.0));
                ui.end_row();
            });
            ui.separator();
            ui.label(egui::RichText::new(tl!("Effects")).font(semibold(12.5)));
            egui::Grid::new("fx").num_columns(2).show(ui, |ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
                ui.end_row();
                ui.checkbox(strike, tl!("Strikethrough"));
                ui.checkbox(double_strike, tl!("Double strikethrough"));
                ui.end_row();
                ui.checkbox(sup, tl!("Superscript"));
                ui.checkbox(sub, tl!("Subscript"));
                ui.end_row();
                ui.checkbox(small_caps, tl!("Small caps"));
                ui.checkbox(caps, tl!("All caps"));
                ui.end_row();
                ui.checkbox(hidden, tl!("Hidden"));
                ui.end_row();
            });
            // Live preview of the chosen font/size/colour/effects.
            ui.separator();
            ui.label(egui::RichText::new(tl!("Preview")).small().weak());
            let sample = if *caps { tl!("AaBbYyZz").to_uppercase() } else { tl!("AaBbYyZz").to_string() };
            let mut job = egui::text::LayoutJob::single_section(sample, fmt);
            job.wrap.max_width = ui.available_width().min(400.0);
            ui.label(job);
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let mut props = json!({
                    "bold": *bold, "italic": *italic, "strike": *strike, "doubleStrike": *double_strike,
                    "underline": underline_style.as_str(),
                    "vertAlign": if *sup { "superscript" } else if *sub { "subscript" } else { "baseline" },
                    "smallCaps": *small_caps, "caps": *caps, "hidden": *hidden,
                    "spacing": *spacing, "position": *position,
                });
                if !font.trim().is_empty() {
                    props["font"] = json!(font.trim());
                }
                if let Ok(s) = size.trim().parse::<f64>() {
                    props["size"] = json!(s);
                }
                if let Some(c) = wordcraft_doc::Rgb::parse(color) {
                    props["color"] = json!({"Rgb": [c.0, c.1, c.2]});
                }
                let _ = app.run("format.set", json!({"props": props}));
            }
            ok || cancel
        }
        Dialog::Paragraph {
            rtl,
            align,
            left,
            right,
            first,
            before,
            after,
            line,
            keep_next,
            keep_lines,
            page_break,
            widow,
            tab,
            asian,
            asian_was,
        } => {
            ui.horizontal(|ui| {
                for (k, name) in ["Indents and Spacing", "Line and Page Breaks", "Asian Typography"].into_iter().enumerate() {
                    if ui.selectable_label(*tab as usize == k, tl!(name)).clicked() {
                        *tab = k as u8;
                    }
                }
            });
            ui.separator();
            match *tab {
                0 => {
                    ui.label(egui::RichText::new(tl!("General")).font(semibold(12.5)));
                    ui.horizontal(|ui| {
                        ui.label(tl!("Direction:"));
                        ui.radio_value(rtl, true, tl!("Right-to-left"));
                        ui.radio_value(rtl, false, tl!("Left-to-right"));
                    });
                    const ALIGNS: [(&str, &str); 4] = [("left", "Left"), ("center", "Center"), ("right", "Right"), ("justify", "Justify")];
                    ui.horizontal(|ui| {
                        ui.label(tl!("Alignment:"));
                        let shown = ALIGNS.iter().find(|(v, _)| v == align).map_or(align.as_str(), |(_, l)| tl!(l));
                        egui::ComboBox::from_id_salt("para_align").selected_text(shown).show_ui(ui, |ui| {
                            for (v, l) in ALIGNS {
                                ui.selectable_value(align, v.to_string(), tl!(l));
                            }
                        });
                    });
                    ui.label(egui::RichText::new(tl!("Indentation")).font(semibold(12.5)));
                    egui::Grid::new("ind").num_columns(4).show(ui, |ui| {
                        ui.label(tl!("Left:"));
                        ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                        ui.label(tl!("Right:"));
                        ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                        ui.end_row();
                        ui.label(tl!("First line:"));
                        ui.add(egui::DragValue::new(first).speed(0.05).suffix("\"").max_decimals(2));
                        ui.end_row();
                    });
                    ui.label(egui::RichText::new(tl!("Spacing")).font(semibold(12.5)));
                    egui::Grid::new("sp").num_columns(4).show(ui, |ui| {
                        ui.label(tl!("Before:"));
                        ui.add(egui::DragValue::new(before).speed(1.0).range(0.0..=1584.0).suffix(format!(" {}", tl!("pt"))));
                        ui.label(tl!("Line spacing:"));
                        ui.add(egui::DragValue::new(line).speed(0.05).range(0.5..=5.0));
                        ui.end_row();
                        ui.label(tl!("After:"));
                        ui.add(egui::DragValue::new(after).speed(1.0).range(0.0..=1584.0).suffix(format!(" {}", tl!("pt"))));
                        ui.end_row();
                    });
                }
                1 => {
                    ui.label(egui::RichText::new(tl!("Line and Page Breaks")).font(semibold(12.5)));
                    ui.checkbox(widow, tl!("Widow/Orphan control"));
                    ui.checkbox(keep_next, tl!("Keep with next"));
                    ui.checkbox(keep_lines, tl!("Keep lines together"));
                    ui.checkbox(page_break, tl!("Page break before"));
                }
                _ => {
                    let [kinsoku, word_wrap, overflow, top_line, de, dn] = asian;
                    ui.label(egui::RichText::new(tl!("Line breaking")).font(semibold(12.5)));
                    ui.checkbox(kinsoku, tl!("Apply Asian line-breaking rules (kinsoku)"));
                    // The model's flag is "wrap whole words"; the box offers the opposite.
                    let mut mid_word = !*word_wrap;
                    if ui.checkbox(&mut mid_word, tl!("Allow Latin text to wrap in the middle of a word")).changed() {
                        *word_wrap = !mid_word;
                    }
                    ui.checkbox(overflow, tl!("Allow hanging punctuation"));
                    ui.label(egui::RichText::new(tl!("Character Spacing")).font(semibold(12.5)));
                    ui.checkbox(top_line, tl!("Compress punctuation at the start of a line"));
                    ui.checkbox(de, tl!("Add space between Asian and Latin text"));
                    ui.checkbox(dn, tl!("Add space between Asian text and numbers"));
                }
            }
            ui.add_space(6.0);
            let tabs = ui.button(tl!("Tabs…")).clicked();
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok || tabs {
                apply_paragraph(app, *rtl, align, *left, *right, *first, *before, *after, *line, [*keep_next, *keep_lines, *page_break, *widow]);
                apply_asian(app, *asian, *asian_was);
            }
            // Tabs… keeps what was set here and moves on to the Tabs dialog.
            if tabs {
                app.dialog = Dialog::open("tabs", app);
            }
            ok || cancel || tabs
        }
        Dialog::Tabs { form } => crate::dialogs_para::tabs(app, ui, form),
        Dialog::Borders { form } => crate::dialogs_para::borders(app, ui, form),
        Dialog::Find { query, replace, match_case, whole_word, regex, extra, replace_mode, message } => {
            ui.horizontal(|ui| {
                if ui.selectable_label(!*replace_mode, tl!("Find")).clicked() {
                    *replace_mode = false;
                }
                if ui.selectable_label(*replace_mode, tl!("Replace")).clicked() {
                    *replace_mode = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Find what:"));
                if ui.add(egui::TextEdit::singleline(query).desired_width(240.0)).gained_focus() {
                    extra.to_replace = false;
                }
            });
            if *replace_mode {
                ui.horizontal(|ui| {
                    ui.label(tl!("Replace with:"));
                    if ui.add(egui::TextEdit::singleline(replace).desired_width(228.0)).gained_focus() {
                        extra.to_replace = true;
                    }
                });
            } else {
                extra.to_replace = false;
            }
            // Word greys out Match case and the word options with wildcards (they always match case).
            let plain = !extra.wildcards;
            ui.horizontal(|ui| {
                ui.add_enabled(plain, egui::Checkbox::new(match_case, tl!("Match case")));
                ui.add_enabled(plain, egui::Checkbox::new(whole_word, tl!("Whole words")));
                ui.checkbox(&mut extra.wildcards, tl!("Use wildcards"));
            });
            let arrow = if extra.more { "▾" } else { "▸" };
            if ui.selectable_label(extra.more, format!("{arrow} {}", tl!("Search Options"))).clicked() {
                extra.more = !extra.more;
            }
            if extra.more {
                ui.horizontal(|ui| {
                    ui.add_enabled(plain, egui::Checkbox::new(&mut extra.prefix, tl!("Match prefix")));
                    ui.add_enabled(plain, egui::Checkbox::new(&mut extra.suffix, tl!("Match suffix")));
                });
                ui.horizontal(|ui| {
                    ui.add_enabled(plain, egui::Checkbox::new(&mut extra.ignore_punct, tl!("Ignore punctuation characters")));
                    ui.add_enabled(plain, egui::Checkbox::new(&mut extra.ignore_space, tl!("Ignore white-space characters")));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(regex, tl!("Regular expressions"));
                    if let Some(code) = special_menu(ui, extra) {
                        if extra.to_replace { replace.push_str(code) } else { query.push_str(code) }
                    }
                });
            }
            let opts = json!({
                "text": query,
                "with": replace,
                "matchCase": *match_case,
                "wholeWord": *whole_word,
                "regex": *regex,
                "wildcards": extra.wildcards,
                "matchPrefix": extra.prefix,
                "matchSuffix": extra.suffix,
                "ignorePunct": extra.ignore_punct,
                "ignoreSpace": extra.ignore_space,
            });
            if !*replace_mode {
                ui.horizontal(|ui| {
                    let lit = app.session.find.highlight;
                    if ui.selectable_label(lit, tl!("Reading Highlight")).clicked() {
                        let mut v = opts.clone();
                        v["highlight"] = json!(!lit);
                        match app.run("edit.advancedFind", v) {
                            Ok(r) if !lit => *message = crate::i18n::fmt(tl!("{count} items highlighted"), &[("count", &r["count"].to_string())]),
                            Ok(_) => message.clear(),
                            Err(e) => *message = e,
                        }
                    }
                });
            }
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).weak());
            }
            let mut close = false;
            ui.horizontal(|ui| {
                if *replace_mode {
                    if ui.button(tl!("Replace All")).clicked() {
                        match app.run("edit.replaceAll", opts.clone()) {
                            Ok(r) => *message = format!("All done. We made {} replacements.", r["replaced"]),
                            Err(e) => *message = e,
                        }
                    }
                    if ui.button(tl!("Replace")).clicked() {
                        let _ = app.run("edit.replace", opts.clone());
                    }
                }
                if ui.button(tl!("Find Next")).clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)) {
                    let _ = app.session.run("edit.find", &opts);
                    match app.run("edit.findNext", json!({})) {
                        Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                        Err(e) => *message = e,
                    }
                }
                if ui.button(tl!("Find Previous")).clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.shift)) {
                    let _ = app.session.run("edit.find", &opts);
                    match app.run("edit.findPrevious", json!({})) {
                        Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                        Err(e) => *message = e,
                    }
                }
                if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
            close
        }
        Dialog::Goto { page } => {
            let mut submit = false;
            ui.horizontal(|ui| {
                ui.label(tl!("Enter page number or section (e.g. 3, s2):"));
                let resp = ui.text_edit_singleline(page);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
            let (ok, cancel) = buttons(ui, tl!("Go To"));
            if ok || submit {
                let trimmed = page.trim();
                if let Some(sec) = trimmed.strip_prefix('s').or_else(|| trimmed.strip_prefix('S')) {
                    if let Ok(n) = sec.parse::<u64>() {
                        let _ = app.run("edit.goto", json!({"section": n}));
                    }
                } else if let Ok(n) = trimmed.parse::<u64>() {
                    let _ = app.run("edit.goto", json!({"page": n}));
                } else if !trimmed.is_empty() {
                    let _ = app.run("edit.goto", json!({"bookmark": trimmed}));
                }
                return true;
            }
            cancel
        }
        Dialog::TableProperties { form, basis } => {
            table_properties(ui, form);
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let changes = form.changes(basis);
                if changes.as_object().is_some_and(|m| !m.is_empty()) {
                    let _ = app.run("table.properties", changes);
                }
            }
            ok || cancel
        }
        Dialog::CellMargins { top, left, bottom, right } => {
            let unit = wordcraft_geom::Unit::default();
            let k = unit.pt_per_unit();
            ui.label(egui::RichText::new(tl!("Default cell margins")).font(semibold(12.5)));
            ui.add_space(4.0);
            egui::Grid::new("cm_grid").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Top:"));
                ui.add(egui::DragValue::new(top).speed(0.01).range(0.0..=2.0).suffix(unit.suffix()).max_decimals(2));
                ui.end_row();
                ui.label(tl!("Bottom:"));
                ui.add(egui::DragValue::new(bottom).speed(0.01).range(0.0..=2.0).suffix(unit.suffix()).max_decimals(2));
                ui.end_row();
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.01).range(0.0..=2.0).suffix(unit.suffix()).max_decimals(2));
                ui.end_row();
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.01).range(0.0..=2.0).suffix(unit.suffix()).max_decimals(2));
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run(
                    "table.cellMargins",
                    json!({
                        "top": *top * k,
                        "bottom": *bottom * k,
                        "left": *left * k,
                        "right": *right * k,
                    }),
                );
            }
            ok || cancel
        }
        Dialog::InsertTable { rows, cols } => {
            egui::Grid::new("it").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Number of columns:"));
                ui.add(egui::DragValue::new(cols).range(1..=63));
                ui.end_row();
                ui.label(tl!("Number of rows:"));
                ui.add(egui::DragValue::new(rows).range(1..=1000));
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("insert.table", json!({"rows": *rows, "cols": *cols}));
            }
            ok || cancel
        }
        Dialog::PageSetup { top, bottom, left, right, landscape } => {
            ui.label(egui::RichText::new(tl!("Margins")).font(semibold(12.5)));
            egui::Grid::new("ps").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Top:"));
                ui.add(egui::DragValue::new(top).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Bottom:"));
                ui.add(egui::DragValue::new(bottom).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Orientation")).font(semibold(12.5)));
            ui.horizontal(|ui| {
                ui.radio_value(landscape, false, tl!("Portrait"));
                ui.radio_value(landscape, true, tl!("Landscape"));
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("layout.orientation", json!({"value": if *landscape { "landscape" } else { "portrait" }}));
                let _ =
                    app.run("layout.margins", json!({"top": *top * 72.0, "bottom": *bottom * 72.0, "left": *left * 72.0, "right": *right * 72.0}));
            }
            ok || cancel
        }
        Dialog::Link { url, text } => {
            egui::Grid::new("lnk").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Text to display:"));
                ui.text_edit_singleline(text);
                ui.end_row();
                ui.label(tl!("Address:"));
                ui.text_edit_singleline(url);
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("insert.link", json!({"url": url, "text": text}));
            }
            ok || cancel
        }
        Dialog::Bookmark { name } => {
            ui.label(tl!("Bookmark name:"));
            ui.text_edit_singleline(name);
            let marks = app.session.doc.bookmarks();
            for (n, _) in &marks {
                if ui.selectable_label(false, n).clicked() {
                    let _ = app.run("edit.goto", json!({"bookmark": n}));
                }
            }
            let (ok, cancel) = buttons(ui, tl!("Add"));
            if ok {
                let _ = app.run("insert.bookmark", json!({"name": name}));
            }
            ok || cancel
        }
        Dialog::WordCount { stats } => {
            egui::Grid::new("wc").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                for (l, k) in [
                    ("Pages", "pages"),
                    ("Words", "words"),
                    ("Characters (no spaces)", "characters"),
                    ("Characters (with spaces)", "charactersWithSpaces"),
                    ("Paragraphs", "paragraphs"),
                    ("Lines", "lines"),
                ] {
                    ui.label(tl!(l));
                    ui.label(stats.get(k).map(|v| v.to_string()).unwrap_or_default());
                    ui.end_row();
                }
            });
            let mut include = stats.get("includeTextBoxes").and_then(Value::as_bool).unwrap_or(true);
            if ui.checkbox(&mut include, tl!("Count words in text boxes and notes")).changed()
                && let Ok(v) = app.run("review.wordCount", json!({"includeTextBoxes": include}))
            {
                *stats = v;
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::Zoom { percent } => {
            ui.horizontal(|ui| {
                for p in [200.0, 100.0, 75.0] {
                    ui.radio_value(percent, p, format!("{p}%"));
                }
            });
            // Logarithmic like the status bar slider, and wide enough to aim: the default 100 pt
            // linear slider jumped from 100% to 480% within a short drag (issue #67).
            ui.spacing_mut().slider_width = 220.0;
            ui.add(egui::Slider::new(percent, 10.0..=500.0).logarithmic(true).step_by(1.0).suffix("%"));
            let mut fit = None;
            ui.horizontal(|ui| {
                if ui.button(tl!("Page width")).clicked() {
                    fit = Some("pageWidth");
                }
                if ui.button(tl!("Whole page")).clicked() {
                    fit = Some("onePage");
                }
                if ui.button(tl!("Many pages")).clicked() {
                    fit = Some("multiplePages");
                }
            });
            if let Some(f) = fit {
                let _ = app.run("view.zoom", json!({"value": f}));
                return true;
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("view.zoom", json!({"value": *percent}));
            }
            ok || cancel
        }
        Dialog::Watermark { text, diagonal } => {
            ui.horizontal(|ui| {
                ui.label(tl!("Text:"));
                ui.text_edit_singleline(text);
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Presets:"));
                for p in ["CONFIDENTIAL", "DO NOT COPY", "DRAFT", "URGENT", "SAMPLE"] {
                    if ui.button(p).clicked() {
                        *text = p.to_string();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.radio_value(diagonal, true, tl!("Diagonal"));
                ui.radio_value(diagonal, false, tl!("Horizontal"));
            });
            let mut remove = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Remove Watermark")).clicked() {
                    remove = true;
                }
            });
            if remove {
                let _ = app.run("design.watermark", json!({"remove": true}));
                return true;
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("design.watermark", json!({"text": text, "diagonal": *diagonal}));
            }
            ok || cancel
        }
        Dialog::NewStyle { name, based_on, back } => {
            egui::Grid::new("ns").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Style based on:"));
                ui.text_edit_singleline(based_on);
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("The new style takes the formatting of the current paragraph.")).small().weak());
            let (ok, cancel) = buttons(ui, tl!("OK"));
            let mut created = None;
            if ok {
                created =
                    app.run("styles.create", json!({"name": name, "basedOn": based_on})).ok().and_then(|v| v.get("id")?.as_str().map(str::to_string));
            }
            if (ok || cancel) && *back {
                app.dialog = Some(Dialog::ManageStyles { alphabetical: false, selected: created.unwrap_or_default() });
            }
            ok || cancel
        }
        Dialog::ModifyStyle { back, id, name, font, size, bold, italic, color, before, after } => {
            egui::Grid::new("ms").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.add(egui::DragValue::new(size).range(1.0..=1638.0).speed(0.5));
                ui.end_row();
                ui.label(tl!("Color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Space before/after:"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(before).range(0.0..=1584.0));
                    ui.add(egui::DragValue::new(after).range(0.0..=1584.0));
                });
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let mut chr = json!({"font": font, "size": *size, "bold": *bold, "italic": *italic});
                if let Some(c) = wordcraft_doc::Rgb::parse(color) {
                    chr["color"] = json!({"Rgb": [c.0, c.1, c.2]});
                }
                let _ =
                    app.run("styles.modify", json!({"style": id, "name": name, "chr": chr, "para": {"spaceBefore": *before, "spaceAfter": *after}}));
            }
            if (ok || cancel) && *back {
                app.dialog = Some(Dialog::ManageStyles { alphabetical: false, selected: id.clone() });
            }
            ok || cancel
        }
        Dialog::ManageStyles { alphabetical, selected } => manage_styles(app, ui, alphabetical, selected),
        Dialog::TableStyle { id, name, based_on, region, regions, basis } => {
            let styles: Vec<(String, String)> = app
                .session
                .doc
                .styles
                .styles
                .iter()
                .filter(|s| s.kind == wordcraft_doc::StyleKind::Table && Some(&s.id) != id.as_ref())
                .map(|s| (s.id.clone(), s.name.clone()))
                .collect();
            let base_name = styles.iter().find(|(i, _)| i == based_on).map(|(_, n)| n.clone()).unwrap_or_else(|| based_on.clone());
            let mut rebased = false;
            egui::Grid::new("tstyle").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Style based on:"));
                egui::ComboBox::from_id_salt("tstyle_base").width(200.0).selected_text(base_name).show_ui(ui, |ui| {
                    for (sid, sname) in &styles {
                        if ui.selectable_label(sid == based_on, sname).clicked() && sid != based_on {
                            *based_on = sid.clone();
                            rebased = true;
                        }
                    }
                });
                ui.end_row();
                ui.label(tl!("Apply formatting to:"));
                egui::ComboBox::from_id_salt("tstyle_region")
                    .width(200.0)
                    .selected_text(tl!(REGION_LABELS.get(*region).copied().unwrap_or("Whole Table")))
                    .show_ui(ui, |ui| {
                        for (i, n) in REGION_LABELS.iter().enumerate() {
                            ui.selectable_value(region, i, tl!(n));
                        }
                    });
                ui.end_row();
            });
            // A new style shows its new base's formatting; a modified one keeps its own.
            if rebased && id.is_none() {
                *regions = table_regions(app, based_on);
                *basis = regions.clone();
            }
            ui.separator();
            let theme = app.session.doc.settings.theme_colors.clone();
            if let Some(r) = regions.get_mut(*region) {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut r.borders, tl!("Borders"));
                    ui.checkbox(&mut r.bold, tl!("Bold"));
                });
                egui::Grid::new("tstyle_colors").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                    ui.label(tl!("Fill color:"));
                    color_menu(ui, &theme, &mut r.fill);
                    ui.end_row();
                    ui.label(tl!("Font color:"));
                    color_menu(ui, &theme, &mut r.color);
                    ui.end_row();
                });
            }
            // Live preview: the style as OK would leave it, on the current table's options with
            // the region being edited turned on.
            let v = table_style_params(id.as_deref(), name, based_on, regions, basis);
            let mut look = app
                .session
                .sel
                .focus
                .path
                .cell()
                .and_then(|(tp, _, _)| app.session.doc.table(app.session.sel.focus.story, &tp).map(|t| t.props.look))
                .unwrap_or_default();
            match *region {
                1 => look.header_row = true,
                2 => look.total_row = true,
                3 => look.first_column = true,
                4 => look.last_column = true,
                5 => look.banded_rows = true,
                6 => look.banded_columns = true,
                _ => {}
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tl!("Preview")).small().weak());
            table_style_preview(ui, wordcraft_engine::cmd::table_style::preview(&app.session, &v).as_ref(), look);
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let cmd = if id.is_some() { "table.modifyStyle" } else { "table.newStyle" };
                if let Err(e) = app.run(cmd, v) {
                    app.status(e);
                    return false;
                }
            }
            ok || cancel
        }
        Dialog::Insert(f) => crate::dialogs_insert::body(app, ui, f),
        Dialog::Table(f) => crate::dialogs_table::body(app, ui, f),
        Dialog::Commands { query } => {
            let r = ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Type a command, e.g. \"insert table\"")).desired_width(380.0));
            r.request_focus();
            let q = query.to_lowercase();
            let reg = app.session.registry.clone();
            let mut hits: Vec<&wordcraft_engine::CommandSpec> = reg
                .all()
                .iter()
                .filter(|c| {
                    !q.is_empty()
                        && (c.label.to_lowercase().contains(&q)
                            || tl!(c.label).to_lowercase().contains(&q)
                            || c.id.to_lowercase().contains(&q)
                            || c.location.to_lowercase().contains(&q)
                            || crate::i18n::location(c.location).to_lowercase().contains(&q))
                })
                .collect();
            hits.truncate(14);
            let mut close = false;
            for c in hits {
                let sc = crate::widgets::shortcut_text(app, c.id);
                let label = format!("{}   —   {}", tl!(c.label), crate::i18n::location(c.location));
                if ui.add(egui::Button::new(label).shortcut_text(sc).min_size(vec2(380.0, 0.0))).clicked() {
                    let _ = app.run(c.id, json!({}));
                    close = true;
                }
            }
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::PasteSpecial { formats, choice, payload } => {
            ui.label(egui::RichText::new(tl!("Paste as:")).font(semibold(12.5)));
            if formats.is_empty() {
                ui.label(egui::RichText::new(tl!("The clipboard is empty.")).weak());
            }
            for (i, f) in formats.iter().enumerate() {
                let label = wordcraft_engine::cmd::paste::FORMATS.iter().find(|(id, _)| id == f).map(|(_, l)| *l).unwrap_or(f.as_str());
                ui.radio_value(choice, i, tl!(label));
            }
            if let Some(f) = formats.get(*choice) {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(tl!("Result")).font(semibold(12.5)));
                let hint = match f.as_str() {
                    "formatted" => "Keeps the fonts and formatting of the copied text.",
                    "rtf" => "Reads the clipboard's rich text and keeps its formatting.",
                    "html" => "Reads the clipboard's web content and keeps its formatting.",
                    _ => "Drops all formatting and pastes plain text.",
                };
                ui.label(egui::RichText::new(tl!(hint)).weak());
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok && let Some(f) = formats.get(*choice) {
                let mut params = payload.clone();
                params["as"] = json!(f);
                let _ = app.run("edit.pasteSpecial", params);
            }
            ok || cancel
        }
        Dialog::About { tab } => {
            ui.set_width(660.0);
            ui.horizontal(|ui| {
                for (i, l) in ["About", "Contributors", "Models"].into_iter().enumerate() {
                    let i = i as u8;
                    if ui.selectable_label(*tab == i, tl!(l)).clicked() {
                        *tab = i;
                    }
                }
            });
            ui.separator();
            match *tab {
                1 => crate::credits::contributors_ui(ui),
                2 => crate::credits::models_ui(ui),
                _ => {
                    ui.label(egui::RichText::new(tl!("WordCraft")).font(semibold(22.0)));
                    let build = option_env!("WORDCRAFT_BUILD_SHA").map(|s| s.get(..8).unwrap_or(s)).unwrap_or(tl!("development build"));
                    ui.label(crate::i18n::fmt(tl!("Version {version} ({build})"), &[("version", env!("CARGO_PKG_VERSION")), ("build", build)]));
                    ui.label(tl!(
                        "A clean-room, open-source word processor written in native Rust.\nLead Developer & Top Contributor: M S Rana (@rbr48)."
                    ));
                    ui.add_space(6.0);
                    ui.hyperlink_to(tl!("Source code: github.com/rbr48/wordcraft"), "https://github.com/rbr48/wordcraft");
                    ui.add_space(8.0);
                    if ui.button(tl!("Check for Updates...")).clicked() {
                        let _ = app.run("ui.dialog", json!({"name": "checkUpdates"}));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(tl!("MIT OR Apache-2.0. Copyright (c) 2026 M S Rana and WordCraft contributors.")).small().weak());
                }
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::Accessibility { issues } => {
            ui.set_width(480.0);
            ui.label(egui::RichText::new(tl!("Accessibility Checker")).font(semibold(16.0)));
            ui.add_space(6.0);
            if issues.is_empty() {
                ui.label(tl!("No accessibility issues found. People with disabilities should have no difficulty reading this document."));
            } else {
                ui.label(crate::i18n::fmt(tl!("{count} issue(s) found:"), &[("count", &issues.len().to_string())]));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    for it in issues {
                        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("info");
                        let issue = it.get("issue").and_then(Value::as_str).unwrap_or("");
                        let fix = it.get("fix").and_then(Value::as_str).unwrap_or("");
                        let (icon_color, tag) = match kind {
                            "error" => (egui::Color32::from_rgb(0xD1, 0x3B, 0x3B), "Error"),
                            "warning" => (egui::Color32::from_rgb(0xE0, 0x7B, 0x1F), "Warning"),
                            _ => (egui::Color32::from_rgb(0x2B, 0x47, 0xB5), "Tip"),
                        };
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tag).color(icon_color).strong());
                                ui.label(issue);
                            });
                            if !fix.is_empty() {
                                ui.label(egui::RichText::new(format!("How to fix: {fix}")).small().weak());
                            }
                        });
                    }
                });
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::SaveChanges { name, .. } => {
            ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Do you want to save changes to {name}?"), &[("name", name)])).font(semibold(15.0)));
            ui.label(tl!("Your changes will be lost if you don't save them."));
            ui.add_space(8.0);
            let buttons = ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel = ui.button(tl!("Cancel"));
                    let dont_save = ui.button(tl!("Don't Save"));
                    let save = ui.add(egui::Button::new(egui::RichText::new(tl!("Save")).color(egui::Color32::WHITE)).fill(crate::theme::APP_COLOR));
                    [(cancel, "cancel"), (dont_save, "dontSave"), (save, "save")]
                })
                .inner
            });
            let buttons = buttons.inner;
            // The keyboard alone answers, as in Word (#861): Save has focus when the prompt opens,
            // so Enter saves, and the arrow keys (egui's focus navigation) or Tab move between the
            // buttons, shown as Save, Don't Save, Cancel.
            if ui.memory(|m| m.focused().is_none())
                && let Some((save, _)) = buttons.get(2)
            {
                save.request_focus();
            }
            // Enter clicks the focused button; it means Save only when none of them has focus.
            let focused = buttons.iter().any(|(b, _)| b.has_focus());
            let answer = if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                Some("cancel")
            } else if let Some((_, a)) = buttons.iter().find(|(b, _)| b.clicked()) {
                Some(*a)
            } else if !focused && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                Some("save")
            } else {
                None
            };
            let Some(answer) = answer else { return false };
            // `ui.saveChanges` reads the prompt from `app.dialog`, which `show` has taken out.
            app.dialog = Some(d.clone());
            let _ = app.run("ui.saveChanges", json!({"answer": answer}));
            true
        }
        Dialog::RecipientList { fields, rows, message } => recipient_list(app, ui, fields, rows, message),
        Dialog::InsertMergeField { fields, field } => {
            if fields.is_empty() {
                ui.label(egui::RichText::new(tl!("Select recipients first to choose from their fields.")).small().weak());
            } else {
                ui.label(tl!("Fields:"));
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for f in fields.iter() {
                        let resp = ui.selectable_label(field == f, f);
                        if resp.clicked() {
                            field.clone_from(f);
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label(tl!("Field:"));
                ui.text_edit_singleline(field);
            });
            let (ok, cancel) = buttons(ui, tl!("Insert"));
            if ok && !field.trim().is_empty() {
                let _ = app.run("mailings.insertField", json!({"field": field.trim()}));
                return true;
            }
            cancel
        }
        Dialog::Password { name, params, password, message } => {
            ui.label(crate::i18n::fmt(tl!("{name} is protected with a password."), &[("name", name)]));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(tl!("Password:"));
                let r = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
                if password.is_empty() && !r.has_focus() {
                    r.request_focus();
                }
            });
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok && !password.is_empty() {
                let mut p = params.clone();
                if let Some(o) = p.as_object_mut() {
                    o.insert("password".into(), json!(password.as_str()));
                }
                // `execute`, not `run`: Save Changes was asked when the open began.
                match app.execute("file.open", p) {
                    Ok(_) => {
                        app.ui.backstage = false;
                        return true;
                    }
                    Err(e) if wordcraft_engine::io::wrong_password(&e) => {
                        *message = tl!("That password isn't right. Check it (passwords are case-sensitive) and try again.").to_string();
                        password.as_mut_string().clear();
                    }
                    Err(e) => *message = e,
                }
            }
            cancel
        }
        Dialog::EncryptPassword { password, confirm, message } => {
            ui.label(tl!("Anyone who opens the document will need this password."));
            ui.add_space(4.0);
            egui::Grid::new("encrypt_password").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Password:"));
                let r = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
                if password.is_empty() && confirm.is_empty() && !r.has_focus() {
                    r.request_focus();
                }
                ui.end_row();
                ui.label(tl!("Confirm password:"));
                ui.add(egui::TextEdit::singleline(confirm.as_mut_string()).password(true).desired_width(220.0));
                ui.end_row();
            });
            ui.label(
                egui::RichText::new(tl!(
                    "Keep the password somewhere safe: without it nobody, WordCraft included, can open the document. Leave both boxes empty to remove the password."
                ))
                .small()
                .weak(),
            );
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                if password != confirm {
                    *message = tl!("The passwords don't match.").to_string();
                    confirm.as_mut_string().clear();
                    return false;
                }
                let pw = if password.is_empty() { Value::Null } else { json!(password.as_str()) };
                match app.run("file.encrypt", json!({"password": pw})) {
                    Ok(_) => {
                        app.status(if password.is_empty() {
                            tl!("The password is removed. Save the document to keep the change.")
                        } else {
                            tl!("The document will be saved with a password. Save it to protect it.")
                        });
                        return true;
                    }
                    Err(e) => *message = e,
                }
            }
            cancel
        }
        Dialog::DefineList { form } => crate::dialogs_lists::define_list(app, ui, form),
        Dialog::TrackOptions { form } => crate::dialogs_lists::track_options(app, ui, form),
        Dialog::FindRecipient { text, message } => {
            ui.horizontal(|ui| {
                ui.label(tl!("Find:"));
                ui.text_edit_singleline(text);
            });
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).small().weak());
            }
            let (ok, cancel) = buttons(ui, tl!("Find"));
            if ok && !text.trim().is_empty() {
                match app.run("mailings.findRecipient", json!({"text": text.trim()})) {
                    Ok(_) => return true,
                    Err(e) => *message = e,
                }
            }
            cancel
        }
        Dialog::MergeRule { rule, fields, field, value, then, els } => {
            let skip = rule == "SKIPIF";
            ui.label(tl!(if skip {
                "Leave a recipient out of the merge when a field has this value."
            } else {
                "Insert one text or another, depending on a field's value."
            }));
            ui.add_space(4.0);
            egui::Grid::new("merge_rule").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Field:"));
                if fields.is_empty() {
                    ui.text_edit_singleline(field);
                } else {
                    egui::ComboBox::from_id_salt("merge_rule_field").selected_text(field.as_str()).width(200.0).show_ui(ui, |ui| {
                        for f in fields.iter() {
                            ui.selectable_value(field, f.clone(), f);
                        }
                    });
                }
                ui.end_row();
                ui.label(tl!("Equals:"));
                ui.text_edit_singleline(value);
                ui.end_row();
                if !skip {
                    ui.label(tl!("Then insert:"));
                    ui.text_edit_singleline(then);
                    ui.end_row();
                    ui.label(tl!("Otherwise insert:"));
                    ui.text_edit_singleline(els);
                    ui.end_row();
                }
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            let params = rule_params(d);
            if ok && let Some(params) = params {
                let _ = app.run("mailings.rules", params);
                return true;
            }
            cancel
        }
        Dialog::MatchFields { fields, address } => {
            if fields.is_empty() {
                ui.label(egui::RichText::new(tl!("Select recipients first to match their fields.")).small().weak());
            } else {
                ui.label(tl!("The recipient-list field that fills each part of an address block and greeting line."));
                ui.add_space(4.0);
                egui::Grid::new("match_fields").num_columns(2).spacing(vec2(24.0, 6.0)).striped(true).show(ui, |ui| {
                    for (part, key) in ADDRESS_PARTS {
                        ui.label(tl!(part));
                        match address.get(key).and_then(Value::as_str) {
                            Some(f) => ui.label(egui::RichText::new(f).strong()),
                            None => ui.label(egui::RichText::new(tl!("(not matched)")).weak()),
                        };
                        ui.end_row();
                    }
                });
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::CustomizeKeyboard { category, query, selected_cmd, new_key } => {
            ui.set_width(520.0);
            ui.horizontal(|ui| {
                ui.label(tl!("Categories:"));
                egui::ComboBox::from_id_salt("cat_picker").selected_text(category.as_str()).width(180.0).show_ui(ui, |ui| {
                    for cat in ["All Commands", "File", "Home", "Insert", "Layout", "Review", "View", "Tools"] {
                        if ui.selectable_label(category.as_str() == cat, cat).clicked() {
                            *category = cat.into();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Filter commands…")).desired_width(180.0));
            });
            ui.add_space(6.0);

            ui.label(tl!("Commands:"));
            let q = query.to_lowercase();
            let cat = category.clone();
            let reg = app.session.registry.clone();
            let all_specs = reg.all();

            let filtered: Vec<&wordcraft_engine::CommandSpec> = all_specs
                .iter()
                .filter(|c| {
                    let cat_match = if cat == "All Commands" { true } else { c.location.to_lowercase().contains(&cat.to_lowercase()) };
                    let query_match = q.is_empty()
                        || c.label.to_lowercase().contains(&q)
                        || c.id.to_lowercase().contains(&q)
                        || tl!(c.label).to_lowercase().contains(&q);
                    cat_match && query_match
                })
                .collect();

            let mut chosen = selected_cmd.clone();
            egui::Frame::NONE
                .fill(ui.visuals().extreme_bg_color)
                .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                .inner_margin(4)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                        for c in &filtered {
                            let is_sel = *selected_cmd == c.id;
                            let sc = crate::widgets::shortcut_text(app, c.id);
                            let text = if sc.is_empty() {
                                format!("{} ({})", tl!(c.label), c.id)
                            } else {
                                format!("{} ({})   [{}]", tl!(c.label), c.id, sc)
                            };
                            if ui.selectable_label(is_sel, text).clicked() {
                                chosen = c.id.to_string();
                            }
                        }
                    });
                });
            *selected_cmd = chosen;

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let cur_sc = if !selected_cmd.is_empty() {
                let sc = crate::widgets::shortcut_text(app, selected_cmd);
                if sc.is_empty() { "None".to_string() } else { sc }
            } else {
                "Select a command above".to_string()
            };

            ui.horizontal(|ui| {
                ui.label(tl!("Current shortcut:"));
                ui.label(egui::RichText::new(&cur_sc).strong().color(crate::theme::APP_COLOR));
            });
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(tl!("Press new shortcut key:"));
                ui.add(egui::TextEdit::singleline(new_key).desired_width(160.0).hint_text("e.g. Ctrl+Alt+K, Mod+Shift+T"));
                if ui.small_button("+Ctrl").clicked() && !new_key.contains("Ctrl+") && !new_key.contains("Mod+") {
                    *new_key = format!("Ctrl+{new_key}");
                }
                if ui.small_button("+Shift").clicked() && !new_key.contains("Shift+") {
                    *new_key = format!("Shift+{new_key}");
                }
                if ui.small_button("+Alt").clicked() && !new_key.contains("Alt+") {
                    *new_key = format!("Alt+{new_key}");
                }
            });

            ui.add_space(8.0);
            let mut close = false;
            ui.horizontal(|ui| {
                let can_assign = !selected_cmd.is_empty() && !new_key.trim().is_empty();
                if ui.add_enabled(can_assign, egui::Button::new(tl!("Assign"))).clicked() {
                    let standard_key = new_key.replace("Ctrl+", "Mod+").trim().to_string();
                    let _ = app.run("tools.customizeKeyboard", json!({"command": selected_cmd, "key": standard_key}));
                    app.status(format!("Assigned shortcut {standard_key} to {selected_cmd}"));
                    new_key.clear();
                }
                let can_remove = !selected_cmd.is_empty() && cur_sc != "None";
                if ui.add_enabled(can_remove, egui::Button::new(tl!("Remove"))).clicked() {
                    let _ = app.run("tools.customizeKeyboard", json!({"command": selected_cmd, "key": ""}));
                    app.status(format!("Removed shortcut for {selected_cmd}"));
                }
                if ui.button(tl!("Reset All")).clicked() {
                    let _ = app.run("tools.customizeKeyboard", json!({"reset": true}));
                    app.status("Reset all shortcuts to defaults");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::BanglaConverter { source_text, result_text } => {
            ui.heading(tl!("Bangla Converter (Unicode <-> ANSI / SutonnyMJ)"));
            ui.label(tl!("Convert text between modern Unicode and legacy Bijoy/SutonnyMJ ANSI without external software."));
            ui.add_space(8.0);

            ui.label(tl!("Input Text:"));
            ui.add(egui::TextEdit::multiline(source_text).desired_rows(5).desired_width(f32::INFINITY));

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tl!("Unicode -> ANSI (SutonnyMJ)")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::unicode_to_ansi(source_text);
                }
                if ui.button(tl!("ANSI (SutonnyMJ) -> Unicode")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::ansi_to_unicode(source_text);
                }
                if ui.button(tl!("Phonetic -> Unicode")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::phonetic_transliterate(source_text);
                }
                if ui.button(tl!("Clear")).clicked() {
                    source_text.clear();
                    result_text.clear();
                }
            });

            ui.add_space(8.0);
            ui.label(tl!("Converted Result:"));
            ui.add(egui::TextEdit::multiline(result_text).desired_rows(5).desired_width(f32::INFINITY));

            ui.add_space(6.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Copy Result")).clicked() && !result_text.is_empty() {
                    ui.ctx().copy_text(result_text.clone());
                    app.status(tl!("Copied converted text to clipboard"));
                }
                if ui.button(tl!("Insert at Caret")).clicked() && !result_text.is_empty() {
                    let _ = app.run("text.insert", json!({"text": result_text}));
                    close = true;
                }
                ui.separator();
                let has_sel = !app.session.sel.is_collapsed();
                if ui.add_enabled(has_sel, egui::Button::new(tl!("Convert Selection to ANSI"))).clicked() {
                    let _ = app.run("tools.banglaConvert", json!({"direction": "unicodeToAnsi", "target": "selection"}));
                    close = true;
                }
                if ui.add_enabled(has_sel, egui::Button::new(tl!("Convert Selection to Unicode"))).clicked() {
                    let _ = app.run("tools.banglaConvert", json!({"direction": "ansiToUnicode", "target": "selection"}));
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::CheckUpdates { checking: _, latest_version, release_notes, download_url, status_msg } => {
            ui.set_width(460.0);
            ui.label(egui::RichText::new(tl!("WordCraft Update Center")).font(semibold(18.0)));
            ui.add_space(4.0);

            let current_v = env!("CARGO_PKG_VERSION");
            ui.horizontal(|ui| {
                ui.label(tl!("Current Version:"));
                ui.label(egui::RichText::new(format!("v{current_v}")).font(semibold(13.0)));
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Latest Version:"));
                ui.label(egui::RichText::new(format!("v{latest_version}")).color(egui::Color32::from_rgb(0x10, 0x7C, 0x41)).font(semibold(13.0)));
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            ui.label(egui::RichText::new(status_msg.as_str()).italics());
            ui.add_space(6.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new(tl!("Release Information:")).font(semibold(12.0)));
                ui.label(release_notes.as_str());
            });

            ui.add_space(10.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Visit Releases / Download")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(download_url.as_str()));
                }
                if ui.button(tl!("Download Windows Setup")).clicked() {
                    let setup_url = format!(
                        "https://github.com/rbr48/wordcraft/releases/latest/download/wordcraft-{}-windows-x64-setup.exe",
                        env!("CARGO_PKG_VERSION")
                    );
                    ui.ctx().open_url(egui::OpenUrl::new_tab(setup_url));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::CheckErrors { unknown, records } => {
            if unknown.is_empty() {
                ui.label(tl!("No errors found: every merge field is in the recipient list."));
            } else {
                ui.label(tl!("These merge fields aren't in the recipient list:"));
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for f in unknown.iter() {
                        ui.label(egui::RichText::new(format!("• {f}")).color(Tokens::get(ui.ctx()).red));
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Recipients: {count}"), &[("count", &records.to_string())])).weak());
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
    }
}

/// The Recipient List dialog's body: a file button, the editable table, OK/Cancel. True to close.
fn recipient_list(app: &mut WordApp, ui: &mut Ui, fields: &mut Vec<String>, rows: &mut Vec<Vec<String>>, message: &mut String) -> bool {
    const CELL_W: f32 = 120.0;
    const X_W: f32 = 18.0;
    const ROW_H: f32 = 24.0;
    // Wide enough for every field (up to a limit; more scroll sideways).
    let gap = ui.spacing().item_spacing.x;
    let table_w = fields.len() as f32 * (CELL_W + X_W + 2.0 * gap) + X_W + 2.0 * gap;
    ui.set_min_width(table_w.clamp(340.0, 720.0));
    ui.label(tl!("Type the recipients, or load them from a file."));
    if ui.button(tl!("Use an Existing List…")).clicked() {
        // The picker takes over; the list it loads replaces this one.
        let _ = app.run("ui.openRecipientList", json!({}));
        return true;
    }
    ui.add_space(6.0);
    let mut remove_field = None;
    let can_remove = fields.len() > 1;
    let mut remove_row = None;
    let mut x_w = X_W;
    egui::ScrollArea::horizontal().id_salt("recipients_h").max_width(720.0).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (i, f) in fields.iter_mut().enumerate() {
                    ui.add_sized([CELL_W, ROW_H - 4.0], egui::TextEdit::singleline(f).font(egui::TextStyle::Button).hint_text(tl!("Field name")));
                    let x = ui.add_enabled(can_remove, egui::Button::new("×").small().min_size(vec2(X_W, X_W))).on_hover_text(tl!("Remove field"));
                    x_w = x.rect.width();
                    if x.clicked() {
                        remove_field = Some(i);
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("recipients_v").max_height(260.0).show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for r in range {
                    let Some(row) = rows.get_mut(r) else { continue };
                    ui.horizontal(|ui| {
                        for c in row.iter_mut() {
                            ui.add_sized([CELL_W, ROW_H - 4.0], egui::TextEdit::singleline(c));
                            // Under the field's remove button.
                            ui.allocate_exact_size(vec2(x_w, X_W), Sense::hover());
                        }
                        if ui.add(egui::Button::new("×").small().min_size(vec2(X_W, X_W))).on_hover_text(tl!("Remove recipient")).clicked() {
                            remove_row = Some(r);
                        }
                    });
                }
            });
        });
    });
    if let Some(i) = remove_field.filter(|_| fields.len() > 1) {
        fields.remove(i);
        for r in rows.iter_mut() {
            if i < r.len() {
                r.remove(i);
            }
        }
    }
    if let Some(r) = remove_row.filter(|r| *r < rows.len()) {
        rows.remove(r);
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("New Entry")).clicked() {
            rows.push(vec![String::new(); fields.len()]);
        }
        if ui.button(tl!("Add Field")).clicked() {
            fields.push(String::new());
            for r in rows.iter_mut() {
                r.push(String::new());
            }
        }
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).color(Tokens::get(ui.ctx()).red));
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if !ok {
        return cancel;
    }
    match recipient_table(fields, rows).map_err(|e| tl!(e).to_string()).and_then(|params| app.load_recipients(params)) {
        Ok(_) => true,
        Err(e) => {
            *message = e;
            false
        }
    }
}

/// The Paragraph dialog's OK. `align`, `left` and `right` are as seen on the page; `flags` are
/// keep with next, keep lines together, page break before, widow/orphan control.
#[allow(clippy::too_many_arguments)]
/// The Table Properties dialog's fields: Table, Row, Column and Cell sections.
fn table_properties(ui: &mut Ui, f: &mut TableForm) {
    let unit = wordcraft_geom::Unit::default();
    fn len(v: &mut f32, lo: f32, unit: wordcraft_geom::Unit) -> egui::DragValue<'_> {
        egui::DragValue::new(v).speed(0.01).range(lo..=22.0).suffix(unit.suffix()).max_decimals(2)
    }
    let heading = |ui: &mut Ui, s: &str| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(tl!(s)).font(semibold(12.5)));
    };
    heading(ui, "Table");
    egui::Grid::new("tp_table").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.checkbox(&mut f.width_on, tl!("Preferred width:"));
        ui.add_enabled(f.width_on, len(&mut f.width, 0.1, unit));
        ui.end_row();
        ui.label(tl!("Alignment:"));
        ui.horizontal(|ui| {
            for (v, l) in [("left", "Left"), ("center", "Center"), ("right", "Right")] {
                ui.radio_value(&mut f.align, v.to_string(), tl!(l));
            }
        });
        ui.end_row();
        ui.label(tl!("Indent from left:"));
        ui.add_enabled(f.align == "left", egui::DragValue::new(&mut f.indent).speed(0.01).range(-11.0..=22.0).suffix(unit.suffix()).max_decimals(2));
        ui.end_row();
        ui.label("");
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.rtl, false, tl!("Left-to-right"));
            ui.radio_value(&mut f.rtl, true, tl!("Right-to-left"));
        });
        ui.end_row();
    });
    egui::Grid::new("tp_spacing").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.checkbox(&mut f.cell_spacing_on, tl!("Allow spacing between cells:"));
        ui.add_enabled(f.cell_spacing_on, len(&mut f.cell_spacing, 0.0, unit));
        ui.end_row();
    });
    heading(ui, "Alternative text");
    ui.add(egui::TextEdit::singleline(&mut f.alt_text).hint_text(tl!("Describes the table for screen readers")).desired_width(320.0));
    heading(ui, "Row");
    egui::Grid::new("tp_row").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.checkbox(&mut f.row_height_on, tl!("Specify height:"));
        ui.horizontal(|ui| {
            ui.add_enabled(f.row_height_on, len(&mut f.row_height, 0.02, unit));
            ui.add_enabled_ui(f.row_height_on, |ui| {
                egui::ComboBox::from_id_salt("tp_rule").selected_text(tl!(if f.row_exact { "Exactly" } else { "At least" })).show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.row_exact, false, tl!("At least"));
                    ui.selectable_value(&mut f.row_exact, true, tl!("Exactly"));
                });
            });
        });
        ui.end_row();
    });
    ui.checkbox(&mut f.allow_break, tl!("Allow row to break across pages"));
    ui.checkbox(&mut f.header_row, tl!("Repeat as header row on each page"));
    heading(ui, "Column");
    egui::Grid::new("tp_col").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Preferred width:"));
        ui.add(len(&mut f.column_width, 0.1, unit));
        ui.end_row();
    });
    heading(ui, "Cell");
    egui::Grid::new("tp_cell").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
        ui.checkbox(&mut f.cell_width_on, tl!("Preferred width:"));
        ui.add_enabled(f.cell_width_on, len(&mut f.cell_width, 0.1, unit));
        ui.end_row();
        ui.label(tl!("Vertical alignment:"));
        ui.horizontal(|ui| {
            for (v, l) in [("top", "Top"), ("center", "Center"), ("bottom", "Bottom")] {
                ui.radio_value(&mut f.valign, v.to_string(), tl!(l));
            }
        });
        ui.end_row();
    });
}

fn apply_paragraph(
    app: &mut WordApp,
    rtl: bool,
    align: &str,
    left: f32,
    right: f32,
    first: f32,
    before: f32,
    after: f32,
    line: f32,
    flags: [bool; 4],
) {
    let [keep_next, keep_lines, page_break, widow] = flags;
    // Direction first: the alignment and indents are as seen on the page, and their logical
    // values depend on it.
    let _ = app.run(if rtl { "para.rtl" } else { "para.ltr" }, json!({}));
    let _ = app.run("para.align", json!({"value": align}));
    let (start, end) = if rtl { (right, left) } else { (left, right) };
    let _ = app.run(
        "para.set",
        json!({"props": {
            "indentLeft": start * 72.0, "indentRight": end * 72.0, "indentFirst": first * 72.0,
            "spaceBefore": before, "spaceAfter": after, "lineSpacing": {"rule": "multiple", "value": line},
            "keepNext": keep_next, "keepLines": keep_lines, "pageBreakBefore": page_break, "widowControl": widow,
        }}),
    );
}

/// The Paragraph dialog's Asian Typography: sets the flags that changed since it opened, so an
/// untouched tab adds no direct formatting.
fn apply_asian(app: &mut WordApp, asian: [bool; 6], was: [bool; 6]) {
    let keys = ["kinsoku", "wordWrap", "overflowPunct", "topLinePunct", "autoSpaceDE", "autoSpaceDN"];
    let changed: serde_json::Map<String, Value> =
        keys.iter().zip(asian.iter().zip(was)).filter(|(_, (now, was))| **now != *was).map(|(k, (now, _))| (k.to_string(), json!(now))).collect();
    if !changed.is_empty() {
        let _ = app.run("para.asianTypography", Value::Object(changed));
    }
}

/// Manage Styles' body. Reads the list from `styles.manage`'s data (never re-running the command,
/// which would ask to open this dialog again) and acts through commands.
fn manage_styles(app: &mut WordApp, ui: &mut Ui, alphabetical: &mut bool, selected: &mut String) -> bool {
    let list = wordcraft_engine::cmd::para::manage_list(&app.session, *alphabetical);
    let list = list.as_array().cloned().unwrap_or_default();
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if !list.iter().any(|x| str_of(x, "id") == *selected) {
        *selected = list.first().map(|x| str_of(x, "id")).unwrap_or_default();
    }
    ui.horizontal(|ui| {
        ui.label(tl!("Sort order:"));
        egui::ComboBox::from_id_salt("manage_styles_sort")
            .selected_text(if *alphabetical { tl!("Alphabetical") } else { tl!("As Recommended") })
            .show_ui(ui, |ui| {
                ui.selectable_value(alphabetical, false, tl!("As Recommended"));
                ui.selectable_value(alphabetical, true, tl!("Alphabetical"));
            });
    });
    let t = Tokens::get(ui.ctx());
    let shown = egui::Id::new("manage_styles_shown");
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.border)).inner_margin(4).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("manage_styles_list").max_height(200.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(380.0);
            for st in &list {
                let id = str_of(st, "id");
                let glyph = match st.get("type").and_then(Value::as_str) {
                    Some("character") => "a",
                    Some("linked") => "¶a",
                    Some("table") => "⊞",
                    _ => "¶",
                };
                let hidden = st.get("hidden").and_then(Value::as_bool).unwrap_or(false);
                let mut name = egui::RichText::new(format!("{glyph}  {}", str_of(st, "name")));
                if hidden {
                    name = name.weak();
                }
                let r = ui.selectable_label(*selected == id, name);
                // Bring the selection into view once, when it changes (opening, New Style…).
                if *selected == id && ui.data(|d| d.get_temp::<String>(shown)).as_deref() != Some(id.as_str()) {
                    r.scroll_to_me(Some(egui::Align::Center));
                    ui.data_mut(|d| d.insert_temp(shown, id.clone()));
                }
                if r.clicked() {
                    ui.data_mut(|d| d.insert_temp(shown, id.clone()));
                    *selected = id.clone();
                }
                if r.double_clicked() {
                    *selected = id;
                    if let Some(Dialog::ModifyStyle { id, name, font, size, bold, italic, color, before, after, .. }) =
                        Dialog::modify_style(app, selected)
                    {
                        app.dialog = Some(Dialog::ModifyStyle { back: true, id, name, font, size, bold, italic, color, before, after });
                    }
                }
            }
        });
    });
    let leave = |ui: &mut Ui| {
        ui.data_mut(|d| d.remove::<String>(shown));
        true
    };
    if app.dialog.is_some() {
        return leave(ui);
    }
    let Some(cur) = list.iter().find(|x| str_of(x, "id") == *selected).cloned() else { return close_button(ui) && leave(ui) };
    let ty = str_of(&cur, "type");
    ui.add_space(6.0);
    crate::previews::style_preview(app, ui, selected, &ty, vec2(388.0, if ty == "table" { 64.0 } else { 40.0 }));
    ui.add_space(4.0);
    ui.add(egui::Label::new(egui::RichText::new(describe(&cur)).small()).wrap());
    ui.add_space(6.0);
    let builtin = cur.get("builtIn").and_then(Value::as_bool).unwrap_or(true);
    if ty != "table" {
        let mut gallery = cur.get("inGallery").and_then(Value::as_bool).unwrap_or(false);
        if ui.checkbox(&mut gallery, tl!("Show in the Styles gallery")).changed() {
            let _ = app.run("styles.setVisibility", json!({"style": selected, "gallery": gallery}));
        }
    }
    let mut hidden = cur.get("hidden").and_then(Value::as_bool).unwrap_or(false);
    if ui.checkbox(&mut hidden, tl!("Hide from style lists")).changed() {
        let _ = app.run("styles.setVisibility", json!({"style": selected, "hidden": hidden}));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Table styles are edited from Table Design.
        if ui.add_enabled(ty != "table", egui::Button::new(tl!("Modify…"))).clicked()
            && let Some(Dialog::ModifyStyle { id, name, font, size, bold, italic, color, before, after, .. }) = Dialog::modify_style(app, selected)
        {
            app.dialog = Some(Dialog::ModifyStyle { back: true, id, name, font, size, bold, italic, color, before, after });
        }
        if ui.button(tl!("New Style…")).clicked() {
            app.dialog = Some(Dialog::NewStyle { name: "Style1".into(), based_on: str_of(&cur, "name"), back: true });
        }
        let del = ui.add_enabled(!builtin, egui::Button::new(tl!("Delete")));
        let del = if builtin { del.on_disabled_hover_text(tl!("Built-in styles can't be deleted.")) } else { del };
        if del.clicked() {
            let _ = app.run("styles.delete", json!({"style": selected}));
            selected.clear();
        }
    });
    (app.dialog.is_some() || close_button(ui)) && leave(ui)
}

/// A dialog's single Close button (Escape too).
fn close_button(ui: &mut Ui) -> bool {
    ui.add_space(8.0);
    let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape));
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        close |= ui.add(egui::Button::new(egui::RichText::new(tl!("Close")).color(egui::Color32::WHITE)).fill(crate::theme::APP_COLOR)).clicked();
    });
    close
}

/// A style's description: its type, the formatting it sets and what it is based on.
fn describe(st: &Value) -> String {
    let f = st.get("format").cloned().unwrap_or_default();
    let fs = |k: &str| f.get(k).and_then(Value::as_str);
    let fnum = |k: &str| f.get(k).and_then(Value::as_f64);
    let pt = |x: f64| format!("{}", (x * 10.0).round() / 10.0);
    let mut parts: Vec<String> = vec![
        match st.get("type").and_then(Value::as_str) {
            Some("character") => tl!("Character style"),
            Some("linked") => tl!("Linked style (paragraph and character)"),
            Some("table") => tl!("Table style"),
            _ => tl!("Paragraph style"),
        }
        .to_string(),
    ];
    if st.get("builtIn").and_then(Value::as_bool).unwrap_or(false) {
        parts.push(tl!("Built-in style").to_string());
    }
    if let Some(x) = fs("font") {
        parts.push(crate::i18n::fmt(tl!("Font: {font}"), &[("font", x)]));
    }
    if let Some(x) = fnum("size") {
        parts.push(crate::i18n::fmt(tl!("{size} pt"), &[("size", &pt(x))]));
    }
    if f.get("bold").and_then(Value::as_bool) == Some(true) {
        parts.push(tl!("Bold").to_string());
    }
    if f.get("italic").and_then(Value::as_bool) == Some(true) {
        parts.push(tl!("Italic").to_string());
    }
    if let Some(x) = fs("color") {
        parts.push(crate::i18n::fmt(tl!("Color: {color}"), &[("color", &format!("#{x}"))]));
    }
    if let Some(x) = fnum("spaceBefore") {
        parts.push(crate::i18n::fmt(tl!("Space before: {pt} pt"), &[("pt", &pt(x))]));
    }
    if let Some(x) = fnum("spaceAfter") {
        parts.push(crate::i18n::fmt(tl!("Space after: {pt} pt"), &[("pt", &pt(x))]));
    }
    if let Some(b) = st.get("basedOn").and_then(Value::as_str) {
        parts.push(crate::i18n::fmt(tl!("Based on: {style}"), &[("style", b)]));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::props::Align;
    use wordcraft_engine::Session;

    fn app_with(text: &str) -> WordApp {
        let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        let _ = app.run("document.setText", json!({"text": text}));
        app
    }

    fn props(app: &WordApp) -> wordcraft_doc::props::ParaProps {
        app.session.doc.para_at(&app.session.sel.focus).map(|p| p.props.clone()).unwrap_or_default()
    }

    /// Open the Paragraph dialog and press OK without changing anything.
    fn ok_unchanged(app: &mut WordApp) {
        let Some(Dialog::Paragraph { rtl, align, left, right, first, before, after, line, keep_next, keep_lines, page_break, widow, .. }) =
            Dialog::open("paragraph", app)
        else {
            panic!("no paragraph dialog")
        };
        apply_paragraph(app, rtl, &align, left, right, first, before, after, line, [keep_next, keep_lines, page_break, widow]);
    }

    #[test]
    fn paragraph_dialog_shows_direction_and_visual_alignment() {
        let mut app = app_with("سلام دنیا");
        let _ = app.run("para.rtl", json!({}));
        let _ = app.run("para.set", json!({"props": {"indentLeft": 36.0}}));
        let Some(Dialog::Paragraph { rtl, align, left, right, .. }) = Dialog::open("paragraph", &mut app) else { panic!() };
        assert!(rtl);
        assert_eq!(align, "right", "a start-aligned right-to-left paragraph shows as right aligned");
        assert_eq!((left, right), (0.0, 0.5), "the start indent is the right one");
    }

    #[test]
    fn paragraph_dialog_ok_keeps_what_it_shows() {
        for (rtl, align) in [(false, "para.alignCenter"), (false, "para.alignRight"), (true, "para.alignLeft"), (true, "para.alignRight")] {
            let mut app = app_with("متن");
            let _ = app.run(if rtl { "para.rtl" } else { "para.ltr" }, json!({}));
            let _ = app.run(align, json!({}));
            let _ = app.run("para.set", json!({"props": {"indentLeft": 18.0, "indentRight": 9.0}}));
            let before = props(&app);
            ok_unchanged(&mut app);
            let after = props(&app);
            assert_eq!((after.bidi, after.align), (before.bidi, before.align), "{align} in rtl={rtl}");
            assert_eq!((after.indent_left, after.indent_right), (Some(18.0), Some(9.0)), "{align} in rtl={rtl}");
        }
    }

    #[test]
    fn paragraph_dialog_changes_direction() {
        let mut app = app_with("Hello");
        let Some(Dialog::Paragraph { align, left, right, first, before, after, line, keep_next, keep_lines, page_break, widow, .. }) =
            Dialog::open("paragraph", &mut app)
        else {
            panic!()
        };
        // Right to left, aligned right on the page, 1" from the right edge.
        apply_paragraph(&mut app, true, "right", left, right + 1.0, first, before, after, line, [keep_next, keep_lines, page_break, widow]);
        let _ = align;
        let p = props(&app);
        assert_eq!(p.bidi, Some(true));
        assert_eq!(p.align, Some(Align::Left), "right on the page is the start edge");
        assert_eq!(p.indent_left, Some(72.0), "the right indent is the start indent");
    }
}
