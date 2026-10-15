import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import type { AiStatus, ExportResult, Settings, Stats, WipeResult } from "../../types";
import * as tauri from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { ModelSelect } from "./ModelSelect";

interface SettingsPageProps {
  settings: Settings;
  onChange: (next: Settings) => void;
  onClose: () => void;
}

type SectionId = "general" | "privacy" | "ai" | "stats";

const SECTIONS: { id: SectionId; label: string }[] = [
  { id: "general", label: "General" },
  { id: "privacy", label: "Privacy" },
  { id: "ai", label: "AI (OpenRouter)" },
  { id: "stats", label: "Statistics" },
];

/** Full-page settings view: General · Privacy · AI · Statistics. */
export function SettingsPage({ settings, onChange, onClose }: SettingsPageProps) {
  const [section, setSection] = useState<SectionId>("general");
  const [toast, setToast] = useState<string | null>(null);

  const notify = useCallback((message: string) => {
    setToast(message);
    setTimeout(() => setToast(null), 2500);
  }, []);

  /** Apply a partial change and persist immediately. */
  const patch = useCallback(
    async (partial: Partial<Settings>) => {
      const next = { ...settings, ...partial };
      onChange(next);
      try {
        await tauri.updateSettings(next);
      } catch (e) {
        notify(`Save failed: ${String(e)}`);
      }
    },
    [settings, onChange, notify],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            ← Back
          </button>
          <h1 className="text-sm font-semibold tracking-tight">Settings</h1>
        </div>
        <span className="text-xs text-muted-foreground">Esc to close</span>
      </header>

      <div className="flex min-h-0 flex-1">
        <nav className="w-44 shrink-0 border-r border-border p-2">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              type="button"
              onClick={() => setSection(s.id)}
              className={cn(
                "mb-1 block w-full rounded-md px-3 py-2 text-left text-sm transition-colors",
                section === s.id
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
              )}
            >
              {s.label}
            </button>
          ))}
        </nav>

        <main className="min-w-0 flex-1 overflow-y-auto p-4">
          {section === "general" && <GeneralSection settings={settings} patch={patch} />}
          {section === "privacy" && <PrivacySection settings={settings} patch={patch} notify={notify} />}
          {section === "ai" && <AiSection settings={settings} patch={patch} notify={notify} />}
          {section === "stats" && <StatsSection />}
        </main>
      </div>

      {toast && (
        <div className="pointer-events-none fixed bottom-8 left-1/2 -translate-x-1/2 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-lg">
          {toast}
        </div>
      )}
    </div>
  );
}

// ---------------- General ----------------

function GeneralSection({
  settings,
  patch,
}: {
  settings: Settings;
  patch: (p: Partial<Settings>) => Promise<void>;
}) {
  const [hotkey, setHotkey] = useState(settings.global_hotkey);

  return (
    <div className="flex max-w-xl flex-col gap-5">
      <Section title="Appearance" desc="ClipDeck follows the Windows theme unless you force one.">
        <Field label="Theme">
          <select
            value={settings.theme}
            onChange={(e) => void patch({ theme: e.target.value as Settings["theme"] })}
            className="rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
          >
            <option value="system">Follow system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </Field>
      </Section>

      <Section title="Startup & hotkey" desc="Run at login and the global quick-pick shortcut.">
        <Toggle
          label="Start ClipDeck when I sign in"
          description="Adds a per-user Run entry (no admin needed)."
          checked={settings.startup}
          onChange={(v) => void patch({ startup: v })}
        />
        <Field label="Global hotkey (quick pick window)">
          <div className="flex gap-2">
            <input
              value={hotkey}
              onChange={(e) => setHotkey(e.target.value)}
              onBlur={() => {
                const trimmed = hotkey.trim();
                if (trimmed !== settings.global_hotkey) void patch({ global_hotkey: trimmed });
              }}
              placeholder="Ctrl+Shift+V"
              className="flex-1 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
            />
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            Format: Ctrl+Shift+V, Ctrl+Alt+Space, … — applied immediately, invalid combos are rejected.
          </p>
        </Field>
        <Toggle
          label="Close button hides to tray"
          description="Keep capturing in the background; quit from the tray menu."
          checked={settings.close_to_tray}
          onChange={(v) => void patch({ close_to_tray: v })}
        />
      </Section>

      <Section title="History" desc="How much clipboard history is kept.">
        <Field label="Maximum items (10–100,000)">
          <input
            type="number"
            min={10}
            max={100000}
            value={settings.history_limit}
            onChange={(e) => void patch({ history_limit: Number(e.target.value) || 10 })}
            className="w-40 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
          />
        </Field>
      </Section>
    </div>
  );
}

