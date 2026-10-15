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
}

export type ItemFilter = "all" | ClipboardItemType | "pinned" | "favorites";

export interface ClearResult {
  deleted: number;
}
