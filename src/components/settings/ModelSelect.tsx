import { useCallback, useEffect, useState } from "react";
import type { AiModelInfo } from "../../types";
import * as tauri from "../../lib/tauri";
import { cn } from "../../lib/utils";

interface ModelSelectProps {
  value: string;
  /** Called with (modelId, displayName, free) once the user picks a model. */
  onChange: (id: string, name: string, free: boolean) => void;
  disabled?: boolean;
}

/** Format a per-million-token USD price for display. */
function priceLabel(perMillion: number): string {
  if (perMillion <= 0) return "free";
  if (perMillion < 0.01) return `$${perMillion.toFixed(4)}`;
  return `$${perMillion.toFixed(2)}`;
}

/**
 * OpenRouter model picker. Fetches the live catalog and groups models into
 * **Free** and **Paid** so cost is never a surprise. Falls back to a manual
 * id input when the catalog can't be fetched (e.g. no key yet / offline).
 */
export function ModelSelect({ value, onChange, disabled }: ModelSelectProps) {
  const [models, setModels] = useState<AiModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [manual, setManual] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await tauri.aiListModels();
      setModels(list);
      setManual(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const free = models.filter((m) => m.free);
  const paid = models.filter((m) => !m.free);

  if (manual || (models.length === 0 && !loading)) {
    return (
      <div className="flex flex-col gap-2">
        <div className="flex gap-2">
          <input
            value={value}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value, e.target.value, false)}
            placeholder="provider/model-id (e.g. deepseek/deepseek-chat-v3-0324:free)"
            className="flex-1 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
          />
          <button
            type="button"
            onClick={() => void load()}
            className="rounded-md border border-border px-2.5 text-xs text-muted-foreground hover:bg-accent"
          >
            Reload catalog
          </button>
        </div>
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <select
          value={value}
          disabled={disabled || loading}
          onChange={(e) => {
            const picked = models.find((m) => m.id === e.target.value);
            if (picked) onChange(picked.id, picked.name, picked.free);
          }}
          className="min-w-0 flex-1 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
        >
          <option value="">— choose a model —</option>
          {free.length > 0 && (
            <optgroup label={`Free (${free.length})`}>
              {free.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} — FREE
                </option>
              ))}
            </optgroup>
          )}
          {paid.length > 0 && (
            <optgroup label={`Paid (${paid.length})`}>
              {paid.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} — {priceLabel(m.prompt_price)}/1M in · {priceLabel(m.completion_price)}/1M out
                </option>
              ))}
            </optgroup>
          )}
        </select>
        <button
          type="button"
          onClick={() => void load()}
          disabled={loading}
          title="Reload model catalog"
          className="rounded-md border border-border px-2.5 py-1.5 text-xs text-muted-foreground hover:bg-accent"
        >
          {loading ? "…" : "↻"}
        </button>
      </div>

      {/* Selection summary with a clear free/paid badge. */}
      {value && (
        <div className="flex items-center gap-2 text-xs">
          <span
            className={cn(
              "rounded px-1.5 py-0.5 font-medium",
              models.find((m) => m.id === value)?.free
                ? "bg-green-500/15 text-green-600 dark:text-green-400"
                : "bg-amber-500/15 text-amber-600 dark:text-amber-400",
            )}
          >
            {models.find((m) => m.id === value)?.free ? "FREE model" : "Paid model"}
          </span>
          <span className="truncate text-muted-foreground">{value}</span>
        </div>
      )}

      {error && (
        <p className="text-xs text-destructive">
          {error}{" "}
          <button type="button" className="underline" onClick={() => setManual(true)}>
            enter model id manually
          </button>
        </p>
      )}
    </div>
  );
}