// ---------------- Privacy ----------------

function PrivacySection({
  settings,
  patch,
  notify,
}: {
  settings: Settings;
  patch: (p: Partial<Settings>) => Promise<void>;
  notify: (m: string) => void;
}) {
  const [apps, setApps] = useState(settings.excluded_apps);
  const [titles, setTitles] = useState(settings.excluded_titles);
  const [exporting, setExporting] = useState(false);
  const [wiping, setWiping] = useState(false);
  const [confirmWipe, setConfirmWipe] = useState(false);

  const saveList = (raw: string, key: "excluded_apps" | "excluded_titles") => {
    const arr = raw
      .split(/[,\n]/)
      .map((s) => s.trim())
      .filter(Boolean);
    void patch({ [key]: JSON.stringify(arr) });
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const res: ExportResult = await tauri.exportHistory();
      notify(`Exported ${res.items} items → ${res.path}`);
    } catch (e) {
      notify(`Export failed: ${String(e)}`);
    } finally {
      setExporting(false);
    }
  };

  const handleWipe = async () => {
    if (!confirmWipe) {
      setConfirmWipe(true);
      notify("Press “Delete everything” again to confirm — this cannot be undone.");
      return;
    }
    setWiping(true);
    try {
      const res: WipeResult = await tauri.wipeData();
      notify(`Deleted ${res.deleted_rows} items and image files`);
      setConfirmWipe(false);
    } catch (e) {
      notify(`Wipe failed: ${String(e)}`);
    } finally {
      setWiping(false);
    }
  };

  return (
    <div className="flex max-w-xl flex-col gap-5">
      <Section title="Monitoring" desc="Pause capture or limit what gets stored.">
        <Toggle
          label="Pause clipboard monitoring"
          description="Nothing new is captured while paused."
          checked={settings.pause_monitoring}
          onChange={(v) => void patch({ pause_monitoring: v })}
        />
      </Section>

      <Section
        title="Sensitive content"
        desc="Detection runs 100% on-device (cards, OTPs, API keys, private keys, passwords…)."
      >
        <Toggle
          label="Detect sensitive content"
          description="Flags secrets and keeps them out of AI processing."
          checked={settings.sensitive_detection}
          onChange={(v) => void patch({ sensitive_detection: v })}
        />
        <Toggle
          label="Never store detected secrets"
          description="Sensitive clips are skipped entirely instead of being saved."
          checked={settings.skip_sensitive}
          onChange={(v) => void patch({ skip_sensitive: v })}
        />
      </Section>

      <Section title="Exclusions" desc="Never capture anything copied from these sources.">
        <Field label="Excluded applications (one per line or comma-separated)">
          <textarea
            value={apps}
            onChange={(e) => setApps(e.target.value)}
            onBlur={() => saveList(apps, "excluded_apps")}
            rows={3}
            placeholder={"keepass.exe\n1password.exe"}
            className="w-full rounded-md border border-input bg-card px-2.5 py-1.5 font-mono text-xs outline-none focus:border-ring"
          />
        </Field>
        <Field label="Excluded window-title substrings">
          <textarea
            value={titles}
            onChange={(e) => setTitles(e.target.value)}
            onBlur={() => saveList(titles, "excluded_titles")}
            rows={3}
            placeholder={"KeePass — password safe\nBank"}
            className="w-full rounded-md border border-input bg-card px-2.5 py-1.5 font-mono text-xs outline-none focus:border-ring"
          />
        </Field>
      </Section>

      <Section title="Retention" desc="Automatic cleanup of old, unpinned items.">
        <Field label="Keep items for days (0 = keep forever)">
          <input
            type="number"
            min={0}
            max={3650}
            value={settings.retention_days}
            onChange={(e) => void patch({ retention_days: Number(e.target.value) || 0 })}
            className="w-32 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
          />
        </Field>
      </Section>

      <Section title="Your data" desc="Everything stays on this PC.">
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={exporting}
            className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
          >
            {exporting ? "Exporting…" : "Export history as JSON"}
          </button>
          <button
            type="button"
            onClick={() => void handleWipe()}
            disabled={wiping}
            className={cn(
              "rounded-md border px-3 py-1.5 text-xs",
              confirmWipe
                ? "border-destructive bg-destructive/10 text-destructive"
                : "border-border text-muted-foreground hover:bg-destructive/10 hover:text-destructive",
            )}
          >
            {wiping ? "Deleting…" : confirmWipe ? "Delete everything — click again" : "Delete all data"}
          </button>
        </div>
      </Section>
    </div>
  );
}

