//! Copilot's document agent: the model edits the open document through WordCraft's own commands.
//!
//! The agent works in rounds. Each round the model gets the user's request, a compact outline of
//! the document (block numbers, styles, fonts, text) and the commands it may use, and answers with
//! one JSON object: a note for the user, the commands to run, and whether it is done. The commands
//! run here through [`Session::run`], exactly as the ribbon runs them; each one's result goes back
//! to the model next round, so it can check its work and correct mistakes. Everything one agent
//! run changes is a single undo step.
//!
//! The model never reaches files, the clipboard, dialogs, the interface, macros or the network:
//! only commands [`allowed`] here run, and each command checks its own params (hostile params
//! fail, they never panic). The caller makes the provider call ([`AgentRun::request`]), off the
//! UI thread, and hands the reply to [`AgentRun::on_reply`] on the thread that owns the session.

use serde_json::{Value, json};
use wordcraft_doc::{Block, Path, Pos, StoryRef};

use crate::cmd::pos_json;
use crate::cmd::tools::{CopilotRequest, copilot_history, cut_chars, provider_request, standing_instructions};
use crate::{Session, p};

/// The most rounds one request may take (the Copilot settings can lower it).
pub const MAX_ROUNDS: usize = 15;
/// The rounds a request takes unless the settings say otherwise.
pub const DEFAULT_ROUNDS: usize = 8;
/// The most commands one round runs; extra ones are reported back as skipped.
pub const MAX_COMMANDS: usize = 40;
/// Outline limits: blocks listed, text shown per paragraph and per cell, rows shown per table.
const OUTLINE_BLOCKS: usize = 400;
const BLOCK_TEXT: usize = 160;
const CELL_TEXT: usize = 60;
const TABLE_ROWS: usize = 12;
/// The most outline text sent per round (characters).
const OUTLINE_CHARS: usize = 40_000;
/// How much of a command's result or error goes back to the model.
const RESULT_TEXT: usize = 160;

/// The agent's own selection helpers (block numbers are easier for a model than byte offsets).
const SELECT_BLOCKS: &str = "agent.selectBlocks";
const SELECT_CELL: &str = "agent.selectCell";

/// Command families the agent may use: editing, formatting, styles, tables, page layout.
const ALLOWED_PREFIXES: &[&str] = &[
    "text.",
    "format.",
    "para.",
    "styles.",
    "table.",
    "layout.",
    "design.",
    "list.",
    "select.",
    "caret.",
    "hf.",
    "references.",
    "insert.",
    "review.",
];
/// Single commands from other families.
const ALLOWED_IDS: &[&str] =
    &["edit.replace", "edit.replaceAll", "edit.find", "edit.copyFormat", "edit.pasteFormat", "tools.banglaConvert", "tools.banglaTransliterate"];
/// Commands in allowed families that read files or the network, act on objects, or change the
/// user's review state: never run by the agent.
const DENIED: &[&str] = &[
    "insert.picture",
    "insert.onlinePicture",
    "insert.video",
    "insert.screenshot",
    "insert.object",
    "insert.spreadsheet",
    "insert.textFromFile",
    "insert.canvas",
    "insert.inkEquation",
    "review.compare",
    "review.combine",
    "review.restrict",
    "review.readAloud",
    "review.translate",
    "review.blockAuthors",
    "review.trackChanges",
    "review.acceptAll",
    "review.rejectAll",
    "review.accept",
    "review.reject",
    "references.researcher",
    "select.addObject",
    "select.objects",
    "styles.delete",
];
/// Words in the ids of commands that open panes or dialogs, which an agent has no use for.
const DENIED_WORDS: &[&str] = &["dialog", "Dialog", "pane", "Pane", "inspector", "Inspector", "manage", "Options", "options"];

