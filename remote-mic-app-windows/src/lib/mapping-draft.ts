import type { ButtonMappings } from "./bridge";

const KEY = "sayall.mappingDraft";

export function persistMappingDraft(edited: ButtonMappings, saved: ButtonMappings): void {
  try {
    const baseline = JSON.stringify(saved);
    if (JSON.stringify(edited) === baseline) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify({ baseline, edited }));
  } catch {
    // Storage may be unavailable. Never apply a draft to the backend implicitly.
  }
}

export function loadMappingDraft(saved: ButtonMappings): ButtonMappings | null {
  try {
    const draft = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (draft?.baseline === JSON.stringify(saved)
      && typeof draft.edited?.enabled === "boolean"
      && draft.edited.actions && typeof draft.edited.actions === "object") {
      return draft.edited;
    }
    localStorage.removeItem(KEY);
  } catch {
    // Corrupt or unavailable storage must not block settings loading.
  }
  return null;
}
