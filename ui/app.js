// dimscreen settings window. Rust owns the settings (main.rs); this page shows a snapshot of them,
// sends changes back with `update`, and repaints when the shade changes from outside (the shortcut,
// the tray, a display being plugged in).
"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const win = window.__TAURI__.window.getCurrentWindow();
const $ = (id) => document.getElementById(id);
const root = document.documentElement;

// ───────────── icons: 9×9 pixel bitmaps for Pocket, 24 px line icons for Standard ─────────────
const PIX = {
  moon: "..###....|.##......|##.......|##.......|##.......|###.....#|.###...##|..######.|...###...",
  power: "....#....|.#..#..#.|#...#...#|#...#...#|#.......#|#.......#|#.......#|.#.....#.|..#####..",
  drop: "....#....|...###...|..#####..|.##.####.|##.######|##.######|###.#####|.#######.|..#####..",
  display: "#########|#.......#|#.......#|#.......#|#.......#|#########|....#....|..#####..|.........",
  keyboard: ".........|#########|#.#.#.#.#|#########|#.#.#.#.#|#########|#.#####.#|#########|.........",
  lab: ".#..#..#.|###.#..#.|.#..#..#.|.#..#.###|.#..#..#.|.#.###.#.|.#..#..#.|.#..#..#.|.#..#..#.",
  app: "#########|#.#.#...#|#########|#.......#|#.......#|#.......#|#.......#|#########|.........",
  camera: ".........|..###....|#########|#..###..#|#.#...#.#|#.#...#.#|#..###..#|#########|.........",
  close: ".........|.#.....#.|..#...#..|...#.#...|....#....|...#.#...|..#...#..|.#.....#.|.........",
  wmin: ".........|.........|.........|.........|.........|.........|.#######.|.#######.|.........",
  check: ".........|........#|.......##|......##.|#....##..|##..##...|.####....|..##.....|.........",
};
const LINE = {
  moon: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z",
  power: "M12 3v8M7 5.5a8 8 0 1 0 10 0",
  drop: "M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z",
  display: "M3 4h18v12H3zM8 20h8M12 16v4",
  keyboard: "M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10",
  lab: "M6 4v16M12 4v16M18 4v16M4 8h4M10 15h4M16 10h4",
  app: "M3 5h18v14H3zM3 9h18",
  camera: "M4 7h4l2-2h4l2 2h4v12H4zM12 10a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
  close: "M6 6l12 12M18 6L6 18",
  wmin: "M5 12h14",
  check: "M5 12l5 5 9-10",
};

function ic(name) {
  if (root.dataset.look === "pocket" && PIX[name]) {
    let rects = "";
    PIX[name].split("|").forEach((row, y) => {
      for (let x = 0; x < row.length; x++) if (row[x] === "#") rects += `<rect x="${x}" y="${y}" width="1" height="1"/>`;
    });
    return `<svg class="i px" viewBox="0 0 9 9" aria-hidden="true">${rects}</svg>`;
  }
  return `<svg class="i ln" viewBox="0 0 24 24" aria-hidden="true"><path d="${LINE[name]}"/></svg>`;
}

function paintIcons() {
  document.querySelectorAll("[data-icon]").forEach((el) => {
    el.querySelector(":scope > svg.i")?.remove();
    el.insertAdjacentHTML("afterbegin", ic(el.dataset.icon));
  });
  document.querySelectorAll(".switch").forEach((el) => { el.innerHTML = ic("check"); });
}

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

// ───────────── state ─────────────
let S = null;            // the last snapshot from Rust
let pending = null;      // changes not sent yet
let inflight = false;
let recording = false;   // recording a new shortcut
let mapSig = "";

/** Applies a change here at once and sends it, coalescing while a send is in flight (slider drags). */
function send(patch) {
  Object.assign(S.settings, patch);
  pending = Object.assign(pending || {}, patch);
  render();
  if (!inflight) flush();
}

async function flush() {
  if (!pending) return;
  const patch = pending;
  pending = null;
  inflight = true;
  try {
    const snap = await invoke("update", { patch });
    if (!pending) { S = snap; render(); }
  } catch (e) {
    showNote(String(e));
  } finally {
    inflight = false;
    if (pending) flush();
  }
}

function showNote(text) {
  $("note").hidden = !text;
  $("note").textContent = text || "";
}

// ───────────── look ─────────────
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

