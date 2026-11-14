// Cross-device verbs: send files to a paired PC and bridge the clipboard.
import { invoke } from "@tauri-apps/api/core";
import { callTool } from "./api";

export const pushFile = (device: string, path: string, dest = "") => invoke<string>("push_file", { device, path, dest });

/** The bytes travel as a raw body; the metadata goes in percent-encoded headers. */
export async function pushBlob(device: string, file: File, dest = ""): Promise<string> {
  const e = encodeURIComponent;
  const headers = { "x-device": e(device), "x-name": e(file.name), "x-dest": e(dest) };
  return invoke<string>("push_bytes", new Uint8Array(await file.arrayBuffer()), { headers });
}

/** Opens the system file picker and sends every chosen file. Rejects on cancel or failure. */
export function pickAndSend(device: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.hidden = true;
    input.onchange = async () => {
      input.remove();
      const files = [...(input.files ?? [])];
      if (!files.length) return reject("No file chosen");
      try {
        const out: string[] = [];
        for (const f of files) out.push(await pushBlob(device, f));
        resolve(out.join("\n"));
      } catch (e) {
        reject(String(e));
      }
    };
    input.oncancel = () => { input.remove(); reject("No file chosen"); };
    document.body.append(input);
    input.click();
  });
}

const fail = (r: { ok: boolean; text: string }) => { if (!r.ok) throw r.text; return r.text; };

export async function clipboardToDevice(device: string): Promise<string> {
  const text = await navigator.clipboard.readText();
  if (!text) throw "The clipboard holds no text";
  fail(await callTool(device, "clipboard_set", { text }));
  return `Clipboard sent to ${device}`;
}

export async function clipboardFromDevice(device: string): Promise<string> {
  const text = fail(await callTool(device, "clipboard_get", {}));
  await navigator.clipboard.writeText(text);
  return `Clipboard copied from ${device}`;
}
