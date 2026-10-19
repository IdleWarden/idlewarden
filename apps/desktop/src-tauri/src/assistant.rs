// SPDX-License-Identifier: MPL-2.0
use std::cmp::Ordering;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::session::SessionHandle;

const EVENT: &str = "assistant";
const MAX_CONTEXT_CHARS: usize = 12_000;
const MAX_PLUGIN_PROMPT_BYTES: u64 = 16 * 1024;
const MAX_LINE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PLUGIN_MANIFEST_BYTES: u64 = 256 * 1024;
const ANSWER_TIMEOUT: Duration = Duration::from_secs(180);

const PERSONA: &str = "You are IdleWarden, the Warden: a small pixel owl guardian who watches over the player's game from an overlay.\n\
Speak as the Warden in the first person, calm, dry and watchful, in short sentences. Call the player 'toi'. Do not use emojis.\n\
Answer in French, plain text without markdown, short and practical.\n\
You are given the live signals of the game before each message. Use them to say what is worth doing next.\n\
If the player sincerely asks what you are, say you are IdleWarden the Warden, powered by Claude.";

#[derive(Default)]
pub struct Assistant {
    session_id: Mutex<Option<String>>,
    generation: AtomicU64,
    busy: Arc<AtomicBool>,
    executable: Mutex<Option<PathBuf>>,
}

impl Assistant {
    fn resume_id(&self) -> Option<String> {
        self.session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn generation(&self) -> u64 {
        self.generation.load(AtomicOrdering::SeqCst)
    }

    fn remember(&self, generation: u64, id: String) {
        let mut slot = self
            .session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.generation() == generation {
            *slot = Some(id);
        }
    }

    fn forget(&self, generation: u64) {
        let mut slot = self
            .session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.generation() == generation {
            *slot = None;
        }
    }

    fn reset(&self) {
        let mut slot = self
            .session_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.generation.fetch_add(1, AtomicOrdering::SeqCst);
        *slot = None;
    }
}

struct Busy(Arc<AtomicBool>);

impl Busy {
    fn claim(assistant: &Assistant) -> Option<Self> {
        (!assistant.busy.swap(true, AtomicOrdering::SeqCst))
            .then(|| Busy(Arc::clone(&assistant.busy)))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.store(false, AtomicOrdering::SeqCst);
    }
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

#[derive(Debug, PartialEq)]
pub enum InitTools {
    NotInit,
    Unreported,
    Reported(Vec<String>),
}

pub fn init_tools(line: &str) -> InitTools {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
        return InitTools::NotInit;
    };
    let is_init = value.get("type").and_then(|t| t.as_str()) == Some("system")
        && value.get("subtype").and_then(|t| t.as_str()) == Some("init");
    if !is_init {
        return InitTools::NotInit;
    }
    match value.get("tools").and_then(|tools| tools.as_array()) {
        Some(tools) => InitTools::Reported(
            tools
                .iter()
                .filter_map(|tool| tool.as_str().map(str::to_owned))
                .collect(),
        ),
        None => InitTools::Unreported,
    }
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

const DISALLOWED_TOOLS: &str =
    "Bash,Edit,Write,NotebookEdit,Read,Glob,Grep,WebFetch,WebSearch,Task,Agent,PowerShell";

const SETTINGS: &str = r#"{"disableAllHooks":true}"#;

pub fn build_args(
    system_prompt_file: &Path,
    settings_file: &Path,
    session_id: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
        "--tools".to_owned(),
        String::new(),
        "--strict-mcp-config".to_owned(),
        "--disable-slash-commands".to_owned(),
        "--permission-mode".to_owned(),
        "plan".to_owned(),
        "--disallowed-tools".to_owned(),
        DISALLOWED_TOOLS.to_owned(),
        "--setting-sources".to_owned(),
        "user".to_owned(),
        "--settings".to_owned(),
        settings_file.display().to_string(),
        "--append-system-prompt-file".to_owned(),
        system_prompt_file.display().to_string(),
    ];
    if let Some(id) = session_id {
        args.push("--resume".to_owned());
        args.push(id.to_owned());
    }
    args
}

pub fn build_prompt(context: &str, text: &str) -> String {
    let truncated = context.chars().nth(MAX_CONTEXT_CHARS).is_some();
    let mut context: String = context.chars().take(MAX_CONTEXT_CHARS).collect();
    if truncated {
        context.push_str("\n[signals truncated]");
    }
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
            .map(|part| {
                part.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
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

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const STDERR_GRACE: Duration = Duration::from_secs(2);

fn is_claude_code(executable: &Path) -> bool {
    let mut command = Command::new(executable);
    command.arg("--version");
    let Ok(scratch) = ScratchDir::create() else {
        return false;
    };
    command.current_dir(scratch.path());
    let mut banner = String::new();
    let Ok(exit) = run_child_with(command, "", PROBE_TIMEOUT, false, |line| {
        banner.push_str(line)
    }) else {
        return false;
    };
    !exit.timed_out && banner.contains("Claude Code")
}

fn locate_claude(assistant: &Assistant) -> Option<PathBuf> {
    let cached = assistant
        .executable
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if cached.is_some() {
        return cached;
    }
    let found = find_claude();
    *assistant
        .executable
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = found.clone();
    found
}

fn forget_executable(assistant: &Assistant) {
    *assistant
        .executable
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn find_claude() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA") {
        candidates.extend(bundled_candidates(Path::new(&app_data)));
    }
    candidates.push(PathBuf::from("claude"));
    candidates
        .into_iter()
        .find(|candidate| is_claude_code(candidate))
}

pub fn plugin_prompt(plugin_root: &Path, plugin_id: &str) -> Option<String> {
    for entry in fs::read_dir(plugin_root).ok()?.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let manifest =
            read_plain_file(&entry.path().join("plugin.json"), MAX_PLUGIN_MANIFEST_BYTES);
        let id = manifest
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                value
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(str::to_owned)
            });
        if id.as_deref() == Some(plugin_id) {
            return read_plain_file(&entry.path().join("assistant.md"), MAX_PLUGIN_PROMPT_BYTES);
        }
    }
    None
}

