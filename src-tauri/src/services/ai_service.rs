//! AI orchestration: background analysis queue + one-shot transforms.
//!
//! Hard rules enforced here:
//! - Sensitive content is never sent to the provider (checked twice).
//! - Nothing is logged with clipboard content; only ids/status.
//! - All provider access goes through `AiProvider`.

use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ai::openrouter::OpenRouterProvider;
use crate::ai::provider::AiProvider;
use crate::models::ai::{AiAnalysis, AiModelInfo};
use crate::security::{secrets, sensitive_detector};
use crate::services::clipboard_service::CLIPBOARD_CHANGED_EVENT;
use crate::services::database_service::DatabaseService;
use crate::services::settings_service::SettingsService;

/// Emitted whenever queue/busy/last-error state changes.
pub const AI_STATUS_EVENT: &str = "ai-status";
/// Emitted after one item finished AI analysis (or was skipped).
pub const AI_ITEM_EVENT: &str = "ai-item-analyzed";

/// Content length bounds for auto-analysis (cheapest possible spend).
const MIN_ANALYZE_CHARS: usize = 12;
const MAX_ANALYZE_CHARS: usize = 6000;
/// How many pending items are discovered per "Analyze pending" job.
const PENDING_BATCH: i64 = 25;

#[derive(Clone, Debug)]
enum AiJob {
    Analyze(i64),
    AnalyzePending,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub busy: bool,
    pub queued: usize,
    pub last_error: Option<String>,
    pub analyzed: u64,
    pub skipped: u64,
}

/// Handle shared across the app; owns the worker channel + status cache.
pub struct AiService {
    tx: tokio::sync::mpsc::UnboundedSender<AiJob>,
    status: Arc<Mutex<AiStatus>>,
    provider: OpenRouterProvider,
}

impl AiService {
    pub fn start(db: Arc<DatabaseService>, settings: Arc<SettingsService>) -> Arc<Self> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AiJob>();
        let status = Arc::new(Mutex::new(AiStatus {
            busy: false,
            queued: 0,
            last_error: None,
            analyzed: 0,
            skipped: 0,
        }));

        let service = Arc::new(Self {
            tx,
            status: Arc::clone(&status),
            provider: OpenRouterProvider::new(),
        });

        let worker = Arc::clone(&service);
        tauri::async_runtime::spawn(async move {
            while let Some(job) = rx.recv().await {
                worker.set_busy(true);
                let result = match job {
                    AiJob::Analyze(id) => worker.analyze_one(&db, &settings, id).await,
                    AiJob::AnalyzePending => {
                        worker.enqueue_pending_ids(&db);
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    worker.set_error(&e);
                }
                worker.set_busy(false);
            }
        });

        service
    }

    /// Queue analysis for one item.
    pub fn enqueue_analyze(&self, id: i64) {
        self.push(AiJob::Analyze(id));
    }

    pub fn status(&self) -> AiStatus {
        self.status.lock().expect("ai status poisoned").clone()
    }

    /// Fetch the OpenRouter catalog (free models first) for the picker.
    pub async fn list_models(&self) -> Result<Vec<AiModelInfo>, String> {
        self.provider.list_models().await
    }

    /// Connectivity probe (key + reachability), never sends content.
    pub async fn health_check(&self) -> Result<(), String> {
        self.provider.health_check().await
    }

    /// Interactive one-shot completion (rewrite/translate/explain/…).
    pub async fn transform(&self, model: &str, system: &str, user: &str) -> Result<String, String> {
        self.provider.complete(model, system, user).await
    }

    /// Manual "analyze everything not yet analyzed" button.
    pub fn enqueue_pending(&self) {
        self.push(AiJob::AnalyzePending);
    }

    // ----- worker internals -----

    fn push(&self, job: AiJob) {
        if self.tx.send(job).is_ok() {
            self.status.lock().expect("ai status poisoned").queued += 1;
            self.emit_status();
        }
    }

    /// Discover eligible items and queue them individually so the queue
    /// counter stays accurate.
    fn enqueue_pending_ids(&self, db: &DatabaseService) {
        let ids = match db.pending_ai_items(PENDING_BATCH) {
            Ok(ids) => ids,
            Err(e) => {
                self.set_error(&e);
                return;
            }
        };
        for id in ids {
            let _ = self.tx.send(AiJob::Analyze(id));
            self.status.lock().expect("ai status poisoned").queued += 1;
        }
        self.emit_status();
    }

