//! `wordcraft-cli`: WordCraft from the command line.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use serde_json::{Value, json};
use wordcraft_engine::Session;

const USAGE: &str = "\
wordcraft-cli — WordCraft from the command line

USAGE:
  wordcraft-cli convert <in> <out>            convert between formats (docx, pdf, odt, rtf, html, md, tex, txt, json, png)
  wordcraft-cli batch <in_dir> [out_dir] [--to ext] [--from ext]
                                              batch convert directory (docx, pdf, epub, odt, rtf, html, md, txt, png)
  wordcraft-cli info <file>                   pages, words, paragraphs, properties (JSON)
  wordcraft-cli text <file>                   plain text
  wordcraft-cli inspect <file>                document structure (JSON)
  wordcraft-cli render <file> <out.png> [--page N] [--scale S]
  wordcraft-cli run [--file F | --template T] --cmd 'id={json}' [--cmd …] [--save OUT] [--print]
                                              run commands headlessly, then save
  wordcraft-cli commands [--json]             list every command
  wordcraft-cli parity [--markdown]           feature-catalog parity
  wordcraft-cli corpus <dir>                  test & benchmark DOCX files in directory
  wordcraft-cli mcp [--connect HOST:PORT]     MCP server on stdio (headless, or bridged to the app)
  wordcraft-cli --version
";