fn read_plain_file(path: &Path, max_bytes: u64) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > max_bytes {
        tracing::warn!(
            "assistant: ignoring {}: not a plain file or too large",
            path.display()
        );
        return None;
    }
    fs::read_to_string(path).ok()
}

fn system_prompt(plugin_root: &Path, plugin: Option<&str>) -> String {
    match plugin.and_then(|id| plugin_prompt(plugin_root, id)) {
        Some(extra) => format!("{PERSONA}\n\n{}", extra.trim()),
        None => PERSONA.to_owned(),
    }
}

#[derive(Clone, Serialize)]
struct Envelope {
    turn: u64,
    #[serde(flatten)]
    reply: Reply,
}

fn emit(app: &AppHandle, turn: u64, reply: Reply) {
    if let Err(error) = app.emit(EVENT, Envelope { turn, reply }) {
        tracing::warn!("assistant: could not emit: {error}");
    }
}

#[derive(Debug)]
struct Exit {
    timed_out: bool,
    stderr: String,
}

static RUNNING: Mutex<Option<std::process::Child>> = Mutex::new(None);

fn with_running<T>(f: impl FnOnce(&mut Option<std::process::Child>) -> T) -> T {
    let mut slot = RUNNING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut slot)
}

struct Handle {
    tracked: bool,
    local: Option<std::process::Child>,
}

impl Handle {
    fn new(child: std::process::Child, tracked: bool) -> Self {
        if tracked {
            with_running(|slot| *slot = Some(child));
            Self {
                tracked,
                local: None,
            }
        } else {
            Self {
                tracked,
                local: Some(child),
            }
        }
    }

    fn with<T>(&mut self, f: impl FnOnce(&mut std::process::Child) -> T) -> Option<T> {
        if self.tracked {
            with_running(|slot| slot.as_mut().map(f))
        } else {
            self.local.as_mut().map(f)
        }
    }