function applyLook() {
  const s = S.settings;
  const scheme = s.Theme === "system" ? (darkQuery.matches ? "dark" : "light") : s.Theme;
  const lookChanged = root.dataset.look !== s.Look;
  root.dataset.look = s.Look;
  root.dataset.scheme = scheme;
  if (lookChanged) { paintIcons(); mapSig = ""; }
}
darkQuery.addEventListener("change", () => S && render());

// ───────────── render ─────────────
function displayName(s) {
  const d = S.displays.find((m) => m.device === s.DisplayTarget);
  return d ? d.name : "All displays";
}

function appLabel(s) {
  return s.TargetAppLabel || `${s.TargetProcessName}.exe`;
}

function render() {
  const s = S.settings;
  const st = S.status;
  applyLook();

  const waiting = s.Enabled && st.waiting;
  $("pill").textContent = !s.Enabled ? "Off" : waiting ? "Waiting" : "On";
  $("pill").className = "pill" + (!s.Enabled ? "" : waiting ? " wait" : " on");

  // the hero
  const lcd = $("lcd");
  const wasOff = lcd.classList.contains("off");
  lcd.classList.toggle("off", !s.Enabled);
  lcd.classList.toggle("hot", waiting);
  if (wasOff && s.Enabled) { lcd.classList.remove("boot"); void lcd.offsetWidth; lcd.classList.add("boot"); }
  $("lcdNum").textContent = `${s.Strength}%`;
  $("lcdBar").style.width = `${s.Strength}%`;
  const where = displayName(s) + (s.TargetProcessName ? ` · only with ${appLabel(s)}` : "");
  $("lcdSub").textContent = !s.Enabled ? `Off · ${where}` : waiting ? `Waiting for ${appLabel(s)}` : `On · ${where}`;
  $("toggleLabel").textContent = s.Enabled ? "Turn off" : "Turn on";
  $("toggle").classList.toggle("is-on", s.Enabled);
  const slider = $("strength");
  if (document.activeElement !== slider || +slider.value !== s.Strength) slider.value = s.Strength;
  slider.style.setProperty("--pct", `${s.Strength}%`);
  slider.setAttribute("aria-valuetext", `${s.Strength}%`);
  document.querySelectorAll("[data-preset]").forEach((b) => b.classList.toggle("on", +b.dataset.preset === s.Strength));
  $("shortcutCnt").textContent = S.hotkeyLabel;

  // tint
  const hex = s.TintHex.toUpperCase();
  let preset = false;
  document.querySelectorAll(".sw[data-hex]").forEach((b) => {
    const on = b.dataset.hex.toUpperCase() === hex;
    preset ||= on;
    b.setAttribute("aria-checked", on);
  });
  $("customSw").setAttribute("aria-checked", !preset);
  $("customSw").style.setProperty("--c", hex);
  $("customColor").value = hex.toLowerCase();
  $("tintCnt").textContent = hex;

  // where
  renderMap();
  const one = !!s.TargetProcessName;
  document.querySelectorAll("#appMode .chip").forEach((c) => c.setAttribute("aria-checked", (c.dataset.v === "one") === one));
  $("target").hidden = !one;
  if (one) {
    $("targetName").textContent = appLabel(s);
    $("targetMeta").textContent = `${s.TargetProcessName}.exe · the shade shows only while it's in front`;
  }

  // behavior
  if (!recording) $("hotkey").textContent = S.hotkeyLabel;
  $("hotkeyErr").hidden = st.hotkeyOk || recording;
  document.querySelectorAll("[data-switch]").forEach((row) => {
    row.querySelector(".switch").setAttribute("aria-checked", !!s[row.dataset.switch]);
  });
  document.querySelector('[data-switch="StartHidden"]').classList.toggle("disabled", !s.LaunchAtStartup);

  // look
  document.querySelectorAll("#lookSeg .chip").forEach((c) => c.setAttribute("aria-checked", c.dataset.v === s.Look));
  document.querySelectorAll("#themeSeg .chip").forEach((c) => c.setAttribute("aria-checked", c.dataset.v === s.Theme));

  $("ver").textContent = S.version;
  showNote(S.note);
}