// ---------------- AI ----------------

function AiSection({
  settings,
  patch,
  notify,
}: {
  settings: Settings;
  patch: (p: Partial<Settings>) => Promise<void>;
  notify: (m: string) => void;
}) {
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [testing, setTesting] = useState(false);
  const [savingKey, setSavingKey] = useState(false);

  useEffect(() => {
    void tauri.hasApiKey().then(setHasKey);
    void tauri.aiGetStatus().then(setStatus).catch(() => undefined);
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void tauri
      .onAiStatus((s) => setStatus(s))
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const saveKey = async () => {
    if (!apiKey.trim()) {
      notify("Paste an OpenRouter API key first");
      return;
    }
    setSavingKey(true);
    try {
      await tauri.setApiKey(apiKey);
      setHasKey(true);
      setApiKey("");
      notify("API key saved to the Windows credential vault");
    } catch (e) {
      notify(`Could not save key: ${String(e)}`);
    } finally {
      setSavingKey(false);
    }
  };

  const clearKey = async () => {
    try {
      await tauri.clearApiKey();
      setHasKey(false);
      notify("API key removed");
    } catch (e) {
      notify(`Could not clear key: ${String(e)}`);
    }
  };

  const testConnection = async () => {
    setTesting(true);
    try {
      await tauri.aiTestConnection();
      notify("OpenRouter connection OK ✓");
    } catch (e) {
      notify(`Connection failed: ${String(e)}`);
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="flex max-w-xl flex-col gap-5">
      <Section
        title="Enable AI"
        desc="Optional. Content is sent to OpenRouter only when enabled — sensitive clips are never sent."
      >
        <Toggle
          label="Enable AI features"
          description="Auto summaries, categories, keywords and per-clip AI actions."
          checked={settings.ai_enabled}
          onChange={(v) => void patch({ ai_enabled: v })}
        />
      </Section>

      <Section title="API key" desc="Stored in the Windows credential vault — never in SQLite, logs or the UI.">
        <div className="flex items-center gap-2">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={hasKey ? "•••••••• (key saved)" : "sk-or-v1-…"}
            className="flex-1 rounded-md border border-input bg-card px-2.5 py-1.5 text-sm outline-none focus:border-ring"
          />
          <button
            type="button"
            onClick={() => void saveKey()}
            disabled={savingKey}
            className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
          >
            {savingKey ? "Saving…" : "Save key"}
          </button>
          {hasKey && (
            <button
              type="button"
              onClick={() => void clearKey()}
              className="rounded-md border border-border px-3 py-1.5 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            >
              Remove
            </button>
          )}
        </div>
        <p className="text-xs text-muted-foreground">
          {hasKey ? "✓ Key configured" : "No key yet — get one at openrouter.ai/keys"}
        </p>
        <div className="mt-1">
          <button
            type="button"
            onClick={() => void testConnection()}
            disabled={testing}
            className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
          >
            {testing ? "Testing…" : "Test connection"}
          </button>
        </div>
      </Section>

      <Section
        title="Model"
        desc="Free models cost nothing on OpenRouter; paid models bill your OpenRouter credits per token."
      >
        <Field label="Model">
          <ModelSelect
            value={settings.ai_model}
            onChange={(id, name, free) => void patch({ ai_model: id, ai_model_name: name, ai_free: free })}
            disabled={!settings.ai_enabled}
          />
        </Field>
        <Toggle
          label="Analyze new clips automatically"
          description="Runs in the background queue after each capture."
          checked={settings.ai_auto_analyze}
          onChange={(v) => void patch({ ai_auto_analyze: v })}
        />
      </Section>

      <Section title="Queue" desc="Background analysis status.">
        <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          <span>{status?.busy ? "Analyzing…" : "Idle"}</span>
          <span>· queued {status?.queued ?? 0}</span>
          <span>· analyzed {status?.analyzed ?? 0}</span>
          <span>· skipped {status?.skipped ?? 0}</span>
          <button
            type="button"
            onClick={() => void tauri.aiAnalyzePending().then(() => notify("Queued pending items"))}
            disabled={!settings.ai_enabled}
            className="rounded-md border border-border px-2.5 py-1 hover:bg-accent disabled:opacity-40"
          >
            Analyze all pending
          </button>
        </div>
        {status?.last_error && (
          <p className="mt-2 rounded-md border border-destructive/30 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
            {status.last_error}
          </p>
        )}
      </Section>
    </div>
  );
}

// ---------------- Statistics ----------------

function StatsSection() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const load = () => {
      void tauri
        .getStats()
        .then((s) => {
          if (!cancelled) {
            setStats(s);
            setError(null);
          }
        })
        .catch((e) => !cancelled && setError(String(e)));
    };
    load();
    void tauri.onClipboardChanged(load).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (error) return <p className="text-sm text-destructive">{error}</p>;
  if (!stats) return <p className="text-sm text-muted-foreground">Loading…</p>;

  const cards: { label: string; value: number }[] = [
    { label: "Total clips", value: stats.total },
    { label: "Copied today", value: stats.today },
    { label: "Pinned", value: stats.pinned },
    { label: "Favorites", value: stats.favorites },
    { label: "Images", value: stats.images },
    { label: "Sensitive", value: stats.sensitive },
    { label: "AI analyzed", value: stats.ai_processed },
  ];

  return (
    <div className="flex max-w-xl flex-col gap-5">
      <Section title="Usage" desc="All numbers, no content.">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
          {cards.map((c) => (
            <div key={c.label} className="rounded-lg border border-border bg-card p-3">
              <div className="text-xl font-semibold">{c.value}</div>
              <div className="text-xs text-muted-foreground">{c.label}</div>
            </div>
          ))}
        </div>
      </Section>

      {stats.top_used.length > 0 && (
        <Section title="Most used" desc="Your top 5 clips by restore count.">
          <ol className="flex flex-col gap-1.5">
            {stats.top_used.map((t) => (
              <li key={t.id} className="flex items-center justify-between gap-3 text-xs">
                <span className="min-w-0 truncate text-foreground">{t.preview || `(${t.item_type})`}</span>
                <span className="shrink-0 text-muted-foreground">{t.use_count}×</span>
              </li>
            ))}
          </ol>
        </Section>
      )}
    </div>
  );
}

// ---------------- small building blocks ----------------

function Section({ title, desc, children }: { title: string; desc?: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="text-sm font-semibold">{title}</h2>
      {desc && <p className="mb-3 mt-0.5 text-xs text-muted-foreground">{desc}</p>}
      <div className="mt-2 flex flex-col gap-3">{children}</div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-xs text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

function Toggle({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="flex items-start justify-between gap-4 rounded-md border border-border bg-card px-3 py-2.5 text-left transition-colors hover:border-ring/50"
    >
      <span className="min-w-0">
        <span className="block text-sm">{label}</span>
        {description && <span className="block text-xs text-muted-foreground">{description}</span>}
      </span>
      <span
        className={cn(
          "mt-0.5 h-5 w-9 shrink-0 rounded-full p-0.5 transition-colors",
          checked ? "bg-primary" : "bg-muted",
        )}
      >
        <span
          className={cn(
            "block h-4 w-4 rounded-full bg-background transition-transform",
            checked ? "translate-x-4" : "translate-x-0",
          )}
        />
      </span>
    </button>
  );
}