    fn finish(&mut self) -> Option<std::process::Child> {
        if self.tracked {
            with_running(Option::take)
        } else {
            self.local.take()
        }
    }
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_pid(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn kill_tree(child: &mut std::process::Child) {
    kill_pid(child.id());
    let _ = child.kill();
}

#[derive(Debug)]
struct RunError {
    spawn_failed: bool,
    message: String,
}

impl RunError {
    fn spawn(error: std::io::Error) -> Self {
        Self {
            spawn_failed: true,
            message: error.to_string(),
        }
    }

    fn other(message: impl ToString) -> Self {
        Self {
            spawn_failed: false,
            message: message.to_string(),
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(mut child) = self.finish() {
            kill_tree(&mut child);
            let _ = child.wait();
        }
    }
}

fn run_child(
    command: Command,
    prompt: &str,
    timeout: Duration,
    on_line: impl FnMut(&str),
) -> Result<Exit, RunError> {
    run_child_with(command, prompt, timeout, true, on_line)
}

fn run_child_with(
    mut command: Command,
    prompt: &str,
    timeout: Duration,
    tracked: bool,
    mut on_line: impl FnMut(&str),
) -> Result<Exit, RunError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(NO_WINDOW);
    }

    let mut child = command.spawn().map_err(RunError::spawn)?;
    if let Some(mut stdin) = child.stdin.take() {
        let prompt = prompt.to_owned();
        std::thread::spawn(move || {
            let _ = stdin.write_all(prompt.as_bytes());
        });
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut handle = Handle::new(child, tracked);
    let Some(stdout) = stdout else {
        handle.with(kill_tree);
        handle.finish();
        return Err(RunError::other("stdout unavailable"));
    };
    let (lines, incoming) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut raw = Vec::new();
        while matches!(
            (&mut reader).take(MAX_LINE_BYTES).read_until(b'\n', &mut raw),
            Ok(read) if read > 0
        ) {
            let line = String::from_utf8_lossy(&raw).trim_end().to_owned();
            raw.clear();
            if lines.send(line).is_err() {
                break;
            }
        }
    });