/** Draws the monitors where they sit, scaled into the box; click one to shade only it. */
function renderMap() {
  const s = S.settings;
  const map = $("map");
  const ds = S.displays;
  const all = !ds.some((d) => d.device === s.DisplayTarget);
  $("allDisplays").classList.toggle("on", all);
  $("allDisplays").setAttribute("aria-pressed", all);

  const sig = JSON.stringify([ds, map.clientWidth, map.clientHeight]);
  if (sig !== mapSig) {
    mapSig = sig;
    map.innerHTML = "";
    if (!ds.length) return;
    const minX = Math.min(...ds.map((d) => d.x)), minY = Math.min(...ds.map((d) => d.y));
    const maxX = Math.max(...ds.map((d) => d.x + d.width)), maxY = Math.max(...ds.map((d) => d.y + d.height));
    const pad = 14;
    const scale = Math.min((map.clientWidth - pad * 2) / (maxX - minX), (map.clientHeight - pad * 2) / (maxY - minY));
    const offX = (map.clientWidth - (maxX - minX) * scale) / 2;
    const offY = (map.clientHeight - (maxY - minY) * scale) / 2;
    for (const d of ds) {
      const b = document.createElement("button");
      b.className = "mon";
      b.dataset.device = d.device;
      const gap = 3;
      b.style.left = `${offX + (d.x - minX) * scale + gap}px`;
      b.style.top = `${offY + (d.y - minY) * scale + gap}px`;
      b.style.width = `${d.width * scale - gap * 2}px`;
      b.style.height = `${d.height * scale - gap * 2}px`;
      b.title = `${d.name} · ${d.width} × ${d.height}${d.primary ? " · main display" : ""}`;
      b.setAttribute("aria-label", b.title);
      b.innerHTML = `<b>${esc(d.name.replace(/^Display /, ""))}</b><span>${d.width}×${d.height}</span>`;
      map.appendChild(b);
    }
  }
  map.querySelectorAll(".mon").forEach((b) => {
    const on = all || b.dataset.device === s.DisplayTarget;
    b.classList.toggle("on", on);
    b.setAttribute("aria-pressed", on);
  });
}
new ResizeObserver(() => S && renderMap()).observe($("map"));

// ───────────── shade ─────────────
$("toggle").addEventListener("click", async () => {
  S = await invoke("toggle");
  render();
});
$("strength").addEventListener("input", (e) => send({ Strength: +e.target.value }));
document.querySelectorAll("[data-preset]").forEach((b) => b.addEventListener("click", () => send({ Strength: +b.dataset.preset })));

// ───────────── tint ─────────────
document.querySelectorAll(".sw[data-hex]").forEach((b) => b.addEventListener("click", () => send({ TintHex: b.dataset.hex })));
$("customColor").addEventListener("input", (e) => send({ TintHex: e.target.value.toUpperCase() }));

// ───────────── where ─────────────
$("allDisplays").addEventListener("click", () => send({ DisplayTarget: "All displays" }));
$("map").addEventListener("click", (e) => {
  const b = e.target.closest(".mon");
  if (b) send({ DisplayTarget: b.dataset.device });
});
document.querySelectorAll("#appMode .chip").forEach((c) => c.addEventListener("click", () => {
  if (c.dataset.v === "all") send({ TargetProcessName: "", TargetExecutablePath: "", TargetAppLabel: "" });
  else openPicker();
}));
$("pickApp").addEventListener("click", openPicker);

let apps = [];
async function openPicker() {
  $("picker").hidden = false;
  $("pickerSearch").value = "";
  $("pickerList").innerHTML = `<div class="empty">Looking for running apps…</div>`;
  $("pickerSearch").focus();
  apps = await invoke("running_apps");
  renderApps();
}
function closePicker() {
  $("picker").hidden = true;
}
function matchingApps() {
  const q = $("pickerSearch").value.trim().toLowerCase();
  return apps.filter((a) => !q || [a.label, a.processName, a.title].some((t) => t.toLowerCase().includes(q)));
}
function renderApps() {
  const cur = S.settings.TargetProcessName.toLowerCase();
  const list = matchingApps();
  $("pickerList").innerHTML = list.length
    ? list.map((a) => `<button class="row${a.processName.toLowerCase() === cur ? " on" : ""}" role="option" data-path="${esc(a.path)}"><b>${esc(a.label)}</b><span>${esc(a.processName)}.exe · ${esc(a.title)}</span></button>`).join("")
    : `<div class="empty">${apps.length ? "No running app matches that." : "No apps with open windows."}</div>`;
}
function chooseApp(a) {
  send({ TargetProcessName: a.processName, TargetExecutablePath: a.path, TargetAppLabel: a.label });
  closePicker();
}
$("pickerSearch").addEventListener("input", renderApps);
$("pickerSearch").addEventListener("keydown", (e) => {
  if (e.key === "Enter") { const a = matchingApps()[0]; if (a) chooseApp(a); }
  if (e.key === "ArrowDown") { e.preventDefault(); $("pickerList").querySelector(".row")?.focus(); }
});
$("pickerList").addEventListener("click", (e) => {
  const b = e.target.closest(".row");
  const a = b && apps.find((x) => x.path === b.dataset.path);
  if (a) chooseApp(a);
});
$("pickerList").addEventListener("keydown", (e) => {
  const rows = [...$("pickerList").querySelectorAll(".row")];
  const i = rows.indexOf(document.activeElement);
  if (e.key === "ArrowDown") { e.preventDefault(); rows[Math.min(i + 1, rows.length - 1)]?.focus(); }
  if (e.key === "ArrowUp") { e.preventDefault(); (i <= 0 ? $("pickerSearch") : rows[i - 1]).focus(); }
});
$("pickerClose").addEventListener("click", closePicker);
$("picker").addEventListener("click", (e) => { if (e.target === $("picker")) closePicker(); });

