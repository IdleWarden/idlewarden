// SPDX-License-Identifier: MPL-2.0
use std::cmp::Ordering;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::session::SessionHandle;

const EVENT: &str = "assistant";
const MAX_CONTEXT_CHARS: usize = 12_000;
const NO_WINDOW: u32 = 0x0800_0000;

const PERSONA: &str = "You are IdleWarden, the Warden: a small pixel owl guardian who watches over the player's game from an overlay.\n\
Speak as the Warden in the first person, calm, dry and watchful, in short sentences. Call the player 'toi'. Do not use emojis.\n\
Answer in French, plain text without markdown, short and practical.\n\
You are given the live signals of the game before each message. Use them to say what is worth doing next.\n\
If the player sincerely asks what you are, say you are IdleWarden the Warden, powered by Claude.";

#[derive(Default)]
pub struct Assistant {
    session_id: Mutex<Option<String>>,
    busy: AtomicBool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Partial { text: String },
    Done { text: String },
    Failed { message: String },
}

#[derive(Debug, PartialEq)]
pub enum StreamLine {
    Session(String),
    Text(String),
    Result { ok: bool, text: String },
}

pub fn parse_stream_line(line: &str) -> Option<StreamLine> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("type")?.as_str()? {
        "system" if value.get("subtype")?.as_str()? == "init" => Some(StreamLine::Session(
            value.get("session_id")?.as_str()?.to_owned(),
        )),
        "assistant" => {
            let text = value
                .get("message")?
                .get("content")?
                .as_array()?
                .iter()
                .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            let text = text.trim();
            (!text.is_empty()).then(|| StreamLine::Text(text.to_owned()))
        }
        "result" => Some(StreamLine::Result {
            ok: !value
                .get("is_error")
                .and_then(|e| e.as_bool())
                .unwrap_or(false),
            text: value
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_owned(),
        }),
        _ => None,
    }
}

pub fn build_args(system_prompt: &str, session_id: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--tools".to_owned(),
        String::new(),
        "--append-system-prompt".to_owned(),
        system_prompt.to_owned(),
    ];
    if let Some(id) = session_id {
        args.push("--resume".to_owned());
        args.push(id.to_owned());
    }
    args
}

pub fn build_prompt(context: &str, text: &str) -> String {
    let context: String = context.chars().take(MAX_CONTEXT_CHARS).collect();
    let context = if context.trim().is_empty() {
        "no signals available, the game may not be running"
    } else {
        context.trim()
    };
    format!(
        "Live game signals:\n{context}\n\nPlayer message:\n{}",
        text.trim()
    )
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (left, right) = (parse(a), parse(b));
    for i in 0..left.len().max(right.len()) {
        let order = left.get(i).unwrap_or(&0).cmp(right.get(i).unwrap_or(&0));
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

pub fn bundled_candidates(app_data: &Path) -> Vec<PathBuf> {
    let root = app_data.join("Claude").join("claude-code");
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let Ok(versions) = fs::read_dir(&root) else {
        return Vec::new();
    };
    for version in versions.flatten() {
        let Ok(builds) = fs::read_dir(version.path()) else {
            continue;
        };
        for build in builds.flatten() {
            let exe = build.path().join("claude.exe");
            if exe.is_file() {
                found.push((version.file_name().to_string_lossy().into_owned(), exe));
            }
        }
    }
    found.sort_by(|a, b| compare_versions(&b.0, &a.0));
    found.into_iter().map(|(_, exe)| exe).collect()
}

fn is_claude_code(executable: &Path) -> bool {
    Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("Claude Code")
        })
        .unwrap_or(false)
}

fn locate_claude() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("claude")];
    if let Some(app_data) = std::env::var_os("APPDATA") {
        candidates.extend(bundled_candidates(Path::new(&app_data)));
    }
    candidates
        .into_iter()
        .find(|candidate| is_claude_code(candidate))
}

