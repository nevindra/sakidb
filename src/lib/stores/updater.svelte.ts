import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

let update = $state<Update | null>(null);
let downloading = $state(false);
let downloaded = $state(0);
let contentLength = $state(0);
let readyToInstall = $state(false);
let dismissed = $state(false);
let checking = $state(false);

export function getUpdate() { return update; }
export function isDownloading() { return downloading; }
export function getDownloaded() { return downloaded; }
export function getContentLength() { return contentLength; }
export function isReadyToInstall() { return readyToInstall; }
export function isDismissed() { return dismissed; }
export function isChecking() { return checking; }

export function dismissBanner() { dismissed = true; }

export async function checkForUpdate(): Promise<boolean> {
  checking = true;
  try {
    update = await check();
    return update !== null;
  } catch {
    return false;
  } finally {
    checking = false;
  }
}

export async function downloadAndInstall() {
  if (!update) return;
  downloading = true;
  downloaded = 0;
  contentLength = 0;

  try {
    await update.downloadAndInstall((progress) => {
      if (progress.event === 'Started' && progress.data.contentLength) {
        contentLength = progress.data.contentLength;
      } else if (progress.event === 'Progress') {
        downloaded += progress.data.chunkLength;
      } else if (progress.event === 'Finished') {
        readyToInstall = true;
      }
    });
    readyToInstall = true;
  } finally {
    downloading = false;
  }
}

export async function restartApp() {
  await relaunch();
}
