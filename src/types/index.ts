/** Shared frontend/domain types for ClipDeck. */

export type ClipboardItemType = "text" | "url" | "image";

export interface ClipboardItem {
  id: number;
  uuid: string;
  type: ClipboardItemType;
  content: string;
  content_hash: string;
  preview: string;
  created_at: number;
  updated_at: number;
  last_used_at: number | null;
  use_count: number;
  is_pinned: boolean;
  is_favorite: boolean;
  is_archived: boolean;
  is_sensitive: boolean;
  /** AI summary, present once the item was analyzed. */
  ai_summary: string | null;
  /** AI category label (Code, Link, Text, …). */
  ai_category: string | null;
  /** AI keywords. */
  ai_keywords: string[];
  /** Linked tag names (manual + AI). */
  tags: string[];
}

export type ItemFilter =
  | "all"
  | ClipboardItemType
  | "pinned"
  | "favorites"
  | "today"
  | "week";

export interface ClearResult {
  deleted: number;
}

/** App settings (mirrors the Rust `Settings` struct). */
export interface Settings {
  theme: "system" | "light" | "dark";
  startup: boolean;
  global_hotkey: string;
  history_limit: number;
  close_to_tray: boolean;
  pause_monitoring: boolean;
  sensitive_detection: boolean;
  skip_sensitive: boolean;
  /** JSON array of excluded executables (e.g. "keepass.exe"). */
  excluded_apps: string;
  /** JSON array of excluded window-title substrings. */
  excluded_titles: string;
  retention_days: number;
  ai_enabled: boolean;
  ai_provider: string;
  ai_model: string;
  ai_model_name: string;
  ai_free: boolean;
  ai_auto_analyze: boolean;
  ai_allow_general: boolean;
}

/** One OpenRouter model, classified free vs paid. */
export interface AiModelInfo {
  id: string;
  name: string;
  free: boolean;
  /** USD per 1M prompt tokens. */
  prompt_price: number;
  /** USD per 1M completion tokens. */
  completion_price: number;
  context_length: number | null;
}

export interface AiStatus {
  busy: boolean;
  queued: number;
  last_error: string | null;
  analyzed: number;
  skipped: number;
}

export interface TopItem {
  id: number;
  preview: string;
  use_count: number;
  item_type: string;
}

export interface Stats {
  total: number;
  today: number;
  pinned: number;
  favorites: number;
  sensitive: number;
  images: number;
  ai_processed: number;
  top_used: TopItem[];
  oldest_at: number | null;
}

export interface ExportResult {
  path: string;
  items: number;
}

export interface WipeResult {
  deleted_rows: number;
  deleted_images: number;
}

export type AiAction = "summarize" | "rewrite" | "translate" | "explain" | "fixgrammar";