/// The agent's instructions; the command catalog and the user's standing instructions follow.
const SYSTEM: &str = "You are WordCraft's document agent. You edit the open document by running WordCraft commands, \
the same commands as the word processor's ribbon. Work in rounds. Every reply is ONE JSON object and nothing else:
{\"say\": \"one or two sentences for the user about what you are doing\", \"commands\": [{\"command\": \"<id>\", \"params\": {}}], \"done\": false}
Rules:
- Commands act on the current selection or caret, as for a user: select first, then format or edit.
- Select whole paragraphs with agent.selectBlocks {\"from\": n, \"to\": m} using block numbers from the outline; the text of \
one table cell with agent.selectCell {\"table\": n, \"row\": r, \"col\": c}; a phrase with select.text {\"text\": \"...\", \"occurrence\": k}; \
everything with select.all.
- Use only commands from the catalog, with params as documented (`?` marks optional params). Never invent ids or params.
- At most 40 commands per reply. After each reply you get every command's result and the updated outline: check them, \
fix errors, then continue.
- Keep the document's text and language unless the user asks to change them; never delete content you were not asked to. \
Prefer paragraph styles (para.style) for headings and body text, and keep fonts the document already uses for its script \
(for Bangla text keep its Bangla font).
- When the task is complete, reply with \"done\": true and say what you changed. If the request is only a question, \
answer it in \"say\" with no commands and \"done\": true.

Command catalog:
";

/// Whether the agent may run command `id`.
pub fn allowed(s: &Session, id: &str) -> bool {
    if id == SELECT_BLOCKS || id == SELECT_CELL {
        return true;
    }
    if DENIED.contains(&id) || DENIED_WORDS.iter().any(|w| id.contains(w)) {
        return false;
    }
    (ALLOWED_PREFIXES.iter().any(|p| id.starts_with(p)) || ALLOWED_IDS.contains(&id)) && s.registry.get(id).is_some()
}

/// The commands the agent may use, one per line: id, label, params.
pub fn catalog(s: &Session) -> String {
    let mut out = String::new();
    out.push_str("agent.selectBlocks {\"from\": n, \"to\"?: n} — select whole paragraphs, from block `from` through block `to`\n");
    out.push_str("agent.selectCell {\"table\": n, \"row\": n, \"col\": n} — select the text of one cell of the table at block `table`\n");
    for spec in s.registry.all() {
        if allowed(s, spec.id) {
            out.push_str(spec.id);
            out.push_str(" — ");
            out.push_str(spec.label);
            if !spec.params.is_empty() {
                out.push(' ');
                out.push_str(spec.params);
            }
            out.push('\n');
        }
    }
    out
}

/// One line of text for the outline: newlines and tabs as spaces, cut to `cap` characters.
fn snippet(text: &str, cap: usize) -> String {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let n = flat.chars().count();
    let mut out = cut_chars(flat, cap);
    if n > cap {
        out.push('…');
    }
    out
}

/// A compact outline of the document for the model: page setup, then each body block with its
/// number, kind, style, alignment, font and (the start of) its text; tables show their first
/// rows cell by cell. The current selection comes last.
pub fn outline(s: &Session) -> String {
    let sp = &s.doc.last_section;
    let mut out = format!(
        "Page: {:.0} × {:.0} pt{}, margins left {:.0}, right {:.0}, top {:.0}, bottom {:.0} pt; {} section(s); {} blocks\n",
        sp.page_w,
        sp.page_h,
        if sp.landscape { ", landscape" } else { "" },
        sp.margin_left,
        sp.margin_right,
        sp.margin_top,
        sp.margin_bottom,
        s.doc.sections().len(),
        s.doc.body.len(),
    );
    for (i, b) in s.doc.body.iter().enumerate() {
        if i >= OUTLINE_BLOCKS || out.len() > OUTLINE_CHARS {
            out.push_str(&format!("… {} more blocks not shown\n", s.doc.body.len().saturating_sub(i)));
            break;
        }
        match &**b {
            Block::Para(p) => {
                let mut desc = vec![p.props.style.clone().unwrap_or_else(|| "Normal".into())];
                if let Some(a) = p.props.align {
                    desc.push(format!("{a:?}").to_lowercase());
                }
                let mut fonts = p.run_ranges().filter(|(r, _)| !r.is_empty()).map(|(_, c)| (c.font.clone(), c.size));
                if let Some((font, size)) = fonts.next() {
                    if let Some(f) = font {
                        desc.push(f);
                    }
                    if let Some(sz) = size {
                        desc.push(format!("{sz}pt"));
                    }
                    if fonts.any(|(f2, _)| f2.is_some() && f2 != p.run_ranges().find_map(|(_, c)| c.font.clone())) {
                        desc.push("mixed fonts".into());
                    }
                }
                if p.props.numbering.is_some_and(|n| n.num != 0) {
                    desc.push("list".into());
                }
                let text = p.plain_text();
                if text.trim().is_empty() {
                    out.push_str(&format!("#{i} ¶ ({}) [empty]\n", desc.join(", ")));
                } else {
                    out.push_str(&format!("#{i} ¶ ({}): {}\n", desc.join(", "), snippet(&text, BLOCK_TEXT)));
                }
            }
            Block::Table(t) => {
                out.push_str(&format!(
                    "#{i} table, {} rows × {} columns{}\n",
                    t.rows.len(),
                    t.cols(),
                    t.props.style.as_ref().map(|s| format!(", style {s}")).unwrap_or_default()
                ));
                for (r, row) in t.rows.iter().enumerate().take(TABLE_ROWS) {
                    let cells: Vec<String> = row
                        .cells
                        .iter()
                        .enumerate()
                        .map(|(c, cell)| {
                            let text: Vec<String> = cell.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect();
                            format!("[{c}] {}", snippet(&text.join(" / "), CELL_TEXT))
                        })
                        .collect();
                    out.push_str(&format!("    row {r}: {}\n", cells.join(" | ")));
                }
                if t.rows.len() > TABLE_ROWS {
                    out.push_str(&format!("    … {} more rows\n", t.rows.len() - TABLE_ROWS));
                }
            }
        }
    }
    let sel = s.selected_text();
    if sel.trim().is_empty() {
        out.push_str(&format!("Selection: caret in block {}\n", s.sel.focus.path.0.first().copied().unwrap_or(0)));
    } else {
        out.push_str(&format!("Selection: \"{}\"\n", snippet(&sel, BLOCK_TEXT)));
    }
    out
}

/// One round's reply from the model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub say: String,
    pub commands: Vec<(String, Value)>,
    pub done: bool,
}

/// Read the model's reply: the JSON object in it (code fences and text around it are ignored).
pub fn parse_reply(text: &str) -> Result<Step, String> {
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else { return Err("no JSON object".into()) };
    let body = text.get(a..=b).ok_or("no JSON object")?;
    let v: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let say = v.get("say").or_else(|| v.get("message")).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let mut commands = Vec::new();
    if let Some(list) = v.get("commands").and_then(Value::as_array) {
        for c in list {
            let id = c.get("command").or_else(|| c.get("id")).and_then(Value::as_str).unwrap_or("").trim();
            if id.is_empty() {
                continue;
            }
            let params = c.get("params").filter(|p| p.is_object()).cloned().unwrap_or_else(|| json!({}));
            commands.push((cut_chars(id.to_string(), 80), params));
        }
    }
    let done = v.get("done").and_then(Value::as_bool).unwrap_or(commands.is_empty());
    Ok(Step { say: cut_chars(say, 4_000), commands, done })
}

/// What one command did.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    pub command: String,
    pub ok: bool,
    /// The selection after a selection command, or the error.
    pub note: String,
}

