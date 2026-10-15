import type { ClipboardItem } from "../../types";
import { cn, formatRelativeTime, truncate } from "../../lib/utils";
import { ImageThumb } from "./ImageThumb";

interface ClipboardCardProps {
  item: ClipboardItem;
  selected: boolean;
  onSelect: () => void;
  onRestore: () => void;
  onTogglePin: () => void;
  onToggleFavorite: () => void;
  onDelete: () => void;
  /** Opens the AI actions modal for this clip (optional). */
  onAi?: () => void;
}

const TYPE_BADGE: Record<ClipboardItem["type"], string> = {
  text: "Text",
  url: "URL",
  image: "Image",
};

/** One row in the clipboard history list. */
export function ClipboardCard({
  item,
  selected,
  onSelect,
  onRestore,
  onTogglePin,
  onToggleFavorite,
  onDelete,
  onAi,
}: ClipboardCardProps) {
  const timestamp = item.last_used_at ?? item.created_at;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onDoubleClick={onRestore}
      onKeyDown={(e) => {
        if (e.key === "Enter") onRestore();
      }}
      className={cn(
        "group cursor-pointer rounded-lg border p-3 transition-colors",
        selected
          ? "border-ring bg-accent"
          : "border-border bg-card hover:border-ring/50",
      )}
    >
      <div className="flex items-start gap-3">
        {item.type === "image" ? (
          <ImageThumb id={item.id} />
        ) : (
          <span className="mt-0.5 inline-flex h-6 shrink-0 items-center rounded bg-muted px-1.5 text-[10px] font-medium text-muted-foreground">
            {TYPE_BADGE[item.type]}
          </span>
        )}

        <div className="min-w-0 flex-1 selectable">
          {item.type === "url" ? (
            <a
              href={item.content}
              target="_blank"
              rel="noreferrer"
              onClick={(e) => e.stopPropagation()}
              className="block truncate text-sm text-blue-500 hover:underline"
            >
              {item.preview || truncate(item.content, 200)}
            </a>
          ) : (
            <p className="text-sm break-words whitespace-pre-wrap text-foreground">
              {item.type === "image" ? item.preview : item.preview || truncate(item.content, 200)}
            </p>
          )}

          {/* AI summary (written by the background analysis queue). */}
          {item.ai_summary && (
            <p className="mt-1 text-xs break-words text-muted-foreground italic">
              ✦ {item.ai_summary}
            </p>
          )}

          {/* Tags + AI category + sensitive flag. */}
          {(item.tags.length > 0 || item.ai_category || item.is_sensitive) && (
            <div className="mt-1.5 flex flex-wrap items-center gap-1">
              {item.is_sensitive && (
                <span className="rounded bg-destructive/15 px-1.5 py-0.5 text-[10px] font-medium text-destructive">
                  sensitive
                </span>
              )}
              {item.ai_category && (
                <span className="rounded bg-accent px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
                  {item.ai_category}
                </span>
              )}
              {item.tags.slice(0, 4).map((tag) => (
                <span
                  key={tag}
                  className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                >
                  #{tag}
                </span>
              ))}
            </div>
          )}

          <div className="mt-1.5 flex items-center gap-2 text-[11px] text-muted-foreground">
            <span>{formatRelativeTime(timestamp)}</span>
            {item.use_count > 1 && <span>· used {item.use_count}×</span>}
          </div>
        </div>

        {/* Actions: visible on hover/selection to keep the list calm. */}
        <div
          className={cn(
            "flex shrink-0 items-center gap-0.5 transition-opacity",
            selected ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          <ActionButton
            label={item.is_pinned ? "Unpin" : "Pin"}
            active={item.is_pinned}
            onClick={onTogglePin}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M11.48 3.5a.56.56 0 011.04 0l2.12 5.12a2.5 2.5 0 001.94 1.53l5.57.48a.56.56 0 01.31.95l-4.22 3.62a2.5 2.5 0 00-.78 2.34l1.27 5.44a.56.56 0 01-.81.59L12.4 20.6a2.5 2.5 0 00-2.35 0l-4.73 2.37a.56.56 0 01-.8-.6l1.26-5.43a2.5 2.5 0 00-.79-2.35l-4.22-3.62a.56.56 0 01.31-.96l5.57-.47a2.5 2.5 0 001.94-1.54l2.12-5.11z"
            />
          </ActionButton>
          <ActionButton
            label={item.is_favorite ? "Unfavorite" : "Favorite"}
            active={item.is_favorite}
            onClick={onToggleFavorite}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 8.25c0-2.485-2.099-4.5-4.688-4.5-1.935 0-3.597 1.126-4.312 2.733-.715-1.607-2.377-2.733-4.313-2.733C5.1 3.75 3 5.765 3 8.25c0 7.22 9 12 9 12s9-4.78 9-12z"
            />
          </ActionButton>
          {onAi && (
            <ActionButton label="AI actions" onClick={onAi}>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 00-2.456 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z"
              />
            </ActionButton>
          )}
          <ActionButton label="Copy" onClick={onRestore}>
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184"
            />
          </ActionButton>
          <ActionButton label="Delete" danger onClick={onDelete}>
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0"
            />
          </ActionButton>
        </div>
      </div>
    </div>
  );
}

interface ActionButtonProps {
  label: string;
  active?: boolean;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function ActionButton({ label, active, danger, onClick, children }: ActionButtonProps) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className={cn(
        "rounded-md p-1.5 transition-colors",
        active
          ? "text-amber-500"
          : danger
            ? "text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      <svg className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.5" viewBox="0 0 24 24">
        {children}
      </svg>
    </button>
  );
}
