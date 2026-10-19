import { beforeEach, describe, expect, it } from "vitest";
import { loadMappingDraft, persistMappingDraft } from "./mapping-draft";
import type { ButtonMappings } from "./bridge";

beforeEach(() => localStorage.clear());
describe("mapping draft persistence", () => {
  const saved: ButtonMappings = { enabled: true, actions: {} };
  const edited: ButtonMappings = { enabled: false, actions: {} };
  it("restores unsaved edits after recreating the window without applying them", () => {
    persistMappingDraft(edited, saved);
    expect(loadMappingDraft(saved)).toEqual(edited);
    expect(saved.enabled).toBe(true);
    persistMappingDraft(edited, edited);
    expect(loadMappingDraft(edited)).toBeNull();
  });
  it("discards a draft if authoritative settings have changed", () => {
    persistMappingDraft(edited, saved);
    expect(loadMappingDraft({ enabled: true, actions: { ok: {
      single: { type: "disabled" }, double: { type: "disabled" }, long: { type: "disabled" },
    } } })).toBeNull();
  });
  it("ignores corrupt data", () => {
    localStorage.setItem("sayall.mappingDraft", "invalid json");
    expect(loadMappingDraft(saved)).toBeNull();
  });
});