/// The `select.range` params selecting body blocks `from` through `to` whole.
fn blocks_range(s: &Session, v: &Value) -> Result<Value, String> {
    let n = s.doc.body.len();
    let from = p::u64(v, "from").ok_or("`from` (a block number) is required")? as usize;
    let to = p::u64(v, "to").map_or(from, |t| t as usize);
    if from >= n {
        return Err(format!("there is no block {from} (the document has {n})"));
    }
    let to = to.clamp(from, n - 1);
    let para_len = |i: usize| -> Result<usize, String> {
        match s.doc.body.get(i).map(|b| &**b) {
            Some(Block::Para(p)) => Ok(p.len()),
            _ => Err(format!("block {i} is a table: use agent.selectCell or table.selectRange for tables")),
        }
    };
    para_len(from)?;
    let end = para_len(to)?;
    Ok(json!({"anchor": pos_json(&Pos::body(from, 0)), "focus": pos_json(&Pos::body(to, end))}))
}

/// The `select.range` params selecting the text of cell (`row`, `col`) of the table at `table`.
fn cell_range(s: &Session, v: &Value) -> Result<Value, String> {
    let (Some(tb), Some(r), Some(c)) = (p::u64(v, "table"), p::u64(v, "row"), p::u64(v, "col")) else {
        return Err("`table`, `row` and `col` are required".into());
    };
    let (tb, r, c) = (tb as usize, r as usize, c as usize);
    let Some(Block::Table(t)) = s.doc.body.get(tb).map(|b| &**b) else { return Err(format!("block {tb} is not a table")) };
    let cell = t.rows.get(r).and_then(|row| row.cells.get(c)).ok_or_else(|| format!("the table at block {tb} has no cell at row {r}, column {c}"))?;
    let last = cell.blocks.len().saturating_sub(1);
    let end = match cell.blocks.get(last).map(|b| &**b) {
        Some(Block::Para(p)) => p.len(),
        _ => return Err("that cell ends in a nested table: select its paragraphs with select.text".into()),
    };
    let at = |k: usize, off: usize| Pos { story: StoryRef::Body, path: Path(vec![tb as u32, r as u32, c as u32, k as u32]), off };
    Ok(json!({"anchor": pos_json(&at(0, 0)), "focus": pos_json(&at(last, end))}))
}

