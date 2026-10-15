import { useState } from "react";
import type { AiAction, ClipboardItem } from "../../types";
import * as tauri from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { TRANSLATE_LANGUAGES } from "../../lib/constants";

interface AiActionModalProps {
  item: ClipboardItem;
  onClose: () => void;
  /** Called after a successful action so the list can refresh. */
  onCopied?: () => void;
}

const ACTIONS: { id: AiAction; label: string }[] = [
  { id: "summarize", label: "Summarize" },
  { id: "rewrite", label: "Rewrite" },
  { id: "translate", label: "Translate" },
  { id: "explain", label: "Explain" },
  { id: "fixgrammar", label: "Fix grammar" },
];

/**
 * One-shot AI actions for a single clip (Phase 7).
 * Runs through the configured OpenRouter model; sensitive clips are blocked
 * on the Rust side.
 */
export function AiActionModal({ item, onClose, onCopied }: AiActionModalProps) {
  const [action, setAction] = useState<AiAction>("summarize");
  const [lang, setLang] = useState<string>("English");
  const [result, setResult] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (next: AiAction = action) => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const text = await tauri.aiTransform(item.id, next, next === "translate" ? lang : undefined);
      setResult(text);
      onCopied?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const copyResult = async () => {
    if (!result) return;
    await navigator.clipboard.writeText(result);
    onCopied?.();
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="flex max-h-[80%] w-full max-w-lg flex-col gap-3 rounded-xl border border-border bg-card p-4 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="AI actions"
      >
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold">AI actions</h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            Esc ✕
          </button>
        </div>

        <p className="truncate rounded-md border border-border bg-muted/50 px-2.5 py-1.5 text-xs text-muted-foreground">
          {item.preview || item.content.slice(0, 160)}
        </p>

        <div className="flex flex-wrap gap-1.5">
          {ACTIONS.map((a) => (
            <button
              key={a.id}
              type="button"
              disabled={busy}
              onClick={() => {
                setAction(a.id);
                void run(a.id);
              }}
              className={cn(
                "rounded-md border px-2.5 py-1 text-xs transition-colors disabled:opacity-50",
                action === a.id
                  ? "border-ring bg-accent text-foreground"
                  : "border-border text-muted-foreground hover:bg-accent/60",
              )}
            >
              {a.label}
            </button>
          ))}
          <select
            value={lang}
            disabled={busy}
            onChange={(e) => setLang(e.target.value)}
            className="rounded-md border border-border bg-card px-2 py-1 text-xs outline-none"
            aria-label="Translate target language"
          >
            {TRANSLATE_LANGUAGES.map((l) => (
              <option key={l} value={l}>
                → {l}
              </option>
            ))}
          </select>
        </div>

        <div className="min-h-24 overflow-y-auto rounded-md border border-border bg-background p-3">
          {busy && <p className="text-xs text-muted-foreground">Thinking…</p>}
          {error && <p className="text-xs text-destructive">{error}</p>}
          {result && (
            <p className="selectable text-sm whitespace-pre-wrap text-foreground">{result}</p>
          )}
          {!busy && !error && !result && (
            <p className="text-xs text-muted-foreground">
              Pick an action above. Sent to OpenRouter — sensitive clips are blocked automatically.
            </p>
          )}
        </div>

        {result && (
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => void run()}
              disabled={busy}
              className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
            >
              Retry
            </button>
            <button
              type="button"
              onClick={() => void copyResult()}
              className="rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90"
            >
              Copy result
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