fn open(path: &str) -> Result<Session, String> {
    let doc = wordcraft_engine::io::open_path(std::path::Path::new(path))?;
    let mut s = Session::new(doc);
    s.path = Some(path.into());
    Ok(s)
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

#[derive(Clone, Copy)]
struct OptionSpec {
    name: &'static str,
    takes_value: bool,
}

fn validate_options(command: &str, args: &[String], specs: &[OptionSpec]) -> Result<(), String> {
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if let Some(name) = arg.strip_prefix("--") {
            let Some(spec) = specs.iter().find(|spec| spec.name == arg) else {
                return Err(format!("{command}: unknown option --{name}"));
            };
            if spec.takes_value && args.get(i + 1).is_some_and(|value| !value.starts_with("--")) {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

const NO_OPTIONS: &[OptionSpec] = &[];
const BATCH_OPTIONS: &[OptionSpec] = &[OptionSpec { name: "--to", takes_value: true }, OptionSpec { name: "--from", takes_value: true }];
const RENDER_OPTIONS: &[OptionSpec] = &[OptionSpec { name: "--page", takes_value: true }, OptionSpec { name: "--scale", takes_value: true }];
const RUN_OPTIONS: &[OptionSpec] = &[
    OptionSpec { name: "--file", takes_value: true },
    OptionSpec { name: "--template", takes_value: true },
    OptionSpec { name: "--cmd", takes_value: true },
    OptionSpec { name: "--save", takes_value: true },
    OptionSpec { name: "--page", takes_value: true },
    OptionSpec { name: "--scale", takes_value: true },
    OptionSpec { name: "--print", takes_value: false },
];
const COMMANDS_OPTIONS: &[OptionSpec] = &[OptionSpec { name: "--json", takes_value: false }];
const PARITY_OPTIONS: &[OptionSpec] = &[OptionSpec { name: "--markdown", takes_value: false }];
const MCP_OPTIONS: &[OptionSpec] = &[OptionSpec { name: "--connect", takes_value: true }];

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let pos = |i: usize| rest.iter().filter(|a| !a.starts_with("--")).nth(i).cloned().ok_or_else(|| format!("missing argument\n\n{USAGE}"));
    match cmd {
        "batch" => {
            validate_options("batch", &rest, BATCH_OPTIONS)?;
            let in_dir = pos(0)?;
            let out_dir = pos(1).unwrap_or_else(|_| in_dir.clone());
            let target_ext = arg_value(&rest, "--to").unwrap_or_else(|| "pdf".to_string()).to_ascii_lowercase();
            let from_ext = arg_value(&rest, "--from").map(|e| e.to_ascii_lowercase());
            let dir = std::path::Path::new(&in_dir);
            let out_path = std::path::Path::new(&out_dir);
            if !dir.is_dir() {
                return Err(format!("not a directory: {in_dir}"));
            }
            std::fs::create_dir_all(out_path).map_err(|e| format!("cannot create output directory: {e}"))?;

            let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    if !p.is_file() {
                        return false;
                    }
                    if let Some(ref ext) = from_ext {
                        p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case(ext))
                    } else {
                        matches!(
                            p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase).as_deref(),
                            Some("docx" | "odt" | "rtf" | "html" | "htm" | "md" | "txt")
                        )
                    }
                })
                .collect();
            entries.sort();
            if entries.is_empty() {
                println!("No matching input files found in {}", dir.display());
                return Ok(());
            }
            println!("Batch converting {} files from {} to .{} in {}...", entries.len(), dir.display(), target_ext, out_path.display());
            let mut succeeded = 0;
            let mut failed = 0;
            let t0 = std::time::Instant::now();
            for p in &entries {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
                let out_file = out_path.join(format!("{stem}.{target_ext}"));
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("file");
                match wordcraft_engine::io::open_path(p) {
                    Ok(doc) => {
                        let mut s = Session::new(doc);
                        let save_res = if target_ext == "png" {
                            s.run("file.exportPng", &json!({"path": out_file.to_string_lossy(), "page": 1}))
                        } else {
                            s.run("file.save", &json!({"path": out_file.to_string_lossy()}))
                        };
                        match save_res {
                            Ok(_) => {
                                succeeded += 1;
                                println!("  ✓ {} -> {}", fname, out_file.file_name().and_then(|f| f.to_str()).unwrap_or(""));
                            }
                            Err(e) => {
                                failed += 1;
                                eprintln!("  ✗ {} -> conversion failed: {}", fname, e);
                            }
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        eprintln!("  ✗ {} -> open failed: {}", fname, e);
                    }
                }
            }
            let el = t0.elapsed().as_millis();
            println!("Batch complete: {} succeeded, {} failed in {} ms", succeeded, failed, el);
            if failed > 0 {
                return Err(format!("{failed} files failed conversion"));
            }
            Ok(())
        }
        "convert" => {
            validate_options("convert", &rest, NO_OPTIONS)?;
            let (i, o) = (pos(0)?, pos(1)?);
            let mut s = open(&i)?;
            let r = if o.to_ascii_lowercase().ends_with(".png") {
                s.run("file.exportPng", &json!({"path": o, "page": 1}))
            } else {
                s.run("file.save", &json!({"path": o}))
            };
            r.map_err(|e| e.to_string())?;
            eprintln!("wrote {o}");
            Ok(())
        }
        "info" => {
            validate_options("info", &rest, NO_OPTIONS)?;
            let mut s = open(&pos(0)?)?;
            println!("{}", serde_json::to_string_pretty(&s.run("file.info", &json!({})).map_err(|e| e.to_string())?).unwrap_or_default());
            Ok(())
        }
        "text" => {
            validate_options("text", &rest, NO_OPTIONS)?;
            let s = open(&pos(0)?)?;
            println!("{}", s.doc.plain_text(wordcraft_doc::StoryRef::Body));
            Ok(())
        }
        "inspect" => {
            validate_options("inspect", &rest, NO_OPTIONS)?;
            let mut s = open(&pos(0)?)?;
            println!("{}", serde_json::to_string_pretty(&s.run("document.inspect", &json!({})).map_err(|e| e.to_string())?).unwrap_or_default());
            Ok(())
        }
        "render" => {
            validate_options("render", &rest, RENDER_OPTIONS)?;
            let (i, o) = (pos(0)?, pos(1)?);
            let mut s = open(&i)?;
            let page = arg_value(&rest, "--page").and_then(|p| p.parse::<u64>().ok()).unwrap_or(1);
            let scale = arg_value(&rest, "--scale").and_then(|p| p.parse::<f64>().ok()).unwrap_or(2.0);
            let r = s.run("file.exportPng", &json!({"path": o, "page": page, "scale": scale})).map_err(|e| e.to_string())?;
            println!("{r}");
            Ok(())
        }
        "run" => {
            validate_options("run", &rest, RUN_OPTIONS)?;
            let mut s = match (arg_value(&rest, "--file"), arg_value(&rest, "--template")) {
                (Some(f), _) => open(&f)?,
                (None, Some(t)) => {
                    let mut s = Session::new(wordcraft_doc::Document::new());
                    s.run("file.new", &json!({"template": t})).map_err(|e| e.to_string())?;
                    s
                }
                _ => Session::new(wordcraft_doc::Document::new()),
            };
            let mut i = 0;
            while i < rest.len() {
                if rest.get(i).map(String::as_str) == Some("--cmd") {
                    let spec = rest.get(i + 1).ok_or("--cmd needs a value")?;
                    let (id, params) = match spec.split_once('=') {
                        Some((id, p)) => (id.to_string(), serde_json::from_str::<Value>(p).map_err(|e| format!("{id}: bad JSON params: {e}"))?),
                        None => (spec.clone(), json!({})),
                    };
                    let r = s.run(&id, &params).map_err(|e| format!("{id}: {e}"))?;
                    if rest.iter().any(|a| a == "--print") {
                        println!("{id}: {r}");
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if let Some(out) = arg_value(&rest, "--save") {
                let r = if out.to_ascii_lowercase().ends_with(".png") {
                    let mut v = json!({"path": out});
                    if let Some(n) = arg_value(&rest, "--page").and_then(|p| p.parse::<u64>().ok()) {
                        v["page"] = json!(n);
                    }
                    if let Some(k) = arg_value(&rest, "--scale").and_then(|p| p.parse::<f64>().ok()) {
                        v["scale"] = json!(k);
                    }
                    s.run("file.exportPng", &v)
                } else {
                    s.run("file.save", &json!({"path": out}))
                };
                r.map_err(|e| e.to_string())?;
                eprintln!("wrote {out}");
            }
            Ok(())
        }
        "commands" => {
            validate_options("commands", &rest, COMMANDS_OPTIONS)?;
            let s = Session::new(wordcraft_doc::Document::new());
            if rest.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&s.registry.describe()).unwrap_or_default());
            } else {
                for c in s.registry.all() {
                    println!("{:<32} {:<36} {:<24} {}", c.id, c.label, c.shortcut, c.location);
                }
            }
            Ok(())
        }
        "parity" => {
            validate_options("parity", &rest, PARITY_OPTIONS)?;
            let s = Session::new(wordcraft_doc::Document::new());
            let p = wordcraft_engine::catalog::parity(&s.registry);
            if rest.iter().any(|a| a == "--markdown") {
                print!("{}", parity_markdown(&p));
            } else {
                println!("{}", serde_json::to_string_pretty(&p).unwrap_or_default());
            }
            Ok(())
        }
        "corpus" => {
            validate_options("corpus", &rest, NO_OPTIONS)?;
            let dir_path = pos(0)?;
            let dir = std::path::Path::new(&dir_path);
            if !dir.is_dir() {
                return Err(format!("not a directory: {dir_path}"));
            }
            let mut entries: Vec<std::path::PathBuf> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("docx")) {
                        entries.push(p);
                    }
                }
            }
            entries.sort();
            if entries.is_empty() {
                println!("No .docx files found in {}", dir.display());
                return Ok(());
            }
            println!("Benchmarking {} DOCX files in {}...", entries.len(), dir.display());
            println!("{:<36} {:<8} {:<8} {:<10} {:<10}", "File", "Status", "Pages", "Words", "Layout");
            println!("{}", "-".repeat(76));
            let mut passed = 0;
            let mut failed = 0;
            let mut total_layout_ms = 0.0;
            for p in &entries {
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("file");
                let t0 = std::time::Instant::now();
                match wordcraft_engine::io::open_path(p) {
                    Ok(doc) => {
                        let mut s = Session::new(doc);
                        match s.run("file.info", &json!({})) {
                            Ok(info) => {
                                let pages = info.get("pages").and_then(Value::as_u64).unwrap_or(0);
                                let words = info.get("words").and_then(Value::as_u64).unwrap_or(0);
                                let lms = info.get("layoutMs").and_then(Value::as_f64).unwrap_or_else(|| t0.elapsed().as_secs_f64() * 1000.0);
                                total_layout_ms += lms;
                                passed += 1;
                                println!("{:<36} {:<8} {:<8} {:<10} {:.1} ms", fname, "PASS", pages, words, lms);
                            }
                            Err(e) => {
                                failed += 1;
                                println!("{:<36} {:<8} error: {}", fname, "FAIL", e);
                            }
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        println!("{:<36} {:<8} open error: {}", fname, "FAIL", e);
                    }
                }
            }
            println!("{}", "-".repeat(76));
            let avg = if passed > 0 { total_layout_ms / (passed as f64) } else { 0.0 };
            println!("Summary: {} passed, {} failed, avg layout {:.1} ms", passed, failed, avg);
            if failed > 0 {
                return Err(format!("{failed} corpus files failed validation"));
            }
            Ok(())
        }
        "mcp" => {
            validate_options("mcp", &rest, MCP_OPTIONS)?;
            let backend: Box<dyn wordcraft_mcp::Backend> = match arg_value(&rest, "--connect") {
                Some(addr) => {
                    let remote = wordcraft_mcp::Remote::connect(&addr).map_err(|e| format!("can't reach the WordCraft app at {addr}: {e}"))?;
                    if let Some(w) = remote.key_warning() {
                        eprintln!("wordcraft-cli: {w}");
                    }
                    Box::new(remote)
                }
                None => Box::new(wordcraft_mcp::Headless::default()),
            };
            let mut server = wordcraft_mcp::Server::new(backend);
            let stdin = std::io::stdin();
            server.serve(stdin.lock(), std::io::stdout()).map_err(|e| e.to_string())
        }
        "--version" | "-V" => {
            println!("wordcraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "" | "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn parity_markdown(p: &Value) -> String {
    let mut s = String::from(
        "# WordCraft feature parity\n\nGenerated by `cargo xtask parity` (`wordcraft-cli parity --markdown`): the word-processor feature catalog (`crates/engine/src/catalog.rs`) compared with the live command registry.\n\n",
    );
    s += &format!(
        "**{} of {} catalog features have commands ({}%).**\n\n| Tab | Live | Total |\n|---|---|---|\n",
        p["live"], p["total"], p["percent"]
    );
    for t in p["tabs"].as_array().cloned().unwrap_or_default() {
        s += &format!("| {} | {} | {} |\n", t["tab"].as_str().unwrap_or(""), t["live"], t["total"]);
    }
    s += "\n## Missing\n\n";
    for t in p["tabs"].as_array().cloned().unwrap_or_default() {
        let m = t["missing"].as_array().cloned().unwrap_or_default();
        if m.is_empty() {
            continue;
        }
        s += &format!("### {}\n\n", t["tab"].as_str().unwrap_or(""));
        for x in m {
            s += &format!("- {}\n", x.as_str().unwrap_or(""));
        }
        s += "\n";
    }
    s
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wordcraft-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn validators_accept_only_the_documented_options() {
        assert!(validate_options("info", &args(&["file.docx"]), NO_OPTIONS).is_ok());
        assert!(validate_options("render", &args(&["in.docx", "out.png", "--page", "2", "--scale", "1.5"]), RENDER_OPTIONS).is_ok());
        assert!(validate_options("run", &args(&["--cmd", "text.insert={}", "--cmd", "edit.undo", "--print"]), RUN_OPTIONS).is_ok());
        assert!(validate_options("commands", &args(&["--json"]), COMMANDS_OPTIONS).is_ok());
        assert!(validate_options("parity", &args(&["--markdown"]), PARITY_OPTIONS).is_ok());
        assert!(validate_options("mcp", &args(&["--connect", "127.0.0.1:9000"]), MCP_OPTIONS).is_ok());
        assert!(validate_options("render", &args(&["in.docx", "out.png", "--page"]), RENDER_OPTIONS).is_ok());
    }

    #[test]
    fn unknown_options_are_rejected_before_file_access() {
        assert_eq!(run(&args(&["info", "missing.docx", "--passwrod", "x"])), Err("info: unknown option --passwrod".into()));
        assert_eq!(run(&args(&["render", "missing.docx", "out.png", "--sclae", "2"])), Err("render: unknown option --sclae".into()));
    }
}