/// Run the round's commands in order. Once the agent has changed the document, every later
/// command joins that undo step, so one Undo reverts the whole run. `edited` carries that across
/// rounds.
pub fn apply(s: &mut Session, commands: &[(String, Value)], edited: &mut bool) -> Vec<Ran> {
    let mut out = Vec::with_capacity(commands.len().min(MAX_COMMANDS + 1));
    for (i, (id, params)) in commands.iter().enumerate() {
        if i >= MAX_COMMANDS {
            out.push(Ran { command: id.clone(), ok: false, note: format!("skipped: at most {MAX_COMMANDS} commands per reply") });
            continue;
        }
        if !allowed(s, id) {
            out.push(Ran { command: id.clone(), ok: false, note: "not an available command (use the catalog)".into() });
            continue;
        }
        let (run_id, run_params) = match id.as_str() {
            SELECT_BLOCKS => match blocks_range(s, params) {
                Ok(v) => ("select.range", v),
                Err(e) => {
                    out.push(Ran { command: id.clone(), ok: false, note: e });
                    continue;
                }
            },
            SELECT_CELL => match cell_range(s, params) {
                Ok(v) => ("select.range", v),
                Err(e) => {
                    out.push(Ran { command: id.clone(), ok: false, note: e });
                    continue;
                }
            },
            other => (other, params.clone()),
        };
        if *edited {
            s.join_next_undo();
        }
        let rev = s.rev();
        match s.run(run_id, &run_params) {
            Ok(_) => {
                if s.rev() != rev {
                    *edited = true;
                }
                let note = if run_id.starts_with("select.") || run_id.starts_with("caret.") {
                    let sel = s.selected_text();
                    if sel.trim().is_empty() { "ok (caret placed)".to_string() } else { format!("ok, selected \"{}\"", snippet(&sel, 60)) }
                } else {
                    "ok".to_string()
                };
                out.push(Ran { command: id.clone(), ok: true, note });
            }
            Err(e) => out.push(Ran { command: id.clone(), ok: false, note: cut_chars(e.to_string(), RESULT_TEXT) }),
        }
    }
    out
}

/// What a round did, for the user.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    /// The model's note.
    pub say: String,
    pub ran: Vec<Ran>,
    /// No more rounds: the agent is done, or stopped.
    pub finished: bool,
    /// Why the agent stopped before saying it was done.
    pub stopped: Option<String>,
}

