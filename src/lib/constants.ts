/** App-wide constants. */

export const APP_NAME = "ClipDeck";

/** Filter chips shown above the history list. */
export const FILTERS = [
  { id: "all", label: "All" },
  { id: "pinned", label: "Pinned" },
  { id: "favorites", label: "Favorites" },
  { id: "today", label: "Today" },
  { id: "week", label: "This week" },
  { id: "text", label: "Text" },
  { id: "url", label: "URLs" },
  { id: "image", label: "Images" },
] as const;

/** Languages offered in the AI translate action. */
export const TRANSLATE_LANGUAGES = [
  "English",
  "Hindi",
  "Bengali",
  "Tamil",
  "Telugu",
  "Marathi",
  "Gujarati",
  "Kannada",
  "Malayalam",
  "Punjabi",
  "Urdu",
  "Spanish",
  "French",
  "German",
  "Japanese",
  "Korean",
  "Chinese (Simplified)",
  "Arabic",
  "Portuguese",
  "Russian",
] as const;
