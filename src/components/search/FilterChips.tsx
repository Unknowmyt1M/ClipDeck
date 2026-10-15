import type { ItemFilter } from "../../types";
import { FILTERS } from "../../lib/constants";
import { cn } from "../../lib/utils";

interface FilterChipsProps {
  active: ItemFilter;
  onChange: (filter: ItemFilter) => void;
}

/** Horizontal filter chips (All / Pinned / Favorites / Text / URLs / Images). */
export function FilterChips({ active, onChange }: FilterChipsProps) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {FILTERS.map((f) => (
        <button
          key={f.id}
          type="button"
          onClick={() => onChange(f.id as ItemFilter)}
          className={cn(
            "rounded-full px-3 py-1 text-xs font-medium transition-colors",
            active === f.id
              ? "bg-primary text-primary-foreground"
              : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground",
          )}
        >
          {f.label}
        </button>
      ))}
    </div>
  );
}