/// One agent request in progress.
#[derive(Clone, Debug)]
pub struct AgentRun {
    goal: String,
    /// The Copilot settings: provider, key, model, generation, language, instructions, history.
    params: Value,
    max_rounds: usize,
    /// Rounds answered so far.
    pub round: usize,
    /// The last round's results, for the next request.
    feedback: String,
    /// The agent has changed the document (its later edits join that undo step).
    edited: bool,
    pub commands_run: usize,
    pub errors: usize,
    bad_replies: usize,
    /// The document revision after the agent's last round: anything else means the user edited.
    rev: u64,
    finished: bool,
}

impl AgentRun {
    /// Start an agent run for `goal` with the Copilot settings in `params` (`max_rounds` too).
    pub fn new(s: &Session, goal: &str, params: Value) -> Self {
        let max_rounds = p::u64(&params, "max_rounds").map_or(DEFAULT_ROUNDS, |r| (r as usize).clamp(1, MAX_ROUNDS));
        AgentRun {
            goal: cut_chars(goal.trim().to_string(), 8_000),
            params,
            max_rounds,
            round: 0,
            feedback: String::new(),
            edited: false,
            commands_run: 0,
            errors: 0,
            bad_replies: 0,
            rev: s.rev(),
            finished: false,
        }
    }

    /// The agent has changed the document.
    pub fn edited(&self) -> bool {
        self.edited
    }

    /// No more rounds.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// The next round's provider call (made by the caller, off the UI thread).
    pub fn request(&self, s: &Session) -> CopilotRequest {
        let system = format!("{SYSTEM}{}{}", catalog(s), standing_instructions(&self.params));
        let history = copilot_history(&self.params);
        let mut user = String::new();
        if !history.is_empty() {
            user.push_str(&format!("Conversation so far:\n{history}\n"));
        }
        user.push_str(&format!(
            "Request: {}\n\nRound {} of at most {}. Document outline:\n{}",
            self.goal,
            self.round + 1,
            self.max_rounds,
            outline(s)
        ));
        if !self.feedback.is_empty() {
            user.push_str(&format!("\n{}", self.feedback));
        }
        provider_request(&self.params, "agent", "Agent", system, user)
    }

