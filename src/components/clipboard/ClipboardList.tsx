import { useEffect, useRef } from "react";
import type { ClipboardItem } from "../../types";
import { ClipboardCard } from "./ClipboardCard";

interface ClipboardListProps {
  items: ClipboardItem[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onRestore: (item: ClipboardItem) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onToggleFavorite: (item: ClipboardItem) => void;
  onDelete: (item: ClipboardItem) => void;
  /** Optional AI actions handler (omitted in the quick-pick window). */
  onAi?: (item: ClipboardItem) => void;
}

/** Scrollable history list with keyboard navigation support. */
export function ClipboardList({
  items,
  selectedIndex,
  onSelect,
  onRestore,
  onTogglePin,
  onToggleFavorite,
  onDelete,
  onAi,
}: ClipboardListProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  // Keep the selected card in view during keyboard navigation.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const el = container.children[selectedIndex] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  return (
    <div ref={containerRef} className="flex flex-col gap-2 p-4">
      {items.map((item, index) => (
        <ClipboardCard
          key={item.id}
          item={item}
          selected={index === selectedIndex}
          onSelect={() => onSelect(index)}
          onRestore={() => onRestore(item)}
          onTogglePin={() => onTogglePin(item)}
          onToggleFavorite={() => onToggleFavorite(item)}
          onDelete={() => onDelete(item)}
          onAi={onAi ? () => onAi(item) : undefined}
        />
      ))}
    </div>
  );
}