    let (collected, drained) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = stderr {
            let _ = pipe.read_to_string(&mut text);
        }
        let _ = collected.send(text);
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    loop {
        match incoming.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => on_line(&line),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timed_out = true;
                handle.with(kill_tree);
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    while !timed_out {
        match handle.with(|child| child.try_wait()) {
            None | Some(Ok(Some(_))) => break,
            Some(Err(error)) => {
                handle.with(kill_tree);
                handle.finish();
                return Err(RunError::other(error));
            }
            Some(Ok(None)) if Instant::now() >= deadline => {
                timed_out = true;
                handle.with(kill_tree);
            }
            Some(Ok(None)) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    if let Some(mut child) = handle.finish() {
        child.wait().map_err(RunError::other)?;
    }
    let stderr = if timed_out {
        String::new()
    } else {
        drained.recv_timeout(STDERR_GRACE).unwrap_or_default()
    };
    Ok(Exit { timed_out, stderr })
}

fn working_dir(base: &Path) -> Result<PathBuf, String> {
    let dir = base.join("assistant");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

fn stop_running_child() {
    with_running(|slot| {
        if let Some(child) = slot.as_mut() {
            kill_tree(child);
        }
    });
}

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn create() -> Result<Self, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "idlewarden-assistant-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&dir).map_err(|error| error.to_string())?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

mod turn;

use turn::{lost_note, Turn};

fn ask(
    app: &AppHandle,
    turn: u64,
    system: &str,
    prompt: &str,
    generation: u64,
) -> Result<Reply, String> {
    let assistant = app.state::<Assistant>();
    let not_found = || {
        "Claude Code introuvable (seule l'installation avec claude.exe est prise en charge). Installe-le et connecte-toi avec `claude` dans un terminal."
            .to_owned()
    };
    let resume = assistant.resume_id();
    let resumed = resume.is_some();
    let scratch = working_dir(
        &app.path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())?,
    )?;
    let system_file = scratch.join("system.md");
    fs::write(&system_file, system).map_err(|error| error.to_string())?;
    let settings_file = scratch.join("settings.json");
    fs::write(&settings_file, SETTINGS).map_err(|error| error.to_string())?;
    let args = build_args(&system_file, &settings_file, resume.as_deref());

    let mut turn_state = Turn::new(resumed);
    let mut on_line = |line: &str| {
        let effects = turn_state.feed(line);
        if effects.kill {
            stop_running_child();
        }
        if let Some(text) = effects.partial {
            emit(app, turn, Reply::Partial { text });
        }
        if let Some(id) = effects.remember {
            assistant.remember(generation, id);
        }
        if effects.forget {
            assistant.forget(generation);
        }
    };

    let started = Instant::now();
    let mut launch = |executable: PathBuf| {
        let mut command = Command::new(executable);
        command.args(&args).current_dir(&scratch);
        let budget = ANSWER_TIMEOUT.saturating_sub(started.elapsed());
        run_child(command, prompt, budget, &mut on_line)
    };
    let executable = locate_claude(&assistant).ok_or_else(not_found)?;
    let exit = match launch(executable) {
        Ok(exit) => exit,
        Err(error) if error.spawn_failed => {
            forget_executable(&assistant);
            let executable = locate_claude(&assistant).ok_or_else(not_found)?;
            launch(executable).map_err(|error| {
                forget_executable(&assistant);
                error.message
            })?
        }
        Err(error) => return Err(error.message),
    };

    if let Some(reply) = turn_state.into_verdict() {
        return Ok(reply);
    }
    if exit.timed_out {
        return Err(format!(
            "Claude n'a pas répondu en {} secondes.",
            ANSWER_TIMEOUT.as_secs()
        ));
    }
    let detail = exit.stderr.trim();
    let note = lost_note(resumed, detail);
    if note.is_some() {
        assistant.forget(generation);
    }
    let note = note.unwrap_or_default();
    if !detail.is_empty() {
        tracing::warn!("assistant: claude stderr: {detail}");
    }
    Err(if detail.is_empty() {
        "Claude s'est arrêté sans répondre. Vérifie que tu es connecté.".to_owned()
    } else {
        format!("{}{note}", shorten(detail))
    })
}

fn shorten(detail: &str) -> String {
    const LIMIT: usize = 300;
    let first_line = detail.lines().next().unwrap_or_default();
    let mut short: String = first_line.chars().take(LIMIT).collect();
    if first_line.chars().nth(LIMIT).is_some() || detail.lines().nth(1).is_some() {
        short.push('…');
    }
    short
}

fn is_resume_failure(detail: &str) -> bool {
    detail.to_lowercase().contains("no conversation found")
}

#[tauri::command]
pub fn assistant_ask(
    app: AppHandle,
    session: State<'_, SessionHandle>,
    assistant: State<'_, Assistant>,
    turn: u64,
    text: String,
    context: String,
    plugin: Option<String>,
) -> Result<(), String> {
    let Some(busy) = Busy::claim(&assistant) else {
        return Err("busy: le Gardien est déjà en train de répondre".to_owned());
    };

    let system = system_prompt(&session.plugin_root(), plugin.as_deref());
    let generation = assistant.generation();
    let prompt = build_prompt(&context, &text);

    std::thread::spawn(move || {
        let _busy = busy;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ask(&app, turn, &system, &prompt, generation)
        }));
        let reply = match outcome {
            Ok(Ok(reply)) => reply,
            Ok(Err(message)) => Reply::Failed { message },
            Err(_) => Reply::Failed {
                message: "Le Gardien a rencontré une erreur interne.".to_owned(),
            },
        };
        drop(_busy);
        emit(&app, turn, reply);
    });
    Ok(())
}

pub fn shutdown(assistant: &Assistant) {
    stop_running_child();
    assistant.reset();
}

#[tauri::command]
pub fn assistant_cancel() {
    stop_running_child();
}