pub fn plugin_prompt(plugin_root: &Path, plugin_id: &str) -> Option<String> {
    for entry in fs::read_dir(plugin_root).ok()?.flatten() {
        let manifest = fs::read_to_string(entry.path().join("plugin.json")).ok();
        let id = manifest
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                value
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(str::to_owned)
            });
        if id.as_deref() == Some(plugin_id) {
            return fs::read_to_string(entry.path().join("assistant.md")).ok();
        }
    }
    None
}

fn system_prompt(plugin_root: &Path, plugin: Option<&str>) -> String {
    match plugin.and_then(|id| plugin_prompt(plugin_root, id)) {
        Some(extra) => format!("{PERSONA}\n\n{}", extra.trim()),
        None => PERSONA.to_owned(),
    }
}

fn emit(app: &AppHandle, reply: Reply) {
    if let Err(error) = app.emit(EVENT, reply) {
        tracing::warn!("assistant: could not emit: {error}");
    }
}

fn ask(app: &AppHandle, args: Vec<String>, prompt: String) -> Result<(), String> {
    let executable = locate_claude().ok_or_else(|| {
        "Claude Code introuvable. Installe-le et connecte-toi avec `claude` dans un terminal."
            .to_owned()
    })?;

    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(NO_WINDOW);
    }

    let mut child = command.spawn().map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or("stdin unavailable")?
        .write_all(prompt.as_bytes())
        .map_err(|error| error.to_string())?;

    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let assistant = app.state::<Assistant>();
    let mut answered = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        match parse_stream_line(&line) {
            Some(StreamLine::Session(id)) => {
                *assistant
                    .session_id
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some(id);
            }
            Some(StreamLine::Text(text)) => emit(app, Reply::Partial { text }),
            Some(StreamLine::Result { ok: true, text }) => {
                answered = true;
                emit(app, Reply::Done { text });
            }
            Some(StreamLine::Result { ok: false, text }) => {
                answered = true;
                emit(app, Reply::Failed { message: text });
            }
            None => {}
        }
    }

    let status = child.wait().map_err(|error| error.to_string())?;
    if answered {
        return Ok(());
    }
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
    }
    Err(if stderr.trim().is_empty() {
        format!("Claude s'est arrêté sans répondre ({status}). Vérifie que tu es connecté.")
    } else {
        stderr.trim().to_owned()
    })
}

#[tauri::command]
pub fn assistant_ask(
    app: AppHandle,
    session: State<'_, SessionHandle>,
    assistant: State<'_, Assistant>,
    text: String,
    context: String,
    plugin: Option<String>,
) -> Result<(), String> {
    if assistant.busy.swap(true, AtomicOrdering::SeqCst) {
        return Err("le Gardien est déjà en train de répondre".to_owned());
    }

    let system = system_prompt(&session.plugin_root(), plugin.as_deref());
    let resume = assistant
        .session_id
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let args = build_args(&system, resume.as_deref());
    let prompt = build_prompt(&context, &text);

    std::thread::spawn(move || {
        if let Err(message) = ask(&app, args, prompt) {
            emit(&app, Reply::Failed { message });
        }
        app.state::<Assistant>()
            .busy
            .store(false, AtomicOrdering::SeqCst);
    });
    Ok(())
}

