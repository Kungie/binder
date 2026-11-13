import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ContextMenu, type MenuEntry } from "./ContextMenu";

function setup(items: MenuEntry[]) {
  const onClose = vi.fn();
  render(<ContextMenu x={10} y={10} items={items} onClose={onClose} />);
  return { onClose, user: userEvent.setup() };
}

describe("ContextMenu", () => {
  it("renders items and shortcuts", () => {
    setup([{ label: "Open", shortcut: "⌘↓" }, "separator", { label: "Copy", shortcut: "⌘C" }]);
    const menu = screen.getByRole("menu");
    expect(within(menu).getAllByRole("menuitem").map((m) => m.textContent)).toEqual(["Open⌘↓", "Copy⌘C"]);
    expect(within(menu).getByRole("separator")).toBeInTheDocument();
  });

  it("arrows move, separators are skipped, Enter picks", async () => {
    const pick = vi.fn();
    const { user, onClose } = setup([{ label: "Open", onPick: vi.fn() }, "separator", { label: "Copy", onPick: pick }]);
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Copy" })).toHaveAttribute("data-active", "true");
    await user.keyboard("{Enter}");
    expect(pick).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalled();
  });

  it("Esc closes", async () => {
    const { user, onClose } = setup([{ label: "Open" }]);
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("a submenu opens on ArrowRight", async () => {
    const pick = vi.fn();
    const { user } = setup([{ label: "Open with", submenu: [{ label: "TextEdit", onPick: pick }, { label: "Other…" }] }]);
    await user.keyboard("{ArrowDown}{ArrowRight}");
    const sub = screen.getByRole("menu", { name: "Open with" });
    expect(within(sub).getAllByRole("menuitem").map((m) => m.textContent)).toEqual(["TextEdit", "Other…"]);
    await user.keyboard("{Enter}");
    expect(pick).toHaveBeenCalledOnce();
  });

  it("a click picks", async () => {
    const pick = vi.fn();
    const { user } = setup([{ label: "Duplicate", onPick: pick }]);
    await user.click(screen.getByRole("menuitem", { name: "Duplicate" }));
    expect(pick).toHaveBeenCalledOnce();
  });
});