    async fn analyze_one(
        &self,
        db: &DatabaseService,
        settings: &SettingsService,
        id: i64,
    ) -> Result<(), String> {
        self.bump_queued_start();

        let s = settings.get();
        if !s.ai_enabled {
            self.bump_skipped();
            return Ok(());
        }
        let model = s.ai_model.clone();
        if model.is_empty() {
            self.set_error("No AI model selected — pick one in Settings → AI.");
            return Ok(());
        }
        if !secrets::has_openrouter_api_key() {
            self.set_error("No API key configured — add it in Settings → AI.");
            return Ok(());
        }

        let item = db
            .get_item(id)?
            .ok_or_else(|| format!("item {id} missing for analysis"))?;

        // Privacy gate: flagged-sensitive or any detected secret pattern.
        if item.is_sensitive || sensitive_detector::detect(&item.content).is_some() {
            self.bump_skipped();
            self.emit_item(id, false);
            return Ok(());
        }
        if item.item_type == crate::models::clipboard::ClipboardItemType::Image {
            self.bump_skipped();
            self.emit_item(id, false);
            return Ok(());
        }
        let len = item.content.chars().count();
        if !(MIN_ANALYZE_CHARS..=MAX_ANALYZE_CHARS).contains(&len) {
            self.bump_skipped();
            self.emit_item(id, false);
            return Ok(());
        }

        let analysis: AiAnalysis = self.provider.analyze(&model, &item.content).await?;
        db.save_ai_metadata(id, "openrouter", &model, &analysis)?;
        db.add_ai_tags(id, &analysis.tags)?;

        self.bump_analyzed();
        self.emit_item(id, true);
        Ok(())
    }

    // ----- status plumbing -----

    fn set_busy(&self, busy: bool) {
        self.status.lock().expect("ai status poisoned").busy = busy;
        self.emit_status();
    }

    fn set_error(&self, msg: &str) {
        // Error strings never contain clipboard content (constructed from
        // provider/infra errors only).
        let mut s = self.status.lock().expect("ai status poisoned");
        s.last_error = Some(msg.chars().take(300).collect());
        drop(s);
        self.emit_status();
    }

    fn bump_queued_start(&self) {
        {
            let mut s = self.status.lock().expect("ai status poisoned");
            s.queued = s.queued.saturating_sub(1);
        }
        self.emit_status();
    }

    fn bump_analyzed(&self) {
        let mut s = self.status.lock().expect("ai status poisoned");
        s.analyzed += 1;
        s.last_error = None;
        drop(s);
        self.emit_status();
    }

    fn bump_skipped(&self) {
        self.status.lock().expect("ai status poisoned").skipped += 1;
        self.emit_status();
    }

    fn emit_status(&self) {
        if let Some(app) = app_handle() {
            let _ = app.emit(AI_STATUS_EVENT, self.status());
        }
    }

    fn emit_item(&self, id: i64, done: bool) {
        if let Some(app) = app_handle() {
            let _ = app.emit(AI_ITEM_EVENT, serde_json::json!({ "id": id, "done": done }));
            // List rows gain an AI badge → refresh main view cheaply.
            let _ = app.emit(CLIPBOARD_CHANGED_EVENT, ());
        }
    }
}

// The worker needs an AppHandle without threading it through every call;
// Tauri apps are single-instance processes, so a global slot is safe.
static APP_SLOT: OnceLock<Mutex<Option<AppHandle>>> = OnceLock::new();

pub fn set_app_handle(app: AppHandle) {
    let slot = APP_SLOT.get_or_init(|| Mutex::new(None));
    *slot.lock().expect("app slot poisoned") = Some(app);
}

fn app_handle() -> Option<AppHandle> {
    APP_SLOT
        .get()
        .and_then(|s| s.lock().expect("app slot poisoned").clone())
}

/// Build the system prompt for a user-triggered transform action.
pub fn transform_system(action: &str, target_lang: Option<&str>) -> Result<String, String> {
    let base = match action {
        "rewrite" => "Rewrite the user's text to be clearer and more natural. Preserve meaning, code, links and formatting. Reply with ONLY the rewritten text.".to_string(),
        "translate" => {
            let lang = target_lang.unwrap_or("English");
            format!("Translate the user's text into {lang}. Preserve code, links and formatting. Reply with ONLY the translation.")
        }
        "explain" => "Explain what the user's text is about in 2-4 short sentences, plain language. Reply with ONLY the explanation.".to_string(),
        "summarize" => "Summarize the user's text in at most 2 sentences. Reply with ONLY the summary.".to_string(),
        "fixgrammar" => "Fix grammar and spelling in the user's text without changing its meaning or language. Reply with ONLY the corrected text.".to_string(),
        other => return Err(format!("unknown AI action: {other}")),
    };
    Ok(base)
}
