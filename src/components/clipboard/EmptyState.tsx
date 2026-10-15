interface EmptyStateProps {
  hasQuery: boolean;
}

/** Friendly message when there's nothing to show. */
export function EmptyState({ hasQuery }: EmptyStateProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-12 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-full bg-muted">
        <svg
          className="h-7 w-7 text-muted-foreground"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          viewBox="0 0 24 24"
        >
          {hasQuery ? (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 21l-4.35-4.35M17 11a6 6 0 11-12 0 6 6 0 0112 0z"
            />
          ) : (
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184"
            />
          )}
        </svg>
      </div>
      <div>
        <p className="text-sm font-medium text-foreground">
          {hasQuery ? "No matching clips" : "No clips yet"}
        </p>
        <p className="mt-1 text-xs text-muted-foreground">
          {hasQuery
            ? "Try a different search or filter."
            : "Copy something — ClipDeck saves it here automatically."}
        </p>
      </div>
    </div>
  );
}
