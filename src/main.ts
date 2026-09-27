import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { startupUpdateCheck } from "@/updater";
import { loadAndApplyConfig } from "@/settings";

mount(App, {
  target: document.getElementById("app")!,
});

startupUpdateCheck();
void loadAndApplyConfig().catch(() => {});
