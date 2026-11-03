import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorText } from "./api";
import { store } from "./store.svelte";

let checking = false;

/** Checks GitHub Releases for a newer signed build. */
export async function checkForUpdates(manual = false) {
  if (checking) return;
  checking = true;
  try {
    const update = await check();
    if (!update) {
      if (manual) store.toast("Hexly is up to date", "success");
      return;
    }
    store.update = {
      version: update.version,
      install: async () => {
        await store.withBusy(`Downloading Hexly ${update.version}…`, async (progress) => {
          let total = 0;
          let done = 0;
          await update.downloadAndInstall((ev) => {
            if (ev.event === "Started") total = ev.data.contentLength ?? 0;
            else if (ev.event === "Progress") {
              done += ev.data.chunkLength;
              if (total) progress(done / total);
            }
          });
        });
        await relaunch();
      },
    };
  } catch (e) {
    // The updater only reports a generic error for "offline" and for "no
    // published release yet", so explain both in plain words.
    console.warn("update check failed:", errorText(e));
    if (manual) {
      store.toast("Couldn't check for updates. You may be offline, or no release has been published yet.", "error");
    }
  } finally {
    checking = false;
  }
}
