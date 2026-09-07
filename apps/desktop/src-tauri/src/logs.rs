// SPDX-License-Identifier: MPL-2.0
//! The `tracing` stream, kept in memory so the app can show it (#15).
//!
//! A packaged desktop app has no console, so structured logs that only reach
//! stdout reach nobody. This buffers them instead, and keeps each field as a
//! typed JSON value rather than folding everything into one formatted line:
//! flattening on the way out is exactly what makes a structured log stop being
//! worth having.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::State;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Records are drained by the UI, and nothing guarantees the UI is polling.
const MAX_BUFFERED_LOGS: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
pub struct LogRecord {
    pub at_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Default)]
pub struct LogBuffer(Mutex<VecDeque<LogRecord>>);

impl LogBuffer {
    fn push(&self, record: LogRecord) {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if held.len() == MAX_BUFFERED_LOGS {
            held.pop_front();
        }
        held.push_back(record);
    }

    pub fn drain(&self) -> Vec<LogRecord> {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        held.drain(..).collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or_default()
}

/// Collects an event's fields, keeping their types. `message` is pulled out
/// because it is the one field every log line has and the UI renders on its
/// own row.
#[derive(Default)]
struct Collect {
    message: String,
    fields: BTreeMap<String, Value>,
}

impl Collect {
    fn put(&mut self, field: &Field, value: Value) {
        if field.name() == "message" {
            self.message = match value {
                Value::String(text) => text,
                other => other.to_string(),
            };
            return;
        }
        self.fields.insert(field.name().to_owned(), value);
    }
}

impl Visit for Collect {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, Value::from(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, Value::String(value.to_owned()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, Value::String(format!("{value:?}")));
    }
}

pub struct BufferLayer(Arc<LogBuffer>);

impl BufferLayer {
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        BufferLayer(buffer)
    }
}

impl<S: Subscriber> Layer<S> for BufferLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut collect = Collect::default();
        event.record(&mut collect);

        self.0.push(LogRecord {
            at_ms: now_ms(),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_owned(),
            message: collect.message,
            fields: collect.fields,
        });
    }
}

/// Everything buffered since the last call. Drains for the same reason the
/// event stream does: the app accumulates its own history and the buffer here
/// only has to survive between polls.
#[tauri::command]
pub fn drain_logs(buffer: State<'_, Arc<LogBuffer>>) -> Vec<LogRecord> {
    buffer.drain()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::prelude::*;

    fn capture(emit: impl FnOnce()) -> Vec<LogRecord> {
        let buffer = Arc::new(LogBuffer::default());
        let subscriber = tracing_subscriber::registry().with(BufferLayer::new(buffer.clone()));
        tracing::subscriber::with_default(subscriber, emit);
        buffer.drain()
    }

    #[test]
    fn fields_keep_their_types_instead_of_being_folded_into_the_message() {
        let records = capture(|| {
            tracing::info!(
                plugin = "example-game",
                actions = 3u64,
                dry_run = true,
                "started"
            );
        });

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.message, "started");
        assert_eq!(record.level, "INFO");
        assert_eq!(
            record.fields["plugin"],
            Value::String("example-game".to_owned())
        );
        assert_eq!(
            record.fields["actions"],
            Value::from(3u64),
            "a number that arrives as a string cannot be filtered or compared"
        );
        assert_eq!(record.fields["dry_run"], Value::Bool(true));
    }

    #[test]
    fn the_target_is_kept_so_the_ui_can_filter_by_it() {
        let records = capture(|| {
            tracing::warn!(target: "idlewarden::governor", "rate ceiling reached");
        });

        assert_eq!(records[0].target, "idlewarden::governor");
        assert_eq!(records[0].level, "WARN");
    }

    #[test]
    fn the_buffer_drops_the_oldest_rather_than_growing_without_bound() {
        let buffer = LogBuffer::default();

        for index in 0..MAX_BUFFERED_LOGS + 10 {
            buffer.push(LogRecord {
                at_ms: 0,
                level: "INFO".to_owned(),
                target: "t".to_owned(),
                message: index.to_string(),
                fields: BTreeMap::new(),
            });
        }

        let drained = buffer.drain();

        assert_eq!(drained.len(), MAX_BUFFERED_LOGS);
        assert_eq!(
            drained[0].message, "10",
            "the oldest records are the ones dropped"
        );
    }

    #[test]
    fn draining_twice_does_not_repeat_records() {
        let buffer = Arc::new(LogBuffer::default());
        let subscriber = tracing_subscriber::registry().with(BufferLayer::new(buffer.clone()));
        tracing::subscriber::with_default(subscriber, || tracing::info!("once"));

        assert_eq!(buffer.drain().len(), 1);
        assert!(
            buffer.drain().is_empty(),
            "a second poll must not replay what the first already took"
        );
    }
}
