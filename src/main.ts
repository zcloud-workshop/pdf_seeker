import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { startupUpdateCheck } from "@/updater";

mount(App, {
  target: document.getElementById("app")!,
});

startupUpdateCheck();