    /// Handle the model's reply to the last request: run its commands and get ready for the next
    /// round, or finish.
    pub fn on_reply(&mut self, s: &mut Session, reply: &str) -> Turn {
        if self.finished {
            return Turn { finished: true, ..Default::default() };
        }
        if s.rev() != self.rev {
            self.finished = true;
            return Turn {
                finished: true,
                stopped: Some("The document changed while the agent was working, so it stopped.".into()),
                ..Default::default()
            };
        }
        self.round += 1;
        let step = match parse_reply(reply) {
            Ok(step) => step,
            Err(e) => {
                self.bad_replies += 1;
                self.feedback = format!("Your last reply was not the JSON object asked for ({e}). Reply with the JSON object only.");
                let give_up = self.bad_replies >= 2 || self.round >= self.max_rounds;
                self.finished = give_up;
                return Turn {
                    finished: give_up,
                    stopped: give_up.then(|| "The model's replies weren't usable commands; try again or pick another model.".into()),
                    ..Default::default()
                };
            }
        };
        let ran = apply(s, &step.commands, &mut self.edited);
        self.rev = s.rev();
        self.commands_run += ran.len();
        self.errors += ran.iter().filter(|r| !r.ok).count();
        let mut fb = String::from("Results of your last commands:\n");
        for (i, r) in ran.iter().enumerate() {
            fb.push_str(&format!("{}. {} → {}{}\n", i + 1, r.command, if r.ok { "" } else { "error: " }, r.note));
        }
        self.feedback = fb;
        let out_of_rounds = self.round >= self.max_rounds;
        self.finished = step.done || step.commands.is_empty() || out_of_rounds;
        let stopped = (out_of_rounds && !step.done && !step.commands.is_empty())
            .then(|| format!("Stopped after {} rounds; ask again to continue.", self.max_rounds));
        Turn { say: step.say, ran, finished: self.finished, stopped }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wordcraft_doc::{Document, StoryRef};

    use super::*;

    fn params() -> Value {
        json!({"provider": "Custom", "model": "m", "api_key": "k", "base_url": "http://127.0.0.1:9/"})
    }

    fn para_style(s: &Session, i: usize) -> Option<String> {
        s.doc.body.get(i).and_then(|b| b.as_para()).and_then(|p| p.props.style.clone())
    }

    #[test]
    fn only_document_editing_commands_are_allowed() {
        let s = Session::new(Document::new());
        for id in [
            "format.bold",
            "para.style",
            "para.alignCenter",
            "table.insertRowBelow",
            "select.text",
            "layout.margins",
            "tools.banglaConvert",
            SELECT_BLOCKS,
        ] {
            assert!(allowed(&s, id), "{id} should be allowed");
        }
        for id in [
            "file.save",
            "file.open",
            "edit.paste",
            "edit.undo",
            "tools.copilot",
            "insert.picture",
            "review.acceptAll",
            "styles.pane",
            "format.fontDialog",
            "view.zoom",
            "nope.nothing",
        ] {
            assert!(!allowed(&s, id), "{id} must not be allowed");
        }
        let c = catalog(&s);
        assert!(c.contains("format.bold — Bold") && c.contains("para.style — Apply Style {\"style\""));
        assert!(!c.contains("file.save"));
        assert!(c.len() < 60_000, "the catalog fits a prompt: {} chars", c.len());
    }

    #[test]
    fn the_outline_numbers_blocks_and_shows_tables() {
        let mut s = Session::new(Document::from_text("প্রতিবেদন\nBody text"));
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("insert.table", &json!({"rows": 2, "cols": 2})).unwrap();
        s.run("text.insert", &json!({"text": "cell text"})).unwrap();
        let o = outline(&s);
        assert!(o.contains("#0 ¶ (Normal): প্রতিবেদন"), "{o}");
        assert!(o.contains("#1 ¶ (Normal): Body text"), "{o}");
        assert!(o.contains("table, 2 rows × 2 columns") && o.contains("row 0: [0] cell text | [1] "), "{o}");
        assert!(o.starts_with("Page: "));
    }

    #[test]
    fn replies_are_read_leniently_and_junk_is_an_error() {
        let fenced = "Here you go:\n```json\n{\"say\": \"Bolding\", \"commands\": [{\"command\": \"format.bold\"}], \"done\": true}\n```";
        let step = parse_reply(fenced).unwrap();
        assert_eq!(step, Step { say: "Bolding".into(), commands: vec![("format.bold".into(), json!({}))], done: true });
        // `id` for `command`, missing `done` means done when there is nothing to run.
        let step = parse_reply("{\"commands\": [{\"id\": \"para.alignCenter\", \"params\": {}}]}").unwrap();
        assert!(!step.done && step.commands.len() == 1);
        assert!(parse_reply("{\"say\": \"Only an answer\"}").unwrap().done);
        assert!(parse_reply("I can't do that").is_err());
        assert!(parse_reply("{not json}").is_err());
    }

    /// Two rounds: style the title, then bold the body and finish. Everything is one Undo.
    #[test]
    fn an_agent_run_edits_through_commands_and_undoes_in_one_step() {
        let mut s = Session::new(Document::from_text("Appraisal report\nThe loan was approved.\nThe branch agrees."));
        let before = s.doc.plain_text(StoryRef::Body);
        let depth = s.undo_depth();
        let mut run = AgentRun::new(&s, "Make the first line a heading and the rest bold", params());
        let req = run.request(&s);
        assert!(req.system.contains("Command catalog") && req.system.contains("para.style"));
        assert!(req.user.contains("Request: Make the first line a heading") && req.user.contains("#0 ¶ (Normal): Appraisal report"));

        let t1 = run.on_reply(
            &mut s,
            r#"{"say": "Styling the title.", "commands": [{"command": "agent.selectBlocks", "params": {"from": 0}}, {"command": "para.style", "params": {"style": "Heading1"}}], "done": false}"#,
        );
        assert!(!t1.finished && t1.ran.iter().all(|r| r.ok), "{t1:?}");
        assert!(t1.ran[0].note.contains("Appraisal report"), "selection results go back to the model");
        assert_eq!(para_style(&s, 0).as_deref(), Some("Heading1"));
        // The next request carries the results.
        assert!(run.request(&s).user.contains("1. agent.selectBlocks → ok"));

        let t2 = run.on_reply(
            &mut s,
            r#"{"say": "Done: heading and bold body.", "commands": [{"command": "agent.selectBlocks", "params": {"from": 1, "to": 2}}, {"command": "format.bold"}], "done": true}"#,
        );
        assert!(t2.finished && t2.stopped.is_none());
        let bold =
            |s: &Session, i: usize| s.doc.body.get(i).and_then(|b| b.as_para()).is_some_and(|p| p.run_ranges().any(|(_, c)| c.bold == Some(true)));
        assert!(bold(&s, 1) && bold(&s, 2));
        assert_eq!(s.undo_depth(), depth + 1, "the whole run is one undo step");
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.plain_text(StoryRef::Body), before);
        assert_eq!(para_style(&s, 0), None);
        assert!(!bold(&s, 1));
    }

