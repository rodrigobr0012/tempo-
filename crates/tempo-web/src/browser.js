// Serviços do navegador usados pela versão web: IndexedDB (cópia do banco), notificações,
// som, vibração, download e aviso entre abas. Equivalente web do `Bridge.java` do Android.
//
// Dentro do programa para Windows (Tauri, crates/tempo-pc) o banco vai para um arquivo no PC e o
// aviso de fim do timer é agendado pelo processo nativo.

const tauri = window.__TAURI__;

/** Rodando no programa para Windows? */
export function isDesktop() {
  return !!tauri;
}

const DB_NAME = "tempo";
const STORE = "kv";
const KEY = "db";

let dbPromise = null;
const channel = typeof BroadcastChannel !== "undefined" ? new BroadcastChannel("tempo") : null;

function openDb() {
  if (!dbPromise) {
    dbPromise = new Promise((resolve, reject) => {
      const r = indexedDB.open(DB_NAME, 1);
      r.onupgradeneeded = () => r.result.createObjectStore(STORE);
      r.onsuccess = () => resolve(r.result);
      r.onerror = () => reject(r.error);
    });
  }
  return dbPromise;
}

function put(key, value) {
  return openDb().then(
    (db) =>
      new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, "readwrite");
        tx.objectStore(STORE).put(value, key);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
      }),
  );
}

/** Bytes do banco salvo, ou null se ainda não há. */
export async function loadDb() {
  if (tauri) {
    const bytes = new Uint8Array(await tauri.core.invoke("load_db"));
    return bytes.length ? bytes : null;
  }
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const r = db.transaction(STORE).objectStore(STORE).get(KEY);
    r.onsuccess = () => resolve(r.result ?? null);
    r.onerror = () => reject(r.error);
  });
}

/** Salva o banco e avisa as outras abas. */
export function saveDb(view) {
  // `view` aponta para a memória do WebAssembly: copia antes de qualquer espera.
  const copy = new Uint8Array(view);
  if (tauri) return tauri.core.invoke("save_db", copy);
  return put(KEY, copy).then(() => channel?.postMessage("saved"));
}

/** Guarda uma cópia de um banco que não abriu, para não perder dados. */
export function backupDb(view) {
  const copy = new Uint8Array(view);
  if (tauri) return tauri.core.invoke("backup_db", copy);
  return put(`backup-${Date.now()}`, copy);
}

/** Programa para Windows: bandeja + aviso nativo no fim do timer. `tagName` vazio = sem timer. */
export function syncTimer(tagName, deadlineMs, tooltip) {
  if (!tauri) return;
  const sync = tagName ? { tagName, deadlineMs, tooltip } : null;
  tauri.core.invoke("sync_timer", { sync }).catch((e) => console.error(e));
}

export async function autostartEnabled() {
  if (!tauri) return false;
  try {
    return await tauri.core.invoke("plugin:autostart|is_enabled");
  } catch (_) {
    return false;
  }
}

export async function setAutostart(on) {
  if (!tauri) return;
  await tauri.core.invoke(on ? "plugin:autostart|enable" : "plugin:autostart|disable");
}

/** Chama `cb` quando outra aba salvar o banco. */
export function onOtherTabSaved(cb) {
  if (channel) channel.onmessage = () => cb();
}

/** Pede ao navegador para não apagar os dados quando faltar espaço. */
export function persistStorage() {
  try {
    navigator.storage?.persist?.().catch(() => {});
  } catch (_) {}
}

/** "granted", "denied", "default" ou "unsupported". */
export function notificationPermission() {
  return typeof Notification === "undefined" ? "unsupported" : Notification.permission;
}

export async function requestNotifications() {
  if (typeof Notification === "undefined") return "unsupported";
  try {
    return await Notification.requestPermission();
  } catch (_) {
    return Notification.permission;
  }
}

export function notify(title, body) {
  if (typeof Notification === "undefined" || Notification.permission !== "granted") return;
  try {
    new Notification(title, { body, tag: "tempo-timer", icon: "icon.svg" });
  } catch (_) {
    // Alguns navegadores móveis só aceitam notificações via service worker.
  }
}

let audio = null;

/** Cria o contexto de áudio durante um gesto do usuário (política de autoplay). */
export function unlockAudio() {
  try {
    audio ??= new AudioContext();
    if (audio.state === "suspended") audio.resume();
  } catch (_) {}
}

/** Três bipes curtos. */
export function beep() {
  try {
    audio ??= new AudioContext();
    const t0 = audio.currentTime;
    [
      [0, 880],
      [0.25, 880],
      [0.5, 1175],
    ].forEach(([dt, freq]) => {
      const osc = audio.createOscillator();
      const gain = audio.createGain();
      osc.frequency.value = freq;
      osc.connect(gain);
      gain.connect(audio.destination);
      const t = t0 + dt;
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.3, t + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.2);
      osc.start(t);
      osc.stop(t + 0.22);
    });
  } catch (_) {}
}

export function canVibrate() {
  return typeof navigator.vibrate === "function";
}

export function vibrate() {
  try {
    navigator.vibrate?.([300, 150, 300]);
  } catch (_) {}
}

/** Baixa um texto como arquivo (BOM para o Excel reconhecer UTF-8). */
export function download(name, text, mime) {
  const blob = new Blob(["﻿" + text], { type: mime });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
