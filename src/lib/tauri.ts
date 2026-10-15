/**
 * Typed frontend ↔ Tauri command bridge.
 * All `invoke()` calls live here; components use these functions only.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ClipboardItem, ClearResult, ClipboardItemType } from "../types";

export function getItems(opts?: {
  typeFilter?: ClipboardItemType | "pinned" | "favorites" | null;
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