#[tauri::command]
pub fn assistant_reset(assistant: State<'_, Assistant>) {
    *assistant
        .session_id
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_init_line_gives_the_session_id() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc"}"#;
        assert_eq!(
            parse_stream_line(line),
            Some(StreamLine::Session("abc".to_owned()))
        );
    }

    #[test]
    fn assistant_text_blocks_are_joined_and_tool_blocks_ignored() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"un"},{"type":"tool_use","name":"Read"},{"type":"text","text":"deux"}]}}"#;
        assert_eq!(
            parse_stream_line(line),
            Some(StreamLine::Text("un\ndeux".to_owned()))
        );
    }

    #[test]
    fn a_turn_without_text_produces_nothing() {
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit"}]}}"#;
        assert_eq!(parse_stream_line(line), None);
    }

    #[test]
    fn the_result_line_carries_the_error_flag() {
        let line = r#"{"type":"result","is_error":true,"result":"Not logged in"}"#;
        assert_eq!(
            parse_stream_line(line),
            Some(StreamLine::Result {
                ok: false,
                text: "Not logged in".to_owned()
            })
        );
    }

    #[test]
    fn garbage_and_unknown_lines_are_ignored() {
        assert_eq!(parse_stream_line("not json"), None);
        assert_eq!(parse_stream_line(r#"{"type":"user"}"#), None);
        assert_eq!(parse_stream_line(""), None);
    }

    #[test]
    fn the_session_is_resumed_only_when_there_is_one() {
        let first = build_args("system", None);
        assert!(!first.contains(&"--resume".to_owned()));
        let next = build_args("system", Some("abc"));
        assert_eq!(&next[next.len() - 2..], ["--resume", "abc"]);
    }

    #[test]
    fn the_assistant_has_no_tools() {
        let args = build_args("system", None);
        let at = args
            .iter()
            .position(|arg| arg == "--tools")
            .expect("a --tools flag");
        assert_eq!(args[at + 1], "");
    }

    #[test]
    fn the_prompt_truncates_huge_context_and_names_a_missing_one() {
        let long = "x".repeat(50_000);
        assert!(build_prompt(&long, "salut").len() < MAX_CONTEXT_CHARS + 200);
        assert!(build_prompt("  ", "salut").contains("no signals available"));
    }

    #[test]
    fn versions_compare_numerically_not_as_text() {
        assert_eq!(compare_versions("2.1.299", "2.1.295"), Ordering::Greater);
        assert_eq!(compare_versions("2.1.9", "2.1.10"), Ordering::Less);
        assert_eq!(compare_versions("2.1", "2.1.0"), Ordering::Equal);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idlewarden-assistant-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn the_newest_bundled_build_comes_first() {
        let root = scratch("bundled");
        for version in ["2.1.295", "2.1.299", "2.1.9"] {
            let build = root
                .join("Claude")
                .join("claude-code")
                .join(version)
                .join("abc");
            fs::create_dir_all(&build).expect("build dir");
            fs::write(build.join("claude.exe"), b"").expect("exe");
        }
        let found = bundled_candidates(&root);
        let versions: Vec<_> = found
            .iter()
            .map(|path| {
                path.parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .collect();
        assert_eq!(
            versions,
            [
                Some("2.1.299".to_owned()),
                Some("2.1.295".to_owned()),
                Some("2.1.9".to_owned())
            ]
        );
    }

    #[test]
    fn a_missing_bundled_folder_gives_no_candidates() {
        assert!(bundled_candidates(&scratch("none")).is_empty());
    }

    #[test]
    fn a_plugin_prompt_is_found_by_the_plugin_id_not_the_folder_name() {
        let root = scratch("plugins");
        let folder = root.join("any-folder-name");
        fs::create_dir_all(&folder).expect("plugin dir");
        fs::write(folder.join("plugin.json"), r#"{"id":"dev.example.game"}"#).expect("manifest");
        fs::write(folder.join("assistant.md"), "Parle de ce jeu.").expect("prompt");
        assert_eq!(
            plugin_prompt(&root, "dev.example.game").as_deref(),
            Some("Parle de ce jeu.")
        );
        assert_eq!(plugin_prompt(&root, "dev.example.other"), None);
    }

    #[test]
    fn the_system_prompt_keeps_the_persona_and_appends_the_plugin_text() {
        let root = scratch("system");
        let folder = root.join("game");
        fs::create_dir_all(&folder).expect("plugin dir");
        fs::write(folder.join("plugin.json"), r#"{"id":"dev.example.game"}"#).expect("manifest");
        fs::write(folder.join("assistant.md"), "Parle de ce jeu.").expect("prompt");
        let with_plugin = system_prompt(&root, Some("dev.example.game"));
        assert!(with_plugin.starts_with("You are IdleWarden"));
        assert!(with_plugin.ends_with("Parle de ce jeu."));
        assert_eq!(system_prompt(&root, None), PERSONA);
    }
}
