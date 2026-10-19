import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { EntitySearchBox } from "./entity-search-box";

vi.mock("@/lib/demo-mode", () => ({ PUBLIC_PERSON_SEARCH_ENABLED: true }));
vi.mock("@/lib/i18n", () => ({
  useI18n: () => ({ t: {
    searchPlaceholder: "Search a person",
    searchUnavailable: "Unavailable",
    personSearch: "Person search",
    searchHint: "Select a person",
    loading: "Loading",
    noResults: "No results",
  }}),
}));

describe("EntitySearchBox", () => {
  it("submits keystrokes without another component debounce and supports keyboard identity selection", () => {
    const onSubmitQuery = vi.fn();
    const onSelect = vi.fn();
    const suggestions = [
      { qid: "Q1", label: "Alexandre Dumas", known_locally: true, entity_id: "father" },
      { qid: "Q2", label: "Alexandre Dumas", known_locally: true, entity_id: "son" },
    ];
    render(<EntitySearchBox suggestions={suggestions} onSubmitQuery={onSubmitQuery} onSelect={onSelect}/>);
    const input = screen.getByRole("combobox");
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: "Alexandre" } });
    expect(onSubmitQuery).toHaveBeenCalledWith("Alexandre");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledWith(suggestions[1]);
    expect(onSubmitQuery).toHaveBeenLastCalledWith("");
  });
});
