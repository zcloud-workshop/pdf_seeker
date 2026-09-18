import { describe, it, expect, beforeEach } from "vitest";
import { mount, unmount, flushSync } from "svelte";
import ToolboxModal from "@/components/editor/ToolboxModal.svelte";

describe("ToolboxModal Svelte 5 Component", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("does not render modal dialog when visible is false", () => {
    const app = mount(ToolboxModal, {
      target: document.body,
      props: {
        visible: false,
        onselecttool: () => {},
      },
    });

    expect(document.querySelector("input[type='text']")).toBeNull();
    unmount(app);
  });

  it("renders search input, categories and tool cards when visible is true", () => {
    let selectedTool = "";

    const app = mount(ToolboxModal, {
      target: document.body,
      props: {
        visible: true,
        onselecttool: (id: string) => {
          selectedTool = id;
        },
      },
    });

    const searchInput = document.querySelector("input[type='text']") as HTMLInputElement;
    expect(searchInput).not.toBeNull();
    expect(searchInput.placeholder).toContain("搜索全部工具");

    // Check category pills (全部, 页面管理, 注释标注, 格式转换, 提取与安全)
    const categoryButtons = Array.from(document.querySelectorAll("button"));
    expect(categoryButtons.length).toBeGreaterThan(5);

    unmount(app);
  });

  it("filters cards when search query changes", async () => {
    const app = mount(ToolboxModal, {
      target: document.body,
      props: {
        visible: true,
        onselecttool: () => {},
      },
    });

    // Wait for the visible effect to finish resetting searchQuery
    await new Promise((r) => setTimeout(r, 60));

    const searchInput = document.querySelector("input[type='text']") as HTMLInputElement;
    expect(searchInput).not.toBeNull();

    // Type query "脱敏"
    searchInput.value = "脱敏";
    searchInput.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();

    // Check that results contain sanitize
    expect(document.body.innerHTML).toContain("元数据脱敏");

    unmount(app);
  });
});