// ───────────── behavior ─────────────
const MOD_KEYS = new Set([16, 17, 18, 91, 92, 229]);
function modsOf(e) {
  return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.shiftKey ? 4 : 0) | (e.metaKey ? 8 : 0);
}
function modsLabel(m) {
  return [[2, "Ctrl"], [1, "Alt"], [4, "Shift"], [8, "Win"]].filter(([b]) => m & b).map(([, n]) => n).join(" + ");
}
async function startRecording() {
  recording = true;
  $("hotkey").classList.add("rec");
  $("hotkey").textContent = "Press a shortcut…";
  $("hotkeyErr").hidden = true;
  await invoke("pause_hotkey", { paused: true });
}
async function stopRecording() {
  if (!recording) return;
  recording = false;
  $("hotkey").classList.remove("rec");
  await invoke("pause_hotkey", { paused: false });
  render();
}
$("hotkey").addEventListener("click", () => (recording ? stopRecording() : startRecording()));
window.addEventListener("keydown", async (e) => {
  if (!recording) {
    if (e.key === "Escape" && !$("picker").hidden) closePicker();
    return;
  }
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") return stopRecording();
  const mods = modsOf(e);
  if (!e.keyCode || MOD_KEYS.has(e.keyCode)) {
    $("hotkey").textContent = mods ? `${modsLabel(mods)} + …` : "Press a shortcut…";
    return;
  }
  if (!mods) {
    $("hotkey").textContent = "Add Ctrl, Alt, Shift or Win";
    return;
  }
  recording = false;
  $("hotkey").classList.remove("rec");
  Object.assign(S.settings, { HotkeyModifiers: mods, HotkeyKey: e.keyCode });
  try {
    S = await invoke("update", { patch: { HotkeyModifiers: mods, HotkeyKey: e.keyCode } });
  } finally {
    await invoke("pause_hotkey", { paused: false });
    S = await invoke("get_state");
    render();
  }
}, true);
window.addEventListener("blur", stopRecording);

document.querySelectorAll("[data-switch]").forEach((row) => row.addEventListener("click", () => {
  const key = row.dataset.switch;
  send({ [key]: !S.settings[key] });
}));

// ───────────── look ─────────────
document.querySelectorAll("#lookSeg .chip").forEach((c) => c.addEventListener("click", () => send({ Look: c.dataset.v })));
document.querySelectorAll("#themeSeg .chip").forEach((c) => c.addEventListener("click", () => send({ Theme: c.dataset.v })));

// ───────────── window keys and key feel ─────────────
$("wkMin").addEventListener("click", () => win.minimize());
$("wkClose").addEventListener("click", () => win.close());
// Pocket keys spring back past the top when let go
document.addEventListener("pointerup", (e) => {
  const key = e.target.closest(".btn");
  if (!key || root.dataset.look !== "pocket") return;
  key.classList.remove("ack");
  void key.offsetWidth;
  key.classList.add("ack");
});
document.addEventListener("animationend", (e) => e.target.classList?.remove("ack", "boot"));

// ───────────── start ─────────────
listen("state", (e) => {
  if (inflight || pending) return;
  S = e.payload;
  render();
});

(async () => {
  S = await invoke("get_state");
  applyLook();
  paintIcons();
  render();
  await document.fonts.ready;
  await win.show();
  await win.setFocus();
})();
