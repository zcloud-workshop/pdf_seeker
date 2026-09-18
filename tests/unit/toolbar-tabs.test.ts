import { describe, it, expect, beforeEach } from "vitest";
import { mount, unmount, tick } from "svelte";
import ToolbarTabs from "@/components/editor/ToolbarTabs.svelte";

describe("ToolbarTabs Svelte 5 Component", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("mounts properly into the DOM and renders all 4 categories", () => {
    let selectedTool = "";

    const app = mount(ToolbarTabs, {
      target: document.body,
      props: {
        currentCategory: "page",
        onselecttool: (toolId: string) => {
          selectedTool = toolId;
        },
      },
    });

    expect(document.body.innerHTML).toContain("页面管理");
    expect(document.body.innerHTML).toContain("标注编辑");
    expect(document.body.innerHTML).toContain("转换提取");
    expect(document.body.innerHTML).toContain("批处理流水线");

    // Check that categories are rendered
    const buttons = Array.from(document.querySelectorAll("button"));
    expect(buttons.length).toBeGreaterThanOrEqual(4);

    unmount(app);
  });

  it("allows clicking and switching between all 4 categories (页面管理, 标注编辑, 转换提取, 批处理流水线)", async () => {
    let activeCat = "page";

    const app = mount(ToolbarTabs, {
      target: document.body,
      props: {
        ontoolaction: () => {},
      },
    });

    const categoryNames = ["页面管理", "标注编辑", "转换提取", "批处理流水线"];
    for (const name of categoryNames) {
      const catBtn = Array.from(document.querySelectorAll("button")).find((btn) =>
        btn.textContent?.trim().includes(name)
      );
      expect(catBtn).toBeDefined();
      catBtn?.click();
      await tick();

      // Ensure button was clicked and registered
      expect(catBtn?.className).toContain("bg-background");
    }

    // When clicking "标注编辑", edit tools like "添加文字" should be displayed in the popover
    const editBtn = Array.from(document.querySelectorAll("button")).find((btn) =>
      btn.textContent?.trim().includes("标注编辑")
    );
    editBtn?.click();
    await tick();
    expect(document.body.innerHTML).toContain("添加文字");
    expect(document.body.innerHTML).toContain("高亮标注");

    // When clicking "转换提取", conversion tools like "PDF 转图片" should be displayed
    const convertBtn = Array.from(document.querySelectorAll("button")).find((btn) =>
      btn.textContent?.trim().includes("转换提取")
    );
    convertBtn?.click();
    await tick();
    expect(document.body.innerHTML).toContain("PDF 转图片");

    // When clicking "批处理流水线", batch workspace tool should be displayed
    const batchBtn = Array.from(document.querySelectorAll("button")).find((btn) =>
      btn.textContent?.trim().includes("批处理流水线")
    );
    batchBtn?.click();
    await tick();
    expect(document.body.innerHTML).toContain("打开批处理工作台");

    unmount(app);
  });

  it("automatically minimizes popover on document scroll and restores on hover", async () => {
    const app = mount(ToolbarTabs, {
      target: document.body,
      props: {
        ontoolaction: () => {},
      },
    });

    // 1. Click "标注编辑" to open the popover
    const editBtn = Array.from(document.querySelectorAll("button")).find((btn) =>
      btn.textContent?.trim().includes("标注编辑")
    );
    editBtn?.click();
    await tick();
    expect(document.body.innerHTML).toContain("添加文字");

    // 2. Dispatch scroll event on window (simulating user scrolling the PDF up/down)
    window.dispatchEvent(new Event("scroll"));
    await tick();

    // The popover should automatically minimize into the compact pill showing "悬停重现"
    expect(document.body.innerHTML).toContain("悬停重现");

    // 3. Hover over the container (mouseenter)
    const popoverContainer = document.querySelector('[role="region"][aria-label="工具选择面板"]');
    expect(popoverContainer).toBeDefined();
    popoverContainer?.dispatchEvent(new MouseEvent("mouseenter", { bubbles: true }));
    await tick();

    // The popover should restore and re-expand with all tools visible
    expect(document.body.innerHTML).toContain("添加文字");

    unmount(app);
  });
});