    #[test]
    fn hostile_replies_never_panic_or_reach_outside_the_document() {
        let mut s = Session::new(Document::from_text("Text"));
        let mut run = AgentRun::new(&s, "x", params());
        let mut many = vec![json!({"command": "file.save", "params": {"path": "/tmp/agent-should-not-write.docx"}})];
        many.push(json!({"command": "agent.selectBlocks", "params": {"from": 99}}));
        many.push(json!({"command": "agent.selectCell", "params": {"table": 0, "row": 9, "col": 9}}));
        many.push(json!({"command": "para.style", "params": {"style": 12}}));
        for _ in 0..60 {
            many.push(json!({"command": "caret.right", "params": {}}));
        }
        let reply = json!({"say": "x", "commands": many, "done": false}).to_string();
        let t = run.on_reply(&mut s, &reply);
        assert!(!t.ran[0].ok && t.ran[0].note.contains("not an available command"));
        assert!(!t.ran[1].ok && t.ran[1].note.contains("no block 99"));
        assert!(!t.ran[2].ok);
        assert!(t.ran.iter().filter(|r| r.note.contains("skipped")).count() > 0, "at most 40 commands run per reply");
        assert!(!std::path::Path::new("/tmp/agent-should-not-write.docx").exists());
        assert_eq!(s.doc.plain_text(StoryRef::Body), "Text");
        assert!(!run.edited());
    }

    #[test]
    fn the_agent_stops_when_the_user_edits_or_the_model_rambles() {
        let mut s = Session::new(Document::from_text("Text"));
        let mut run = AgentRun::new(&s, "x", params());
        s.run("text.insert", &json!({"text": "!"})).unwrap();
        let t = run.on_reply(&mut s, r#"{"commands": [{"command": "format.bold"}]}"#);
        assert!(t.finished && t.stopped.is_some_and(|w| w.contains("document changed")));

        let mut run = AgentRun::new(&s, "x", params());
        let t = run.on_reply(&mut s, "Sure! I will make it bold.");
        assert!(!t.finished, "one bad reply gets a second chance");
        assert!(run.request(&s).user.contains("not the JSON object"));
        let t = run.on_reply(&mut s, "Bold it is.");
        assert!(t.finished && t.stopped.is_some());

        let mut run = AgentRun::new(&s, "x", json!({"max_rounds": 1}));
        let t = run.on_reply(&mut s, r#"{"commands": [{"command": "select.all"}], "done": false}"#);
        assert!(t.finished && t.stopped.is_some_and(|w| w.contains("1 rounds")));
    }
}
