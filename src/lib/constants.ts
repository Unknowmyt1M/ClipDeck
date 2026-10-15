/** App-wide constants. */

export const APP_NAME = "ClipDeck";

/** Filter chips shown above the history list. */
export const FILTERS = [
  { id: "all", label: "All" },
  { id: "pinned", label: "Pinned" },
  { id: "favorites", label: "Favorites" },
  { id: "text", label: "Text" },
  { id: "url", label: "URLs" },
  { id: "image", label: "Images" },
] as const;
