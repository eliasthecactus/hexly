import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

// Block the webview's own context menu and reload shortcuts in production.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement;
    if (!(t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement)) e.preventDefault();
  });
}

const app = mount(App, { target: document.getElementById("app")! });

export default app;
