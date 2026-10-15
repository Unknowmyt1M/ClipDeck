/**
 * Typed frontend ↔ Tauri command bridge.
 * All `invoke()` calls live here; components use these functions only.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AiAction,
  AiModelInfo,
  AiStatus,
  ClipboardItem,
  ClipboardItemType,
  ClearResult,
  ExportResult,
  Settings,
  Stats,
  WipeResult,
} from "../types";

export type HistoryFilter = ClipboardItemType | "pinned" | "favorites" | "today" | "week";

// ---------- clipboard ----------

export function getItems(opts?: {
  typeFilter?: string | null;
  query?: string | null;
  limit?: number;
}): Promise<ClipboardItem[]> {
  return invoke<ClipboardItem[]>("get_items", {
    typeFilter: opts?.typeFilter ?? null,
    query: opts?.query?.trim() || null,
    limit: opts?.limit ?? 200,
  });
}

export function restoreItem(id: number): Promise<ClipboardItem> {
  return invoke<ClipboardItem>("restore_item", { id });
}

export function setItemPinned(id: number, pinned: boolean): Promise<ClipboardItem> {
  return invoke<ClipboardItem>("set_item_pinned", { id, pinned });
}

export function setItemFavorite(id: number, favorite: boolean): Promise<ClipboardItem> {
  return invoke<ClipboardItem>("set_item_favorite", { id, favorite });
}

export function deleteItem(id: number): Promise<boolean> {
  return invoke<boolean>("delete_item", { id });
}

export function clearHistory(): Promise<ClearResult> {
  return invoke<ClearResult>("clear_history");
}

/** Base64 data URL of the item's image (thumbnail by default). */
export function getItemImage(id: number, thumb = true): Promise<string | null> {
  return invoke<string | null>("get_item_image", { id, thumb });
}

export function getItemCount(): Promise<number> {
  return invoke<number>("get_item_count");
}

/** Listen for clipboard history change events pushed from Rust. */
export function onClipboardChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("clipboard-changed", cb);
}

// ---------- settings + privacy ----------

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function updateSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("update_settings", { settings });
}

export function setStartupEnabled(enabled: boolean): Promise<Settings> {
  return invoke<Settings>("set_startup_enabled", { enabled });
}

/** Store the OpenRouter key in the OS credential vault. */
export function setApiKey(key: string): Promise<boolean> {
  return invoke<boolean>("set_api_key", { key });
}

/** Only a boolean crosses IPC — the key itself never comes back. */
export function hasApiKey(): Promise<boolean> {
  return invoke<boolean>("has_api_key");
}

export function clearApiKey(): Promise<boolean> {
  return invoke<boolean>("clear_api_key");
}

export function getStats(): Promise<Stats> {
  return invoke<Stats>("get_stats");
}

export function exportHistory(): Promise<ExportResult> {
  return invoke<ExportResult>("export_history");
}

export function wipeData(): Promise<WipeResult> {
  return invoke<WipeResult>("wipe_data");
}

// ---------- AI ----------

export function aiListModels(): Promise<AiModelInfo[]> {
  return invoke<AiModelInfo[]>("ai_list_models");
}

export function aiTestConnection(): Promise<void> {
  return invoke<void>("ai_test_connection");
}

export function aiGetStatus(): Promise<AiStatus> {
  return invoke<AiStatus>("ai_get_status");
}

export function aiAnalyzeItem(id: number): Promise<void> {
  return invoke<void>("ai_analyze_item", { id });
}

export function aiAnalyzePending(): Promise<void> {
  return invoke<void>("ai_analyze_pending");
}

export function aiTransform(
  id: number,
  action: AiAction,
  targetLang?: string,
): Promise<string> {
  return invoke<string>("ai_transform", { id, action, targetLang: targetLang ?? null });
}

export function onAiStatus(cb: (status: AiStatus) => void): Promise<UnlistenFn> {
  return listen<AiStatus>("ai-status", (e) => cb(e.payload));
}

export function onAiItem(cb: (payload: { id: number; done: boolean }) => void): Promise<UnlistenFn> {
  return listen<{ id: number; done: boolean }>("ai-item-analyzed", (e) => cb(e.payload));
}

// ---------- windows ----------

export function toggleQuickWindow(): Promise<void> {
  return invoke<void>("toggle_quick_window");
}

export function hideQuickWindow(): Promise<void> {
  return invoke<void>("hide_quick_window");
}

export function showMainWindow(): Promise<void> {
  return invoke<void>("show_main_window");
}