#[tauri::command]
pub fn assistant_reset(assistant: State<'_, Assistant>) -> Result<(), String> {
    let Some(_claim) = Busy::claim(&assistant) else {
        return Err("le Gardien est en train de répondre".to_owned());
    };
    assistant.reset();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static SUBPROCESS: Mutex<()> = Mutex::new(());

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
        let first = build_args(Path::new("system.md"), Path::new("settings.json"), None);
        assert!(!first.contains(&"--resume".to_owned()));
        let next = build_args(
            Path::new("system.md"),
            Path::new("settings.json"),
            Some("abc"),
        );
        assert_eq!(&next[next.len() - 2..], ["--resume", "abc"]);
    }

    #[test]
    fn the_assistant_has_no_tools_no_mcp_servers_and_no_project_settings() {
        let args = build_args(Path::new("system.md"), Path::new("settings.json"), None);
        let at = args
            .iter()
            .position(|arg| arg == "--tools")
            .expect("a --tools flag");
        assert_eq!(args[at + 1], "");
        assert!(args.contains(&"--strict-mcp-config".to_owned()));
        assert!(args.contains(&"--disable-slash-commands".to_owned()));
        let sources = args
            .iter()
            .position(|arg| arg == "--setting-sources")
            .expect("a setting sources flag");
        assert_eq!(args[sources + 1], "user");
        let mode = args
            .iter()
            .position(|arg| arg == "--permission-mode")
            .expect("a permission mode flag");
        assert_eq!(args[mode + 1], "plan");
        let denied = args
            .iter()
            .position(|arg| arg == "--disallowed-tools")
            .expect("a disallowed tools flag");
        assert!(args[denied + 1].contains("Bash"));
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

    #[test]
    fn a_session_id_arriving_after_a_reset_is_dropped() {
        let assistant = Assistant::default();
        let started = assistant.generation();
        assistant.remember(started, "first".to_owned());
        assert_eq!(assistant.resume_id().as_deref(), Some("first"));

        assistant.reset();
        assert_eq!(assistant.resume_id(), None);
        assistant.remember(started, "late".to_owned());
        assert_eq!(assistant.resume_id(), None);
        assistant.remember(assistant.generation(), "next".to_owned());
        assert_eq!(assistant.resume_id().as_deref(), Some("next"));
    }

    #[test]
    fn a_second_request_cannot_claim_the_assistant_until_the_first_is_dropped() {
        let assistant = Assistant::default();
        let first = Busy::claim(&assistant).expect("the first claim");
        assert!(Busy::claim(&assistant).is_none());
        drop(first);
        assert!(Busy::claim(&assistant).is_some());
    }

    #[test]
    fn a_failed_turn_forgets_the_session_but_a_stale_one_cannot() {
        let assistant = Assistant::default();
        let generation = assistant.generation();
        assistant.remember(generation, "id".to_owned());
        assistant.reset();
        assistant.remember(assistant.generation(), "new".to_owned());
        assistant.forget(generation);
        assert_eq!(assistant.resume_id().as_deref(), Some("new"));
        assistant.forget(assistant.generation());
        assert_eq!(assistant.resume_id(), None);
    }

    #[test]
    fn a_context_of_exactly_the_cap_is_not_marked_as_truncated() {
        let exact = "x".repeat(MAX_CONTEXT_CHARS);
        assert!(!build_prompt(&exact, "q").contains("truncated"));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_line_with_invalid_utf8_does_not_cut_the_stream() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut seen = Vec::new();
        run_child(
            shell("printf 'a\\n\\377\\nb\\n'"),
            "",
            Duration::from_secs(20),
            |line| seen.push(line.to_owned()),
        )
        .expect("run");
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[2], "b");
    }

    #[test]
    #[ignore = "needs a local Claude Code install"]
    fn the_real_cli_receives_the_empty_tools_argument_and_reports_none() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let executable = find_claude().expect("a local Claude Code install");
        let scratch = ScratchDir::create().expect("scratch");
        let system_file = scratch.path().join("system.md");
        let settings_file = scratch.path().join("settings.json");
        fs::write(&system_file, PERSONA).expect("system");
        fs::write(&settings_file, SETTINGS).expect("settings");
        let mut command = Command::new(executable);
        command
            .args(build_args(&system_file, &settings_file, None))
            .current_dir(scratch.path());
        let mut turn = Turn::new(false);
        let mut kills = 0;
        let mut saw_empty_tools = false;
        run_child(command, "bonjour", Duration::from_secs(90), |line| {
            saw_empty_tools |= init_tools(line) == InitTools::Reported(Vec::new());
            if turn.feed(line).kill {
                kills += 1;
                stop_running_child();
            }
        })
        .expect("run");
        assert_eq!(
            kills,
            0,
            "the CLI reported tools: {:?}",
            turn.into_verdict()
        );
        assert!(
            saw_empty_tools,
            "no init line with an empty tool list was seen"
        );
    }

    #[test]
    fn the_working_directory_is_one_stable_app_owned_folder() {
        let base = scratch("workdir");
        let first = working_dir(&base).expect("first");
        assert_eq!(working_dir(&base).expect("second"), first);
        assert!(first.is_dir());
        assert!(first.starts_with(&base));
    }

    #[cfg(windows)]
    #[test]
    fn a_child_reporting_tools_is_killed_through_the_turn_on_windows() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let scratch = ScratchDir::create().expect("scratch");
        let script = scratch.path().join("stub.ps1");
        fs::write(
            &script,
            "Write-Output '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s\",\"tools\":[\"Bash\"]}'\r\n[Console]::Out.Flush()\r\nStart-Sleep -Seconds 30\r\n",
        )
        .expect("script");
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
        command.arg(&script);
        let mut turn = Turn::new(false);
        let started = Instant::now();
        let exit = run_child(command, "", Duration::from_secs(25), |line| {
            if turn.feed(line).kill {
                stop_running_child();
            }
        })
        .expect("run");
        assert!(!exit.timed_out);
        assert!(started.elapsed() < Duration::from_secs(20));
        assert!(matches!(
            turn.into_verdict(),
            Some(Reply::Failed { message }) if message.contains("Bash")
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_child_reporting_tools_is_killed_through_the_turn() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let init = r#"{"type":"system","subtype":"init","session_id":"s","tools":["Bash"]}"#;
        let script = format!("printf '%s\\n' '{init}'; exec sleep 30");
        let mut turn = Turn::new(false);
        let started = Instant::now();
        let exit = run_child(shell(&script), "", Duration::from_secs(20), |line| {
            if turn.feed(line).kill {
                stop_running_child();
            }
        })
        .expect("run");
        assert!(!exit.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(matches!(
            turn.into_verdict(),
            Some(Reply::Failed { message }) if message.contains("Bash")
        ));
    }

    #[test]
    fn a_pre_release_suffix_does_not_make_a_version_older() {
        assert_eq!(
            compare_versions("1.2.300-beta", "1.2.299"),
            Ordering::Greater
        );
        assert_eq!(compare_versions("1.2.3", "1.2.3-rc1"), Ordering::Equal);
    }

    #[test]
    fn the_init_line_reports_its_tools_or_fails_closed() {
        let init = r#"{"type":"system","subtype":"init","session_id":"a","tools":["Bash","Read"]}"#;
        assert_eq!(
            init_tools(init),
            InitTools::Reported(vec!["Bash".to_owned(), "Read".to_owned()])
        );
        let empty = r#"{"type":"system","subtype":"init","session_id":"a","tools":[]}"#;
        assert_eq!(init_tools(empty), InitTools::Reported(Vec::new()));
        let missing = r#"{"type":"system","subtype":"init","session_id":"a"}"#;
        assert_eq!(init_tools(missing), InitTools::Unreported);
        assert_eq!(
            init_tools(r#"{"type":"assistant","tools":["Bash"]}"#),
            InitTools::NotInit
        );
        assert_eq!(init_tools("not json"), InitTools::NotInit);
    }

    #[test]
    fn hooks_are_disabled_through_a_settings_file() {
        let value: serde_json::Value = serde_json::from_str(SETTINGS).expect("json");
        assert_eq!(value["disableAllHooks"], true);
        let args = build_args(Path::new("system.md"), Path::new("settings.json"), None);
        let at = args
            .iter()
            .position(|arg| arg == "--settings")
            .expect("a --settings flag");
        assert_eq!(args[at + 1], "settings.json");
    }

    #[test]
    fn a_long_stderr_is_cut_for_the_chat() {
        let long = "e".repeat(1000);
        let short = shorten(&long);
        assert_eq!(short.chars().count(), 301);
        assert!(short.ends_with('…'));
        assert_eq!(shorten("court"), "court");
    }

    #[test]
    fn only_a_failed_resume_drops_the_conversation() {
        assert!(is_resume_failure(
            "Error: No conversation found with session ID: abc"
        ));
        assert!(!is_resume_failure("API Error: 529 overloaded"));
        assert!(!is_resume_failure(""));
    }

    #[test]
    fn an_oversized_context_is_marked_as_truncated() {
        let big = "x".repeat(MAX_CONTEXT_CHARS + 10);
        assert!(build_prompt(&big, "q").contains("[signals truncated]"));
        assert!(!build_prompt("[]", "q").contains("truncated"));
    }

    #[test]
    fn the_claim_survives_being_moved_into_the_worker_thread() {
        let assistant = Assistant::default();
        let claim = Busy::claim(&assistant).expect("the first claim");
        let worker = std::thread::spawn(move || {
            let _held = claim;
        });
        worker.join().expect("worker");
        assert!(Busy::claim(&assistant).is_some());

        let held = Busy::claim(&assistant).expect("claim");
        let still_held = std::thread::spawn(move || held).join().expect("worker");
        assert!(Busy::claim(&assistant).is_none());
        drop(still_held);
    }

    #[test]
    fn a_plugin_prompt_past_the_size_cap_is_refused() {
        let root = scratch("cap");
        let folder = root.join("game");
        fs::create_dir_all(&folder).expect("plugin dir");
        fs::write(folder.join("plugin.json"), r#"{"id":"dev.example.game"}"#).expect("manifest");
        fs::write(
            folder.join("assistant.md"),
            "x".repeat(MAX_PLUGIN_PROMPT_BYTES as usize + 1),
        )
        .expect("prompt");
        assert_eq!(plugin_prompt(&root, "dev.example.game"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_plugin_prompt_that_is_a_symlink_is_refused() {
        let root = scratch("link");
        let folder = root.join("game");
        fs::create_dir_all(&folder).expect("plugin dir");
        fs::write(folder.join("plugin.json"), r#"{"id":"dev.example.game"}"#).expect("manifest");
        let secret = root.join("secret.txt");
        fs::write(&secret, "private").expect("secret");
        std::os::unix::fs::symlink(&secret, folder.join("assistant.md")).expect("symlink");
        assert_eq!(plugin_prompt(&root, "dev.example.game"), None);
    }

    #[cfg(windows)]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("cmd");
        command.args(["/C", script]);
        command
    }

    #[cfg(not(windows))]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    #[cfg(not(windows))]
    #[test]
    fn a_child_that_closes_stdout_but_keeps_running_is_killed_at_the_deadline() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let started = Instant::now();
        let exit = run_child(
            shell("exec 1>&-; sleep 30"),
            "",
            Duration::from_millis(300),
            |_| {},
        )
        .expect("run");
        assert!(exit.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn output_lines_are_delivered_in_order() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        #[cfg(windows)]
        let script = "echo one& echo two";
        #[cfg(not(windows))]
        let script = "echo one; echo two";

        let mut seen = Vec::new();
        let exit = run_child(shell(script), "prompt", Duration::from_secs(20), |line| {
            seen.push(line.trim().to_owned());
        })
        .expect("the child runs");
        assert_eq!(seen, ["one", "two"]);
        assert!(!exit.timed_out);
    }

    #[test]
    fn a_child_that_never_answers_is_killed_at_the_deadline() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        #[cfg(windows)]
        let script = "ping -n 30 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let script = "sleep 30";

        let started = Instant::now();
        let exit = run_child(shell(script), "", Duration::from_millis(300), |_| {})
            .expect("the child runs");
        assert!(exit.timed_out);
        assert!(started.elapsed() < Duration::from_secs(15));
    }

    #[test]
    fn a_child_filling_stderr_does_not_block_the_reader() {
        let _serial = SUBPROCESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        #[cfg(windows)]
        let script = "for /L %i in (1,1,20000) do @echo xxxxxxxxxxxxxxxxxxxxxxxx 1>&2";
        #[cfg(not(windows))]
        let script =
            "i=0; while [ $i -lt 20000 ]; do echo xxxxxxxxxxxxxxxxxxxxxxxx >&2; i=$((i+1)); done";

        let exit =
            run_child(shell(script), "", Duration::from_secs(60), |_| {}).expect("the child runs");
        assert!(!exit.timed_out);
        assert!(exit.stderr.len() > 100_000);
    }
}
