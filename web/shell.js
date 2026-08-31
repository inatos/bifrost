const $ = (id) => document.getElementById(id);

/** Live room+ticket for this tab. Persisted briefly so refresh can leave the lobby. */
const sessionState = {
  room: null,
  ticket: null,
};
const ONLINE_SESSION_KEY = "bifrost-online-session";

let wasmReady = false;
let panel;
let status;
let roomInput;
let inspector;
let arenaHud;
let hudScore;
let hudCaller;
/** Once true, keep results visible until Play Again / Quit. */
let matchLatched = false;
/** Shell-level pre-match (embed) before wasm match starts. */
let preMatchOpen = false;
/** Rematch vote — CPU always votes yes. */
let rematchP0 = false;
/** User asked to start before Trunk finished WASM init. */
let pendingBotStart = false;
/** Join/Create succeeded before WASM ready — connect once modules load. */
let pendingOnlineConnect = null;
/** Quit pressed — suppress match_over results re-latching until a new match starts. */
let userQuit = false;
/** Suppress results panel while rematch countdown / new match boots. */
let suppressResults = false;
/** Warn once if WASM never becomes ready. */
let wasmLoadWarned = false;
/** Online lobby: idle | host_wait | guest_wait | ready | match */
let lobbyPhase = "idle";
/** host | guest | null */
let lobbyRole = null;
/** bot | create | join | null — which lobby control is active */
let playMode = null;
let opponentName = "P2";
/** Pre-match / results pad-keyboard focus: ready | quit | play-again | results-quit */
let uiFocus = "ready";
let lobbyWaitOpen = false;
/** Last known player count from room poll (for leave detection). */
let lobbyPlayerCount = 0;
let lastPadNavAt = 0;
let lastPadConfirm = false;
let lastPadEast = false;
let roomPollTimer = 0;

const PLAYER_NAME_KEY = "bifrost-player-name";

function typingInFormField(target) {
  if (!target || !(target instanceof Element)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  return !!target.closest?.("input, textarea, select, [contenteditable='true']");
}

/** Room code currently in the Join/Create field (if any). */
function roomCodeFromFields() {
  return String($("embed-room-code")?.value || roomInput?.value || "")
    .trim()
    .toUpperCase();
}

/** Prefer Join when a code is present — never start a bot mid-join by accident. */
function startFromPreMatchConfirm() {
  if (sessionState.ticket) {
    setStatus("Online lobby is active — wait for your opponent, or Quit first.");
    return false;
  }
  const code = roomCodeFromFields();
  if (code || playMode === "join") {
    if (!code) {
      setStatus("Enter a room code, then Join / Launch.");
      $("embed-room-code")?.focus();
      return false;
    }
    void joinRoomWithCode(code);
    return true;
  }
  return requestBotStart();
}

function isEmbedded() {
  return (
    document.documentElement.classList.contains("embedded") ||
    document.body?.classList.contains("embedded")
  );
}

function getPlayerName() {
  const fallback = lobbyRole === "guest" ? "P2" : "P1";
  try {
    const raw = sessionStorage.getItem(PLAYER_NAME_KEY) || "";
    return raw.slice(0, 7).trim() || fallback;
  } catch {
    return fallback;
  }
}

function setPlayerName(raw) {
  const name = String(raw || "")
    .slice(0, 7)
    .trim();
  try {
    if (name) sessionStorage.setItem(PLAYER_NAME_KEY, name);
    else sessionStorage.removeItem(PLAYER_NAME_KEY);
  } catch (_) {}
  syncPlayerNameFields();
  updatePlayerNameLabels();
  refreshScoreLabels();
  const next = name || "P1";
  clearTimeout(window.__bifrostNameDebounce);
  window.__bifrostNameDebounce = setTimeout(() => showNameToast(next), 450);
  return next;
}

function syncPlayerNameFields() {
  const name = getPlayerName();
  const embedName = $("embed-player-name");
  if (embedName && document.activeElement !== embedName) embedName.value = name === "P1" ? "" : name;
}

function updatePlayerNameLabels() {
  const name = getPlayerName();
  const preP0 = $("pre-p0");
  if (preP0 && !preP0.classList.contains("is-ready")) {
    preP0.textContent = `${name} — press Ready`;
  }
  const rematchP0El = $("rematch-p0");
  if (rematchP0El && !rematchP0El.classList.contains("is-ready")) {
    rematchP0El.textContent = `${name} — press Play Again`;
  }
  const lobbyNames = $("lobby-wait-names");
  if (lobbyNames && lobbyWaitOpen) {
    const you = name;
    const them = opponentName || "…";
    lobbyNames.textContent =
      lobbyRole === "guest"
        ? `${them} (host) · ${you} (you)`
        : `${you} (you) · ${them} (opponent)`;
  }
}

function refreshScoreLabels() {
  if (!hudScore) return;
  // Last scores are refreshed from syncHudLoop; this only updates name prefixes mid-match.
  const raw = wasmApi()?.bifrost_hud?.();
  if (!raw) return;
  try {
    const hud = JSON.parse(raw);
    if (hud?.in_game) applyScoreText(hud);
  } catch (_) {}
}

function applyScoreText(hud) {
  if (!hudScore) return;
  const me = getPlayerName();
  const them = hud.bot ? "CPU" : opponentName || "P2";
  hudScore.textContent = `${me} ${hud.score[0]} — ${hud.score[1]} ${them}`;
}

function syncOverlayClass() {
  const wrap = document.querySelector(".arena-wrap");
  if (!wrap) return;
  const open =
    preMatchOpen ||
    lobbyWaitOpen ||
    matchLatched ||
    !$("results")?.classList.contains("hidden") ||
    !$("ready-up")?.classList.contains("hidden") ||
    !$("countdown")?.classList.contains("hidden");
  wrap.classList.toggle("is-overlay-open", !!open);
}

function setLaunchEnabled(enabled) {
  for (const id of ["btn-launch", "btn-embed-launch"]) {
    const btn = $(id);
    if (!btn) continue;
    btn.disabled = !enabled;
    btn.classList.toggle("is-disabled", !enabled);
    btn.title = enabled
      ? "Start bot match or connect with a room code"
      : sessionState.ticket
        ? "Lobby connected — Quit to unlock Launch"
        : "Create/Join a room, or clear the code for a bot match";
  }
}

function updateLaunchEnabled() {
  // Once Create/Join succeeds (ticket held), Launch stays grey until Quit / leave.
  if (sessionState.ticket) {
    setLaunchEnabled(false);
    return;
  }
  setLaunchEnabled(true);
}

function persistOnlineSession() {
  try {
    if (sessionState.room && sessionState.ticket) {
      sessionStorage.setItem(
        ONLINE_SESSION_KEY,
        JSON.stringify({
          room: sessionState.room,
          ticket: sessionState.ticket,
          role: lobbyRole,
        })
      );
    } else {
      sessionStorage.removeItem(ONLINE_SESSION_KEY);
    }
  } catch (_) {}
}

function clearPersistedOnlineSession() {
  try {
    sessionStorage.removeItem(ONLINE_SESSION_KEY);
  } catch (_) {}
}

function readPersistedOnlineSession() {
  try {
    const raw = sessionStorage.getItem(ONLINE_SESSION_KEY);
    if (!raw) return null;
    const data = JSON.parse(raw);
    if (!data?.room || !data?.ticket) return null;
    return data;
  } catch (_) {
    return null;
  }
}

/** Best-effort leave during unload (refresh / close tab). */
function leaveRoomBeacon(room, ticket) {
  if (!room || !ticket) return;
  const body = JSON.stringify({
    protocol_version: 1,
    room_code: room,
    ticket,
  });
  const url = "/api/rooms/leave";
  try {
    if (navigator.sendBeacon) {
      const ok = navigator.sendBeacon(
        url,
        new Blob([body], { type: "application/json" })
      );
      if (ok) return;
    }
  } catch (_) {}
  try {
    fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
      keepalive: true,
    });
  } catch (_) {}
}

function leaveOnlineSessionOnUnload() {
  // Keep sessionStorage so a failed beacon can still be reclaimed on next load.
  if (sessionState.room && sessionState.ticket) {
    leaveRoomBeacon(sessionState.room, sessionState.ticket);
    return;
  }
  const persisted = readPersistedOnlineSession();
  if (persisted) {
    leaveRoomBeacon(persisted.room, persisted.ticket);
  }
}

/** If unload leave missed, drop any leftover ticket on next load. */
async function reclaimStaleOnlineSession() {
  const persisted = readPersistedOnlineSession();
  if (!persisted) return;
  clearPersistedOnlineSession();
  try {
    await api("/api/rooms/leave", {
      protocol_version: 1,
      room_code: persisted.room,
      ticket: persisted.ticket,
    });
  } catch (_) {}
}

function clearRoomFields() {
  sessionState.room = null;
  sessionState.ticket = null;
  clearPersistedOnlineSession();
  if (roomInput) {
    roomInput.value = "";
    roomInput.readOnly = false;
  }
  const embedRoom = $("embed-room-code");
  if (embedRoom) {
    embedRoom.value = "";
    embedRoom.readOnly = false;
  }
  syncPlayModeUi();
}

function setPlayMode(mode) {
  playMode = mode;
  syncPlayModeUi();
}

function syncPlayModeUi() {
  const map = {
    bot: ["btn-bot", "btn-embed-bot"],
    create: ["btn-create", "btn-embed-create"],
    join: ["btn-join", "btn-embed-join"],
  };
  for (const [mode, ids] of Object.entries(map)) {
    for (const id of ids) {
      const el = $(id);
      if (!el) continue;
      const on = playMode === mode;
      el.classList.toggle("is-active", on);
      el.setAttribute("aria-pressed", on ? "true" : "false");
    }
  }
  const hosting = !!(sessionState.ticket && lobbyRole === "host");
  const joining = playMode === "join" && !hosting;
  const embedRoom = $("embed-room-code");
  if (embedRoom) {
    embedRoom.readOnly = hosting;
    embedRoom.title = hosting
      ? "Host room code (share with a friend)"
      : "Paste a room code from a friend";
  }
  if (roomInput) {
    roomInput.readOnly = hosting;
  }
  const copyBtn = $("btn-embed-copy");
  if (copyBtn) {
    copyBtn.dataset.mode = joining ? "paste" : "copy";
    copyBtn.title = joining ? "Paste room code" : "Copy room code";
    copyBtn.setAttribute("aria-label", joining ? "Paste room code" : "Copy room code");
    copyBtn.classList.toggle("is-paste", joining);
    const svg = copyBtn.querySelector("svg");
    if (svg) {
      svg.innerHTML = joining
        ? '<path d="M12 2v10"/><path d="m8 8 4 4 4-4"/><path d="M4 14v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4"/>'
        : '<rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>';
    }
  }
}

function syncRoomField(code) {
  const c = (code || sessionState.room || "").trim().toUpperCase();
  if (!c) return;
  if (roomInput) roomInput.value = c;
  const embedRoom = $("embed-room-code");
  if (embedRoom) embedRoom.value = c;
}

function showNameToast(name) {
  showPromptToast(`Playing as ${name}`);
}

function showPromptToast(message) {
  let toast = $("name-toast");
  if (!toast) {
    toast = document.createElement("div");
    toast.id = "name-toast";
    toast.className = "name-toast";
    toast.setAttribute("aria-live", "polite");
    document.body.appendChild(toast);
  }
  toast.textContent = message;
  toast.classList.add("is-visible");
  clearTimeout(window.__bifrostNameToast);
  window.__bifrostNameToast = setTimeout(() => {
    toast.classList.remove("is-visible");
  }, 3200);
}

/** Async copy confirmation toast (+ optional button flash). */
function flashCopied(code, opts = {}) {
  const label = code ? `Copied ${code}` : "Copied!";
  showPromptToast(label);
  setStatus(opts.status || `${label}${opts.suffix || ""}`);
  const el = opts.button;
  if (el) {
    el.classList.add("copied");
    const prevTitle = el.getAttribute("data-title-rest") || el.title || "Copy room code";
    el.setAttribute("data-title-rest", prevTitle);
    el.title = "Copied!";
    clearTimeout(window.__bifrostCopyFlash);
    window.__bifrostCopyFlash = setTimeout(() => {
      el.classList.remove("copied");
      el.title = el.getAttribute("data-title-rest") || "Copy room code";
    }, 1600);
  }
}

function onlineSessionHeld() {
  return !!(sessionState.room && sessionState.ticket);
}

function leaveConfirmMessage() {
  if (lobbyWaitOpen || lobbyPhase === "host_wait" || lobbyPhase === "guest_wait") {
    return lobbyRole === "host"
      ? "Close this lobby? Your friend will be prompted."
      : "Leave this lobby? The host will be prompted.";
  }
  return "Leave the match? Your opponent will be prompted.";
}

function confirmLeaveOnline() {
  if (!onlineSessionHeld()) return true;
  // Native window.confirm is blocked / returns false in many mobile iframes (Lab embed).
  if (isEmbedded()) return true;
  try {
    return window.confirm(leaveConfirmMessage());
  } catch (_) {
    return true;
  }
}

async function leaveRoomOnServer() {
  const room = sessionState.room;
  const ticket = sessionState.ticket;
  if (!room || !ticket) return;
  try {
    await api("/api/rooms/leave", {
      protocol_version: 1,
      room_code: room,
      ticket,
    });
  } catch (_) {
    /* room may already be gone */
  }
  clearPersistedOnlineSession();
}

function dismissOnlineSession(promptMsg) {
  if (promptMsg) {
    showPromptToast(promptMsg);
    setStatus(promptMsg);
  }
  userQuit = true;
  matchLatched = false;
  rematchP0 = false;
  pendingBotStart = false;
  pendingOnlineConnect = null;
  window.__bifrostReadyWanted = false;
  stopRoomPoll();
  hideLobbyWait();
  lobbyPhase = "idle";
  lobbyRole = null;
  try {
    window.__bifrostApplyPadSeatColors?.();
  } catch (_) {}
  lobbyPlayerCount = 0;
  opponentName = "P2";
  setPlayMode(null);
  clearRoomFields();
  hideCountdown();
  clearTimeout(window.__rematchTimer);
  window.__rematchTimer = 0;
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = 0;
  const panel = $("results");
  if (panel) {
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
  }
  $("ready-up")?.classList.add("hidden");
  updateLaunchEnabled();
  window.__bifrostPaused = true;
  try {
    const apiWasm = wasmApi();
    if (wasmReady && apiWasm?.bifrost_leave_match) apiWasm.bifrost_leave_match();
  } catch (_) {}
  if (isEmbedded()) {
    showPreMatch();
    const hint = $("pre-match")?.querySelector(".pre-match-hint");
    if (hint) hint.textContent = promptMsg || "Pick Bot, Create, or Join";
  } else {
    showLobby();
  }
  syncOverlayClass();
}

/** Peer drop mid-match / ready — toast and return to Match Menu. */
async function handleOpponentDisconnect(message) {
  if (userQuit || window.__bifrostHandlingDisconnect) return;
  // Ticket may already be cleared by a concurrent leave; still bounce if we were online.
  const wasOnline =
    onlineSessionHeld() ||
    lobbyPhase === "match" ||
    lobbyPhase === "ready" ||
    lobbyPhase === "host_wait" ||
    lobbyPhase === "guest_wait";
  if (!wasOnline) return;
  window.__bifrostHandlingDisconnect = true;
  const msg = message || "Opponent disconnected.";
  try {
    if (onlineSessionHeld()) await leaveRoomOnServer();
  } catch (_) {}
  dismissOnlineSession(msg);
  window.__bifrostHandlingDisconnect = false;
}

window.bifrostOpponentLeft = function bifrostOpponentLeft(message) {
  void handleOpponentDisconnect(
    typeof message === "string" && message.trim() ? message.trim() : "Opponent disconnected."
  );
};

function showLobbyWait(role, code) {
  lobbyWaitOpen = true;
  lobbyRole = role;
  try {
    window.__bifrostApplyPadSeatColors?.();
  } catch (_) {}
  lobbyPhase = role === "guest" ? "guest_wait" : "host_wait";
  lobbyPlayerCount = role === "guest" ? 2 : 1;
  userQuit = false;
  const room = (code || sessionState.room || "").trim().toUpperCase();
  if (room) {
    sessionState.room = room;
    syncRoomField(room);
  }
  hidePreMatch();
  $("results")?.classList.add("hidden");
  const el = $("lobby-wait");
  if (el) {
    el.classList.remove("hidden");
    const codeEl = $("lobby-wait-code");
    if (codeEl) codeEl.textContent = room || sessionState.room || "";
    const msg = $("lobby-wait-msg");
    if (msg) {
      msg.textContent =
        role === "guest"
          ? "Connected — match starts when both are ready…"
          : "Share this code — friend presses Join with it";
    }
  }
  updatePlayerNameLabels();
  syncOverlayClass();
  updateLaunchEnabled();
  syncPlayModeUi();
  startRoomPoll();
  setStatus(
    role === "guest"
      ? `Joined ${room || sessionState.room} — waiting…`
      : `Room ${room || sessionState.room} — give this code to a friend, then wait`
  );
}

function hideLobbyWait() {
  lobbyWaitOpen = false;
  $("lobby-wait")?.classList.add("hidden");
  // Keep polling while a ticket is held so leave/close is detected in ready/match.
  if (!onlineSessionHeld()) stopRoomPoll();
  syncOverlayClass();
}

function startRoomPoll() {
  stopRoomPoll();
  const tick = async () => {
    if (!onlineSessionHeld() || userQuit) return;
    try {
      const res = await fetch(`/api/rooms/${encodeURIComponent(sessionState.room)}`);
      if (res.status === 404) {
        // Host closed / room expired — prompt remaining player.
        dismissOnlineSession(
          lobbyRole === "guest" ? "Host closed the lobby." : "Lobby closed."
        );
        return;
      }
      if (!res.ok) return;
      const data = await res.json();
      const players = Number(data.players) || 0;
      const host = (data.host_name || "").trim() || "Host";
      const guest = (data.guest_name || "").trim();
      if (lobbyRole === "host") {
        opponentName = guest || "…";
        if (players >= 2) {
          lobbyPhase = "ready";
          const msg = $("lobby-wait-msg");
          if (msg && lobbyWaitOpen) msg.textContent = "Opponent joined — starting…";
        } else if (lobbyPlayerCount >= 2 && players < 2) {
          // Guest left after joining (guest_name already cleared server-side).
          const leftName =
            opponentName && opponentName !== "…" ? opponentName : "Opponent";
          showPromptToast(`${leftName} left the lobby.`);
          setStatus(`${leftName} left the lobby.`);
          opponentName = "…";
          if (lobbyWaitOpen) {
            lobbyPhase = "host_wait";
            const msg = $("lobby-wait-msg");
            if (msg) msg.textContent = "Share this code — friend presses Join with it";
          } else {
            // Left during ready/match — bounce host to match menu.
            dismissOnlineSession(`${leftName} left the lobby.`);
            return;
          }
        }
      } else {
        opponentName = host;
      }
      lobbyPlayerCount = players;
      updatePlayerNameLabels();
      updateLaunchEnabled();
    } catch (_) {}
  };
  tick();
  roomPollTimer = setInterval(tick, 900);
}

function stopRoomPoll() {
  if (roomPollTimer) {
    clearInterval(roomPollTimer);
    roomPollTimer = 0;
  }
}

function applyUiFocusStyles() {
  const map = {
    ready: "btn-pre-ready",
    quit: "btn-pre-quit",
    "play-again": "btn-play-again",
    "results-quit": "btn-quit",
  };
  for (const id of Object.values(map)) {
    $(id)?.classList.remove("ui-focus");
  }
  const id = map[uiFocus];
  if (id) $(id)?.classList.add("ui-focus");
}

function cycleUiFocus(dir) {
  if (preMatchOpen) {
    uiFocus = uiFocus === "ready" ? "quit" : "ready";
  } else if (matchLatched) {
    uiFocus = uiFocus === "play-again" ? "results-quit" : "play-again";
  } else {
    return;
  }
  applyUiFocusStyles();
}

function confirmUiFocus() {
  if (preMatchOpen) {
    if (uiFocus === "quit") quitToMenu();
    else startFromPreMatchConfirm();
    return;
  }
  if (matchLatched) {
    if (uiFocus === "results-quit") quitToMenu();
    else voteRematch();
  }
}

function quitUiFocus() {
  if (preMatchOpen || matchLatched || lobbyWaitOpen) quitToMenu();
}

function showPreMatch() {
  preMatchOpen = true;
  window.__bifrostPaused = true;
  matchLatched = false;
  rematchP0 = false;
  hideLobbyWait();
  lobbyPhase = "idle";
  lobbyRole = null;
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = 0;
  $("results")?.classList.add("hidden");
  $("ready-up")?.classList.add("hidden");
  const pre = $("pre-match");
  if (pre) {
    pre.classList.remove("hidden");
    const preP0 = $("pre-p0");
    if (preP0) {
      preP0.classList.remove("is-ready");
      preP0.textContent = `${getPlayerName()} — press Ready`;
    }
  }
  uiFocus = "ready";
  applyUiFocusStyles();
  showArenaHud();
  setStatus("Ready up to start a bot match.");
  syncOverlayClass();
  updateLaunchEnabled();
}

function hidePreMatch() {
  preMatchOpen = false;
  $("pre-match")?.classList.add("hidden");
  syncOverlayClass();
}

function voteRematch() {
  if (!matchLatched) return;
  rematchP0 = true;
  const el = $("rematch-p0");
  if (el) {
    el.classList.add("is-ready");
    el.textContent = `${getPlayerName()} Ready`;
  }
  // Latch JUMP into the sim for both bot + online. MatchOver accepts rematch votes;
  // when both seats are ready the sim returns to Readying (do NOT tear down to bot).
  window.__bifrostReadyWanted = true;
  pulseReadyJump();
  clearTimeout(window.__rematchTimer);
  window.__rematchTimer = 0;
}

function wasmApi() {
  return window.wasmBindings ?? null;
}

function setStatus(msg) {
  if (status) status.textContent = msg;
  const embedStatus = $("embed-status");
  if (embedStatus) embedStatus.textContent = msg;
}

function hideLobby() {
  if (panel) panel.classList.add("hidden");
  document.querySelector(".stage")?.classList.remove("has-panel");
}

function showLobby() {
  if (panel) panel.classList.remove("hidden");
  document.querySelector(".stage")?.classList.add("has-panel");
}

function onWasmReady() {
  if (wasmReady) return;
  wasmReady = true;
  clearTimeout(window.__bifrostWasmTimeout);
  window.__bifrostWasmTimeout = 0;
  clearTimeout(window.__bifrostWasmPoll);
  window.__bifrostWasmPoll = 0;
  bindHud();
  syncPlayerNameFields();
  const waitingOnRoom = hydrateFromQuery();
  requestAnimationFrame(() => {
    focusPlaySurface();
    if (waitingOnRoom) {
      /* room link — stay in lobby until join */
    } else if (pendingOnlineConnect) {
      const pending = pendingOnlineConnect;
      pendingOnlineConnect = null;
      pendingBotStart = false;
      void connectOnline(pending.room, pending.ticket, pending.role);
    } else if (pendingBotStart) {
      pendingBotStart = false;
      if (!sessionState.ticket) startBotMatch();
    } else if (isEmbedded()) {
      if (!preMatchOpen && !lobbyWaitOpen) showPreMatch();
      else if (!lobbyWaitOpen) setStatus("Ready up to start a bot match.");
    } else {
      startBotMatch();
    }
    requestAnimationFrame(() => focusPlaySurface());
  });
}

function layoutHudTag(el, nx, ny, view) {
  if (!el) return;
  const v = view ?? { left: 0, top: 0, width: 1, height: 1 };
  el.style.left = `${(v.left + nx * v.width) * 100}%`;
  el.style.top = `${(v.top + ny * v.height) * 100}%`;
}

function hideCountdown() {
  const el = $("countdown");
  if (!el) return;
  el.classList.add("hidden");
  el.setAttribute("aria-hidden", "true");
  clearTimeout(window.__bifrostCountdownTimer);
  window.__bifrostCountdownTimer = 0;
}

/** 3-2-1 beat, then run `onDone`. Keeps sim paused until then. */
function runCountdown(onDone) {
  const el = $("countdown");
  const num = $("countdown-num");
  if (!el || !num) {
    onDone();
    return;
  }
  hidePreMatch();
  window.__bifrostPaused = true;
  el.classList.remove("hidden");
  el.setAttribute("aria-hidden", "false");
  const beats = ["3", "2", "1", "GO"];
  let i = 0;
  const tick = () => {
    num.textContent = beats[i];
    el.classList.toggle("go", beats[i] === "GO");
    i += 1;
    if (i >= beats.length) {
      window.__bifrostCountdownTimer = setTimeout(() => {
        hideCountdown();
        onDone();
      }, 420);
      return;
    }
    window.__bifrostCountdownTimer = setTimeout(tick, 700);
  };
  tick();
}

function showArenaHud() {
  if (!arenaHud) return;
  arenaHud.hidden = false;
  arenaHud.setAttribute("aria-hidden", "false");
}

function bindHud() {
  arenaHud = $("arena-hud");
  hudScore = $("hud-score");
  hudCaller = $("hud-caller");
  const playAgain = $("btn-play-again");
  const quit = $("btn-quit");
  const resultsLeave = $("btn-results-leave") || $("btn-results-menu");
  const readyLeave = $("btn-ready-leave") || $("btn-ready-menu");
  const readyQuit = $("btn-ready-quit");
  bindReadyHold();
  if (playAgain) {
    playAgain.addEventListener("click", (e) => {
      e.stopPropagation();
      voteRematch();
    });
  }
  if (quit) {
    quit.addEventListener("click", (e) => {
      e.stopPropagation();
      void quitToMenu();
    });
  }
  if (resultsLeave) {
    resultsLeave.addEventListener("click", (e) => {
      e.stopPropagation();
      void returnToMatchMenu();
    });
  }
  if (readyLeave) {
    readyLeave.addEventListener("click", (e) => {
      e.stopPropagation();
      void returnToMatchMenu();
    });
  }
  if (readyQuit) {
    readyQuit.addEventListener("click", (e) => {
      e.stopPropagation();
      void quitToMenu();
    });
  }
  requestAnimationFrame(syncHudLoop);
}

/** Hold-to-ready (~Arathyll overlay-back ring). */
const READY_HOLD_MS = 629;
let readyHoldStart = 0;
let readyHoldRaf = 0;

function resetReadyHoldUi() {
  readyHoldStart = 0;
  if (readyHoldRaf) {
    cancelAnimationFrame(readyHoldRaf);
    readyHoldRaf = 0;
  }
  const btn = $("btn-ready-hold");
  if (!btn) return;
  btn.classList.remove("holding");
  const ring = btn.querySelector(".ready-hold-ring");
  if (ring) ring.style.setProperty("--hold", "0");
}

function tickReadyHold() {
  if (readyHoldStart <= 0) return;
  const btn = $("btn-ready-hold");
  if (!btn || btn.disabled || !readyUpOpen()) {
    resetReadyHoldUi();
    return;
  }
  const progress = Math.min(1, (performance.now() - readyHoldStart) / READY_HOLD_MS);
  btn.classList.toggle("holding", progress > 0);
  const ring = btn.querySelector(".ready-hold-ring");
  if (ring) ring.style.setProperty("--hold", String(progress));
  if (progress >= 1) {
    resetReadyHoldUi();
    pulseReadyJump();
    return;
  }
  readyHoldRaf = requestAnimationFrame(tickReadyHold);
}

function bindReadyHold() {
  const btn = $("btn-ready-hold");
  if (!btn || btn.dataset.bound === "1") return;
  btn.dataset.bound = "1";
  const begin = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (btn.disabled || !readyUpOpen()) return;
    try {
      btn.setPointerCapture?.(e.pointerId);
    } catch (_) {}
    readyHoldStart = performance.now();
    if (readyHoldRaf) cancelAnimationFrame(readyHoldRaf);
    readyHoldRaf = requestAnimationFrame(tickReadyHold);
  };
  const end = (e) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      btn.releasePointerCapture?.(e.pointerId);
    } catch (_) {}
    resetReadyHoldUi();
  };
  btn.addEventListener("pointerdown", begin);
  btn.addEventListener("pointerup", end);
  btn.addEventListener("pointerleave", end);
  btn.addEventListener("pointercancel", end);
  btn.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
}

/** Tear down lobby / match online state so the next Create/Join starts clean. */
async function teardownOnlineSession({ confirm = true } = {}) {
  if (confirm && !confirmLeaveOnline()) return false;
  if (onlineSessionHeld()) await leaveRoomOnServer();
  userQuit = true;
  matchLatched = false;
  rematchP0 = false;
  pendingBotStart = false;
  pendingOnlineConnect = null;
  window.__bifrostReadyWanted = false;
  resetReadyHoldUi();
  stopRoomPoll();
  hideLobbyWait();
  lobbyPhase = "idle";
  lobbyRole = null;
  try {
    window.__bifrostApplyPadSeatColors?.();
  } catch (_) {}
  lobbyPlayerCount = 0;
  opponentName = "P2";
  setPlayMode(null);
  clearRoomFields();
  hideCountdown();
  clearTimeout(window.__rematchTimer);
  window.__rematchTimer = 0;
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = 0;
  const panel = $("results");
  if (panel) {
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
  }
  $("ready-up")?.classList.add("hidden");
  updateLaunchEnabled();
  window.__bifrostPaused = true;
  try {
    const api = wasmApi();
    if (wasmReady && api?.bifrost_leave_match) {
      api.bifrost_leave_match();
    }
  } catch (_) {}
  syncOverlayClass();
  return true;
}

/** Leave lobby / results but stay in Bifrost match options (Lab stays open). */
async function returnToMatchMenu() {
  if (!(await teardownOnlineSession())) return;
  setStatus("Match menu — Bot, Create, or Join");
  if (isEmbedded()) {
    showPreMatch();
    const hint = $("pre-match")?.querySelector(".pre-match-hint");
    if (hint) hint.textContent = "Pick Bot, Create, or Join";
  } else {
    showLobby();
  }
  syncOverlayClass();
}

async function quitToMenu() {
  if (!(await teardownOnlineSession())) return;
  setStatus("Have a good one!");
  try {
    window.parent?.postMessage({ type: "bifrost-quit", message: "Have a good one!" }, "*");
  } catch (_) {
    /* ignore cross-origin */
  }
  if (isEmbedded()) {
    showPreMatch();
    const hint = $("pre-match")?.querySelector(".pre-match-hint");
    if (hint) hint.textContent = "Have a good one!";
    const embedStatus = $("embed-status");
    if (embedStatus) embedStatus.textContent = "Have a good one!";
    return;
  }
  showLobby();
  if (arenaHud) {
    arenaHud.hidden = true;
    arenaHud.setAttribute("aria-hidden", "true");
  }
  syncOverlayClass();
}

function syncHudLoop() {
  requestAnimationFrame(syncHudLoop);
  if (!wasmReady || !arenaHud) return;
  const raw = wasmApi()?.bifrost_hud?.();
  if (!raw) return;
  let hud;
  try {
    hud = JSON.parse(raw);
  } catch {
    return;
  }

  const lobby = hud.lobby || {};
  if (lobby.phase === "disconnected" && !userQuit) {
    void handleOpponentDisconnect(lobby.status || "Opponent disconnected.");
    return;
  }
  if (lobby.status) {
    const waiting = !!lobby.waiting || lobby.phase === "host_wait" || lobby.phase === "guest_wait";
    // Only treat online HUD as lobby — a leftover bot InGame must not dismiss wait.
    if (waiting && sessionState.room && sessionState.ticket && !(!hud.bot && hud.in_game)) {
      if (!lobbyWaitOpen) showLobbyWait(lobbyRole || "host", sessionState.room);
      setStatus(lobby.status);
      if (lobby.phase) lobbyPhase = lobby.phase;
      updateLaunchEnabled();
    }
    if (lobby.phase === "ready" && lobbyWaitOpen) {
      lobbyPhase = "ready";
      const msg = $("lobby-wait-msg");
      if (msg) msg.textContent = "Opponent joined — starting…";
      updateLaunchEnabled();
    }
    if (sessionState.ticket && (lobby.phase === "match" || (hud.in_game && !hud.bot))) {
      if (lobbyWaitOpen) hideLobbyWait();
      lobbyPhase = "match";
      window.__bifrostPaused = false;
      updateLaunchEnabled();
      if (hud.phase === "readying") {
        setStatus("Ready up — both must confirm");
      } else if (hud.in_game) {
        setStatus("In match — WASD / arrows / mouse / gamepad");
        collapseEmbedBarForMatch();
      }
      try {
        window.parent?.postMessage({ type: "bifrost-game-start" }, "*");
      } catch (_) {}
    }
  }

  if (!hud.in_game) {
    if (matchLatched || preMatchOpen || lobbyWaitOpen) {
      if (matchLatched) showArenaHud();
      syncOverlayClass();
      return;
    }
    arenaHud.hidden = true;
    arenaHud.setAttribute("aria-hidden", "true");
    syncOverlayClass();
    return;
  }
  showArenaHud();
  updatePlayerNameLabels();
  applyScoreText(hud);
  if (hudCaller) {
    const me = getPlayerName();
    const them = hud.bot ? "CPU" : opponentName || "P2";
    const owner =
      hud.owner === 0 ? me : hud.owner === 1 ? them : "Neutral";
    const rounds = hud.rounds ?? [0, 0];
    const stock = (n) =>
      "◆".repeat(Math.min(2, Number(n) || 0)) + "◇".repeat(Math.max(0, 2 - (Number(n) || 0)));
    const remain = Number(hud.round_remain ?? hud.roundRemain ?? 0);
    const secs = Math.max(0, Math.ceil(remain / 60));
    const mm = String(Math.floor(secs / 60)).padStart(1, "0");
    const ss = String(secs % 60).padStart(2, "0");
    const breaks = hud.round_breaks ?? hud.roundBreaks ?? [0, 0];
    hudCaller.textContent = `Caller ${owner} · ${mm}:${ss} · Breaks ${breaks[0]}–${breaks[1]} · Match ${stock(rounds[0])}–${stock(rounds[1])}`;
    hudCaller.classList.toggle("owner-p1", hud.owner === 0);
    hudCaller.classList.toggle("owner-p2", hud.owner === 1);
  }
  syncResults(hud);
  syncReady(hud);
  syncOverlayClass();
  try {
    window.__bifrostApplyPadSeatColors?.();
  } catch (_) {}
}

function syncReady(hud) {
  const el = $("ready-up");
  if (!el) return;
  const phase = hud.phase || "";
  const show = phase === "readying" && !matchLatched;
  el.classList.toggle("hidden", !show);
  if (!show) {
    window.__bifrostReadyWanted = false;
    resetReadyHoldUi();
    return;
  }
  showArenaHud();
  const ready = hud.ready || [false, false];
  const bot = !!hud.bot;
  // Prefer GGRS local seat when present; fall back to lobby role.
  let localSeat = lobbyRole === "guest" ? 1 : 0;
  try {
    const seats = hud.local_seats || hud.localSeats;
    if (Array.isArray(seats) && seats.length > 0) {
      localSeat = Number(seats[0]) | 0;
    }
  } catch (_) {}
  const youReady = !!ready[localSeat];
  const themReady = bot ? true : !!ready[1 - localSeat];
  if (youReady) window.__bifrostReadyWanted = false;
  else if (window.__bifrostReadyWanted) pulseReadyJump();
  const hint = $("ready-hint");
  if (hint) {
    hint.textContent = youReady
      ? bot
        ? "You are ready · CPU always readies"
        : themReady
          ? "Both ready — starting…"
          : "Waiting for opponent…"
      : bot
        ? "Hold Ready · CPU always readies"
        : "Hold Ready — both players must confirm";
  }
  const holdBtn = $("btn-ready-hold");
  if (holdBtn) {
    holdBtn.classList.toggle("is-ready", youReady);
    // Only disable once this seat is actually latched ready — never stick disabled.
    holdBtn.disabled = !!youReady;
    holdBtn.classList.toggle("is-disabled", !!youReady);
    holdBtn.setAttribute("aria-disabled", youReady ? "true" : "false");
    const label = holdBtn.querySelector(".ready-hold-label");
    if (label) label.textContent = youReady ? "Ready!" : "Ready";
    if (youReady) resetReadyHoldUi();
  }
  const p0 = $("ready-p0");
  const p1 = $("ready-p1");
  const youLabel = getPlayerName();
  if (p0) {
    p0.classList.toggle("is-ready", youReady);
    p0.textContent = youReady
      ? `You (${youLabel}) Ready`
      : `You (${youLabel}) — hold Ready`;
  }
  if (p1) {
    const them = bot ? "CPU" : opponentName || (lobbyRole === "guest" ? "Host" : "Opponent");
    p1.classList.toggle("is-ready", themReady);
    p1.textContent = themReady
      ? `${them} Ready`
      : bot
        ? "CPU…"
        : `${them} — waiting`;
  }
}

function syncResults(hud) {
  const panel = $("results");
  if (!panel) return;
  const phase = (hud.phase || "").toLowerCase();

  // Rematch accepted — sim left MatchOver for Readying / serve.
  if (matchLatched && phase && phase !== "match_over") {
    matchLatched = false;
    rematchP0 = false;
    window.__bifrostReadyWanted = false;
    suppressResults = false;
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
    syncOverlayClass();
    return;
  }

  if (userQuit || suppressResults) {
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
    // Clear suppress once the sim has left match_over.
    if (suppressResults && hud && !hud.match_over && !hud.matchOver) {
      if (phase && phase !== "match_over") {
        suppressResults = false;
      }
    }
    return;
  }
  const rounds = hud.rounds ?? [0, 0];
  const over = !!(
    matchLatched ||
    hud.match_over ||
    hud.matchOver ||
    phase === "match_over" ||
    rounds[0] >= 2 ||
    rounds[1] >= 2
  );
  if (!over) {
    // Leave ephemeral round banners alone until their timer clears.
    if (window.__bifrostRoundBanner) return;
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
    return;
  }
  const freshlyLatched = !matchLatched;
  matchLatched = true;
  if (freshlyLatched) {
    rematchP0 = false;
    uiFocus = "play-again";
    applyUiFocusStyles();
    const rematchP0El = $("rematch-p0");
    if (rematchP0El) {
      rematchP0El.classList.remove("is-ready");
      rematchP0El.textContent = `${getPlayerName()} — press Play Again`;
    }
  }

  // Drive rematch votes from sim ready[] (JUMP while MatchOver).
  const readyBits = hud.ready || [false, false];
  const bot = !!hud.bot;
  const localSeat = lobbyRole === "guest" ? 1 : 0;
  const youReady = !!readyBits[localSeat];
  const themReady = bot ? true : !!readyBits[1 - localSeat];
  if (youReady) {
    rematchP0 = true;
    window.__bifrostReadyWanted = false;
  } else if (window.__bifrostReadyWanted || rematchP0) {
    pulseReadyJump();
  }
  const rematchP0El = $("rematch-p0");
  if (rematchP0El) {
    rematchP0El.classList.toggle("is-ready", youReady);
    rematchP0El.textContent = youReady
      ? `${getPlayerName()} Ready`
      : `${getPlayerName()} — press Play Again`;
  }
  const rematchCpu = $("rematch-cpu");
  if (rematchCpu) {
    const them = bot ? "CPU" : opponentName || (lobbyRole === "guest" ? "Host" : "Opponent");
    rematchCpu.classList.toggle("is-ready", themReady);
    rematchCpu.textContent = themReady
      ? `${them} Ready`
      : bot
        ? "CPU Ready"
        : `${them} — waiting`;
  }

  showArenaHud();
  panel.classList.remove("hidden");
  panel.classList.remove("round-only");
  const ready = $("ready-up");
  if (ready) ready.classList.add("hidden");
  const title = $("results-title");
  const detail = $("results-detail");
  const winnerIdx =
    hud.winner ?? hud.winnerIndex ?? (rounds[0] >= 2 ? 0 : rounds[1] >= 2 ? 1 : 0);
  const winnerName =
    winnerIdx === 0 ? getPlayerName() : hud.bot ? "CPU" : opponentName || "P2";
  if (title) title.textContent = `${winnerName} Wins`;
  syncOverlayClass();
  const score = hud.score ?? [0, 0];
  if (detail) {
    const st = hud.stats ?? {};
    const rallySec = ((st.longest_rally ?? 0) / 60).toFixed(1);
    const bricks = st.bricks_broken ?? [0, 0];
    const goals = st.goals ?? score;
    detail.textContent = `Rounds ${rounds[0]} — ${rounds[1]} · Final ${score[0]} — ${score[1]} · Bricks ${bricks[0]}/${bricks[1]} · Goals ${goals[0]}/${goals[1]} · Longest rally ${rallySec}s · Wild bursts ${st.wild_burst ?? 0} · Spins ${st.spins ?? 0}`;
  }
}

/** Called from wasm juice on RoundWin — forces a HUD refresh + match panel when done. */
window.bifrostRoundWin = function bifrostRoundWin(winner, matchOverArg) {
  let hud = null;
  const raw = wasmApi()?.bifrost_hud?.();
  if (raw) {
    try {
      hud = JSON.parse(raw);
    } catch {
      hud = null;
    }
  }
  const rounds = hud?.rounds ?? [0, 0];
  const matchOver =
    matchOverArg === true ||
    matchOverArg === 1 ||
    !!(hud?.match_over || hud?.matchOver) ||
    rounds[0] >= 2 ||
    rounds[1] >= 2;
  if (matchOver) {
    matchLatched = true;
    clearTimeout(window.__bifrostRoundBanner);
    window.__bifrostRoundBanner = 0;
    if (hud) {
      hud.match_over = true;
      hud.winner = Number(winner);
      syncResults(hud);
    } else {
      syncResults({
        match_over: true,
        winner: Number(winner),
        rounds,
        score: [0, 0],
        bot: true,
        stats: {},
      });
    }
    return;
  }
  // Round win only — announce, no Play Again / Quit (those are match-complete).
  const panel = $("results");
  if (!panel) return;
  showArenaHud();
  panel.classList.remove("hidden");
  panel.classList.add("round-only");
  const title = $("results-title");
  const detail = $("results-detail");
  const name = Number(winner) === 0 ? getPlayerName() : hud?.bot ? "CPU" : opponentName || "P2";
  if (title) title.textContent = `${name} Takes The Round`;
  if (detail) detail.textContent = `Match ${rounds[0]} — ${rounds[1]} · First to 2`;
  syncOverlayClass();
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = setTimeout(() => {
    window.__bifrostRoundBanner = 0;
    if (!panel || matchLatched) return;
    let h = null;
    const next = wasmApi()?.bifrost_hud?.();
    try {
      h = next ? JSON.parse(next) : null;
    } catch (_) {}
    const r = h?.rounds ?? [0, 0];
    if (h?.match_over || r[0] >= 2 || r[1] >= 2) {
      syncResults(h ?? { match_over: true, winner: Number(winner), rounds: r, bot: true });
      return;
    }
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
    }, 2200);
};

/** Called from wasm juice on RoundTie. */
window.bifrostRoundTie = function bifrostRoundTie() {
  let hud = null;
  const raw = wasmApi()?.bifrost_hud?.();
  if (raw) {
    try {
      hud = JSON.parse(raw);
    } catch {
      hud = null;
    }
  }
  const panel = $("results");
  if (!panel) return;
  showArenaHud();
  panel.classList.remove("hidden");
  panel.classList.add("round-only");
  const title = $("results-title");
  const detail = $("results-detail");
  const rounds = hud?.rounds ?? [0, 0];
  if (title) title.textContent = "Round Draw";
  if (detail) detail.textContent = `Timeout tie · Match ${rounds[0]} — ${rounds[1]} · First to 2`;
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = setTimeout(() => {
    window.__bifrostRoundBanner = 0;
    if (!panel || matchLatched) return;
    panel.classList.add("hidden");
    panel.classList.remove("round-only");
  }, 2200);
};

function whenWasmReady() {
  const onStarted = () => onWasmReady();
  const onFailed = (e) => {
    const err = e.detail?.err ?? e.detail?.error;
    const msg = err?.message ?? String(err ?? "unknown error");
    pendingBotStart = false;
    pendingOnlineConnect = null;
    setStatus(`Wasm failed to load: ${msg}`);
    console.error("[bifrost] wasm init failed", err);
  };
  // Patch dispatches on window; older Trunk fired on document — listen to both.
  window.addEventListener("TrunkApplicationStarted", onStarted, { once: true });
  document.addEventListener("TrunkApplicationStarted", onStarted, { once: true });
  window.addEventListener("TrunkApplicationFailed", onFailed, { once: true });
  document.addEventListener("TrunkApplicationFailed", onFailed, { once: true });

  // Fallback: Bevy can be alive (hud exporting) even if the CustomEvent was missed.
  const poll = () => {
    if (wasmReady) return;
    const api = wasmApi();
    if (api?.bifrost_hud && api?.bifrost_start_bot) {
      try {
        const raw = api.bifrost_hud();
        // Default HUD JSON is non-empty once the wasm module is live.
        if (typeof raw === "string" && raw.length > 2) {
          onWasmReady();
          return;
        }
      } catch (_) {
        /* not ready yet */
      }
    }
    window.__bifrostWasmPoll = setTimeout(poll, 250);
  };
  clearTimeout(window.__bifrostWasmPoll);
  window.__bifrostWasmPoll = setTimeout(poll, 400);

  clearTimeout(window.__bifrostWasmTimeout);
  window.__bifrostWasmTimeout = setTimeout(() => {
    if (wasmReady || wasmLoadWarned) return;
    wasmLoadWarned = true;
    setStatus("Game still loading… WASM is taking a long time. Check the console if this never finishes.");
  }, 20000);
}

/** Mark Ready UI + start (or queue) a bot match. */
function requestBotStart() {
  setPlayerName($("embed-player-name")?.value || getPlayerName());
  const preP0 = $("pre-p0");
  if (preP0) {
    preP0.classList.add("is-ready");
    preP0.textContent = `${getPlayerName()} Ready`;
  }
  if (!preMatchOpen && isEmbedded()) {
    showPreMatch();
    if (preP0) {
      preP0.classList.add("is-ready");
      preP0.textContent = `${getPlayerName()} Ready`;
    }
  }
  return startBotMatch();
}

function beginBotMatchNow() {
  if (sessionState.ticket || pendingOnlineConnect) {
    setStatus("Online lobby is active — Quit first, or wait for your opponent.");
    return false;
  }
  window.__bifrostPaused = false;
  try {
    window.parent?.postMessage({ type: "bifrost-game-start" }, "*");
  } catch (_) {}
  userQuit = false;
  matchLatched = false;
  rematchP0 = false;
  suppressResults = true;
  opponentName = "CPU";
  lobbyPhase = "idle";
  lobbyRole = null;
  setPlayMode("bot");
  hideLobbyWait();
  clearTimeout(window.__rematchTimer);
  window.__rematchTimer = 0;
  clearTimeout(window.__bifrostRoundBanner);
  window.__bifrostRoundBanner = 0;
  $("results")?.classList.add("hidden");
  $("results")?.classList.remove("round-only");
  sessionState.room = null;
  sessionState.ticket = null;
  clearRoomFields();
  try {
    wasmApi().bifrost_start_bot();
  } catch (e) {
    console.error("[bifrost] bifrost_start_bot failed", e);
    setStatus(`Could not start bot match: ${e.message}`);
    window.__bifrostPaused = true;
    suppressResults = false;
    if (isEmbedded()) showPreMatch();
    return false;
  }
  hideLobby();
  showArenaHud();
  collapseEmbedBarForMatch();
  setStatus("Bot match — WASD / arrows / mouse / gamepad · X/RT spin");
  focusPlaySurface();
  return true;
}

function startBotMatch() {
  if (sessionState.ticket || pendingOnlineConnect) {
    setStatus("Online lobby is active — Quit first before starting a bot match.");
    return false;
  }
  if (!wasmReady) {
    pendingBotStart = true;
    setStatus("Loading game… starting when ready");
    return false;
  }
  pendingBotStart = false;
  setStatus("Get ready…");
  runCountdown(() => beginBotMatchNow());
  return true;
}

async function fetchTurnCredentials() {
  try {
    const res = await fetch("/api/turn");
    if (!res.ok) {
      console.warn("[bifrost] TURN unavailable", res.status);
      return null;
    }
    return await res.json();
  } catch (e) {
    console.warn("[bifrost] TURN fetch failed", e);
    return null;
  }
}

async function connectOnline(room, ticket, role = "host") {
  // Cancel any queued bot start — joining online must win.
  pendingBotStart = false;
  if (!wasmReady) {
    pendingOnlineConnect = { room, ticket, role };
    sessionState.room = room;
    sessionState.ticket = ticket;
    lobbyRole = role;
    persistOnlineSession();
    setPlayMode(role === "guest" ? "join" : "create");
    syncRoomField(room);
    showLobbyWait(role, room);
    setStatus(
      role === "guest"
        ? `Joined ${room} — loading game, then connecting…`
        : `Room ${room} — loading game, then waiting…`
    );
    updateLaunchEnabled();
    return true;
  }
  pendingOnlineConnect = null;
  userQuit = false;
  matchLatched = false;
  rematchP0 = false;
  hidePreMatch();
  // Stay paused until Matchbox reaches an online match (not bot InGame).
  window.__bifrostPaused = true;
  clearTimeout(window.__rematchTimer);
  window.__bifrostRoundBanner = 0;
  const results = $("results");
  if (results) results.classList.add("hidden");
  sessionState.room = room;
  sessionState.ticket = ticket;
  lobbyRole = role;
  persistOnlineSession();
  setPlayMode(role === "guest" ? "join" : "create");
  if (roomInput) roomInput.value = room;
  const embedRoom = $("embed-room-code");
  if (embedRoom) embedRoom.value = room;
  syncPlayModeUi();
  const turn = await fetchTurnCredentials();
  const turnUrls = Array.isArray(turn?.urls) ? turn.urls : [];
  const turnUser = turn?.username || "";
  const turnCred = turn?.credential || "";
  if (turnUrls.length) {
    setStatus(`Room ${room} — negotiating relay…`);
  }
  try {
    wasmApi().bifrost_connect(room, ticket, turnUrls, turnUser, turnCred, role === "host");
  } catch (e) {
    console.error("[bifrost] bifrost_connect failed", e);
    setStatus(`Could not connect: ${e.message}`);
    showLobby();
    updateLaunchEnabled();
    return false;
  }
  hideLobby();
  showArenaHud();
  showLobbyWait(role, room);
  setStatus(
    role === "guest"
      ? `Joined ${room} — connected…`
      : `Room ${room} — waiting for opponent…`
  );
  updateLaunchEnabled();
  return true;
}

async function api(path, body) {
  const res = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

async function joinRoomWithCode(rawCode) {
  const code = String(rawCode || "").trim().toUpperCase();
  if (!code) {
    setStatus("Enter a room code, then Join.");
    $("embed-room-code")?.focus();
    return false;
  }
  if (sessionState.room === code && sessionState.ticket && lobbyRole === "host") {
    setStatus("You already host this room — wait for a friend to Join.");
    return false;
  }
  setPlayMode("join");
  setStatus("Joining room…");
  try {
    const data = await api("/api/rooms/join", {
      protocol_version: 1,
      room_code: code,
      display_name: getPlayerName(),
    });
    syncRoomField(code);
    if (data.host_name) opponentName = String(data.host_name).slice(0, 7) || "P2";
    await connectOnline(code, data.guest_ticket, "guest");
    return true;
  } catch (err) {
    setStatus(`Join failed: ${err.message}`);
    return false;
  }
}

function bindEmbedUi() {
  const embedBot = $("btn-embed-bot");
  const embedCreate = $("btn-embed-create");
  const embedJoin = $("btn-embed-join");
  const embedLaunch = $("btn-embed-launch");
  const embedCopy = $("btn-embed-copy");
  const embedRoom = $("embed-room-code");
  const embedInsp = $("btn-embed-inspector");
  const embedName = $("embed-player-name");
  const preReady = $("btn-pre-ready");
  const preQuit = $("btn-pre-quit");

  if (embedName) {
    embedName.addEventListener("input", () => setPlayerName(embedName.value));
    embedName.addEventListener("change", () => setPlayerName(embedName.value));
    embedName.addEventListener("click", (e) => e.stopPropagation());
    embedName.addEventListener("keydown", (e) => e.stopPropagation());
  }
  if (preReady) {
    preReady.addEventListener("click", (e) => {
      e.stopPropagation();
      startFromPreMatchConfirm();
    });
  }
  if (preQuit) {
    preQuit.addEventListener("click", (e) => {
      e.stopPropagation();
      void quitToMenu();
    });
  }
  if (embedBot) {
    embedBot.addEventListener("click", (e) => {
      e.stopPropagation();
      if (matchLatched) {
        voteRematch();
        return;
      }
      if (sessionState.ticket) {
        setStatus("Quit the online lobby before starting a bot match.");
        return;
      }
      setPlayMode("bot");
      requestBotStart();
    });
  }
  if (embedCopy) {
    embedCopy.addEventListener("click", async (e) => {
      e.stopPropagation();
      const pasteMode = embedCopy.dataset.mode === "paste" || playMode === "join";
      if (pasteMode) {
        try {
          const text = (await navigator.clipboard.readText()).trim().toUpperCase().slice(0, 8);
          if (!text) {
            setStatus("Clipboard empty — paste a room code.");
            return;
          }
          if (embedRoom) embedRoom.value = text;
          if (roomInput) roomInput.value = text;
          setPlayMode("join");
          setStatus(`Pasted ${text} — press Join or Launch`);
          embedCopy.classList.add("copied");
          clearTimeout(window.__bifrostCopyFlash);
          window.__bifrostCopyFlash = setTimeout(() => {
            embedCopy.classList.remove("copied");
          }, 1600);
        } catch (err) {
          setStatus("Could not paste — click the field and paste manually (Ctrl+V).");
          embedRoom?.focus();
        }
        return;
      }
      const code = (embedRoom?.value || roomInput?.value || sessionState.room || "").trim();
      if (!code) {
        setStatus("No room code to copy — Create or Join first.");
        return;
      }
      try {
        await navigator.clipboard.writeText(code);
        flashCopied(code, { button: embedCopy });
      } catch (err) {
        setStatus("Could not copy — select the code and copy manually.");
      }
    });
  }
  if (embedCreate) {
    embedCreate.addEventListener("click", async (e) => {
      e.stopPropagation();
      setPlayMode("create");
      setStatus("Creating private room…");
      try {
        const data = await api("/api/rooms", {
          protocol_version: 1,
          display_name: getPlayerName(),
        });
        syncRoomField(data.room_code);
        await connectOnline(data.room_code, data.host_ticket, "host");
      } catch (err) {
        setStatus(`Could not create room: ${err.message}`);
      }
    });
  }
  if (embedJoin) {
    embedJoin.addEventListener("click", async (e) => {
      e.stopPropagation();
      setPlayMode("join");
      const code = (embedRoom?.value || roomInput?.value || "").trim().toUpperCase();
      await joinRoomWithCode(code);
    });
  }
  if (embedLaunch) {
    embedLaunch.addEventListener("click", async (e) => {
      e.stopPropagation();
      if (embedLaunch.disabled) return;
      const ticket = sessionState.ticket;
      if (ticket && (lobbyPhase === "ready" || lobbyPhase === "match")) {
        setStatus("Lobby ready — starting…");
        return;
      }
      if (ticket && lobbyWaitOpen) {
        setStatus("Waiting for opponent… share the room code.");
        return;
      }
      const code = (embedRoom?.value || roomInput?.value || "").trim().toUpperCase();
      // Code in field + not hosting → Join (Launch-as-join). Empty → bot.
      if (code && !(ticket && lobbyRole === "host")) {
        await joinRoomWithCode(code);
        return;
      }
      if (playMode === "join" && !code) {
        setStatus("Enter a room code, then Launch.");
        embedRoom?.focus();
        return;
      }
      clearRoomFields();
      requestBotStart();
    });
  }
  if (embedRoom) {
    embedRoom.addEventListener("input", () => {
      if (playMode !== "create" && playMode !== "bot") setPlayMode("join");
      updateLaunchEnabled();
      syncPlayModeUi();
    });
    embedRoom.addEventListener("click", (e) => e.stopPropagation());
    // Keep Backspace/arrows in the field — do not bubble to quit/nav handlers.
    embedRoom.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.code === "Enter") {
        e.preventDefault();
        const code = String(embedRoom.value || "").trim().toUpperCase();
        void joinRoomWithCode(code);
      }
    });
  }
  if (roomInput) {
    roomInput.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.code === "Enter") {
        e.preventDefault();
        const code = String(roomInput.value || "").trim().toUpperCase();
        void joinRoomWithCode(code);
      }
    });
  }
  const lobbyCopy = $("btn-lobby-copy");
  const lobbyLeave = $("btn-lobby-leave") || $("btn-lobby-quit");
  if (lobbyCopy) {
    lobbyCopy.addEventListener("click", async (e) => {
      e.stopPropagation();
      const code = sessionState.room || "";
      if (!code) return;
      try {
        await navigator.clipboard.writeText(code);
        flashCopied(code, { button: lobbyCopy });
      } catch (_) {
        setStatus("Could not copy — select the code manually.");
      }
    });
  }
  if (lobbyLeave) {
    lobbyLeave.addEventListener("click", (e) => {
      e.stopPropagation();
      void returnToMatchMenu();
    });
  }
  if (embedInsp) {
    embedInsp.addEventListener("click", (e) => {
      e.stopPropagation();
      const insp = $("inspector");
      const lobbyBtn = $("btn-inspector");
      const on = embedInsp.getAttribute("aria-pressed") !== "true";
      embedInsp.setAttribute("aria-pressed", on ? "true" : "false");
      if (lobbyBtn) lobbyBtn.setAttribute("aria-pressed", on ? "true" : "false");
      if (insp) insp.classList.toggle("hidden", !on);
      setStatus(on ? "Inspector open" : "Inspector closed");
    });
  }
  const embedControls = $("btn-embed-controls");
  const controlsPanel = $("controls-panel");
  const controlsClose = $("btn-controls-close");
  const invertAngle = $("opt-invert-angle");
  const CONTROLS_KEY = "bifrost-invert-angle";
  try {
    window.__bifrostInvertAngle = sessionStorage.getItem(CONTROLS_KEY) === "1";
    if (invertAngle) invertAngle.checked = !!window.__bifrostInvertAngle;
  } catch (_) {
    window.__bifrostInvertAngle = false;
  }
  const toggleControls = (open) => {
    if (!controlsPanel) return;
    controlsPanel.classList.toggle("hidden", !open);
  };
  if (embedControls) {
    embedControls.addEventListener("click", (e) => {
      e.stopPropagation();
      const open = controlsPanel?.classList.contains("hidden");
      toggleControls(!!open);
    });
  }
  if (controlsClose) {
    controlsClose.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleControls(false);
    });
  }
  if (invertAngle) {
    invertAngle.addEventListener("change", () => {
      window.__bifrostInvertAngle = !!invertAngle.checked;
      try {
        sessionStorage.setItem(CONTROLS_KEY, window.__bifrostInvertAngle ? "1" : "0");
      } catch (_) {}
      setStatus(window.__bifrostInvertAngle ? "Paddle angle inverted" : "Paddle angle normal");
    });
  }
}

function bindUi() {
  bindEmbedUi();

  panel = $("panel");
  status = $("status");
  roomInput = $("room-code");
  inspector = $("inspector");

  const lobbyRequired = [
    ["panel", panel],
    ["status", status],
    ["room-code", roomInput],
    ["btn-bot", $("btn-bot")],
    ["btn-create", $("btn-create")],
    ["btn-join", $("btn-join")],
    ["btn-launch", $("btn-launch")],
    ["btn-inspector", $("btn-inspector")],
  ];
  let lobbyOk = true;
  for (const [id, el] of lobbyRequired) {
    if (!el) {
      console.warn(`[bifrost shell] missing #${id} (lobby controls skipped)`);
      lobbyOk = false;
    }
  }

  if (lobbyOk) {
  $("btn-bot").addEventListener("click", () => {
    if (sessionState.ticket) {
      setStatus("Quit the online lobby before starting a bot match.");
      return;
    }
    setPlayMode("bot");
    startBotMatch();
  });

  $("btn-create").addEventListener("click", async () => {
    showLobby();
    setPlayMode("create");
    setStatus("Creating private room…");
    try {
      const data = await api("/api/rooms", {
        protocol_version: 1,
        display_name: getPlayerName(),
      });
      await connectOnline(data.room_code, data.host_ticket, "host");
    } catch (e) {
      showLobby();
      setStatus(`Could not create room: ${e.message}`);
    }
  });

  $("btn-join").addEventListener("click", async () => {
    showLobby();
    setPlayMode("join");
    const code = roomInput.value.trim().toUpperCase();
    await joinRoomWithCode(code);
  });

  $("btn-copy").addEventListener("click", async () => {
    const code = roomInput.value.trim();
    if (!code) return;
    const link = `${window.location.origin}${window.location.pathname}?room=${code}`;
    try {
      await navigator.clipboard.writeText(link);
      flashCopied(code, {
        button: $("btn-copy"),
        status: "Invite link copied (room code only — tickets stay in-memory).",
      });
    } catch (e) {
      setStatus("Could not copy — grant clipboard permission or copy the room code manually.");
    }
  });

  $("btn-launch").addEventListener("click", async () => {
    const launch = $("btn-launch");
    if (launch?.disabled) return;
    const ticket = sessionState.ticket;
    if (ticket && (lobbyPhase === "ready" || lobbyPhase === "match")) {
      setStatus("Lobby ready — starting…");
      return;
    }
    if (ticket && lobbyWaitOpen) {
      setStatus("Waiting for opponent… share the room code.");
      return;
    }
    const code = (roomInput?.value || "").trim().toUpperCase();
    if (code && !(ticket && lobbyRole === "host")) {
      await joinRoomWithCode(code);
      return;
    }
    clearRoomFields();
    startBotMatch();
  });

  roomInput.addEventListener("input", () => updateLaunchEnabled());

  $("btn-inspector").addEventListener("click", (e) => {
    const on = e.currentTarget.getAttribute("aria-pressed") !== "true";
    e.currentTarget.setAttribute("aria-pressed", on ? "true" : "false");
    inspector.classList.toggle("hidden", !on);
    const embedInsp = $("btn-embed-inspector");
    if (embedInsp) embedInsp.setAttribute("aria-pressed", on ? "true" : "false");
  });
  }

  const lagForge = $("lag-forge");
  if (lagForge) {
    lagForge.addEventListener("change", (e) => {
      const frames = e.target.value;
      const url = new URL(window.location.href);
      if (frames === "0") url.searchParams.delete("lag_frames");
      else url.searchParams.set("lag_frames", frames);
      window.history.replaceState({}, "", url);
    });
  }

  const canvas = $("bevy-canvas");
  if (canvas) {
    canvas.addEventListener("pointerdown", () => focusPlaySurface());
  }

  window.addEventListener("keydown", (e) => {
    // Typing a room code / name must never quit or navigate.
    if (typingInFormField(e.target)) return;
    if (readyUpOpen() && (e.code === "Space" || e.code === "Enter")) {
      e.preventDefault();
      pulseReadyJump();
      return;
    }
    if (preMatchOpen || matchLatched) {
      if (e.code === "ArrowLeft" || e.code === "ArrowRight" || e.code === "ArrowUp" || e.code === "ArrowDown") {
        e.preventDefault();
        cycleUiFocus(e.code === "ArrowLeft" || e.code === "ArrowUp" ? -1 : 1);
        return;
      }
      if (e.code === "Space" || e.code === "Enter") {
        e.preventDefault();
        confirmUiFocus();
        return;
      }
      if (e.code === "Escape" || e.code === "KeyB" || e.code === "Backspace") {
        e.preventDefault();
        quitUiFocus();
        return;
      }
    }
  });

  setStatus("Loading game…");
  whenWasmReady();
  updateLaunchEnabled();
  // Show Ready immediately in Lab embed — don't wait for ~16MB WASM.
  if (isEmbedded()) {
    showPreMatch();
    setStatus("Loading game… Ready when WASM finishes, or press Ready/Bot/Launch to queue start.");
  }
}

function focusPlaySurface() {
  const canvas = $("bevy-canvas");
  if (canvas && typeof canvas.focus === "function") {
    try {
      canvas.focus({ preventScroll: true });
    } catch (_) {
      canvas.focus();
    }
  }
  try {
    window.focus();
  } catch (_) {}
  bifrostUnlockAudio();
}

window.__bifrostPad = {
  lx: 0,
  ly: 0,
  rx: 0,
  ry: 0,
  south: false,
  west: false,
  east: false,
  spin: false,
  grapple: false,
};
window.__bifrostKeys = {};
window.__bifrostKeyJump = false;
if (typeof window.__bifrostPaused !== "boolean") {
  window.__bifrostPaused = false;
}

function readyUpOpen() {
  return !$("ready-up")?.classList.contains("hidden");
}

function pulseReadyJump() {
  window.__bifrostReadyWanted = true;
  window.__bifrostKeyJump = true;
  window.__bifrostKeys = window.__bifrostKeys || {};
  window.__bifrostKeys.Space = true;
  clearTimeout(window.__bifrostReadyPulse);
  // Hold JUMP until syncReady clears __bifrostReadyWanted (local seat latched).
  window.__bifrostReadyPulse = setTimeout(() => {
    if (window.__bifrostReadyWanted) {
      window.__bifrostKeyJump = true;
      window.__bifrostKeys.Space = true;
      pulseReadyJump();
      return;
    }
    window.__bifrostKeyJump = false;
    if (window.__bifrostKeys) delete window.__bifrostKeys.Space;
  }, 80);
}

function applyEmbedKey(code, down) {
  if (!code) return;
  window.__bifrostKeys = window.__bifrostKeys || {};
  if (down) {
    window.__bifrostKeys[code] = true;
    if (code === "Space") window.__bifrostKeyJump = true;
  } else {
    delete window.__bifrostKeys[code];
    if (code === "Space") window.__bifrostKeyJump = false;
  }
}

window.addEventListener("message", (ev) => {
  if (ev?.data?.type === "bifrost-focus") {
    focusPlaySurface();
  }
  if (ev?.data?.type === "bifrost-unlock-audio") {
    bifrostUnlockAudio();
  }
  if (ev?.data?.type === "bifrost-key") {
    const code = String(ev.data.code || "");
    const down = !!ev.data.down;
    // Parent forwards gameplay keys only; still ignore when a Bifrost field is focused.
    if (typingInFormField(document.activeElement)) return;
    if (down && readyUpOpen() && (code === "Space" || code === "Enter")) {
      pulseReadyJump();
      applyEmbedKey(code, down);
      return;
    }
    if (down && (preMatchOpen || matchLatched)) {
      if (code === "Enter" || code === "Space") {
        confirmUiFocus();
        return;
      }
      if (code === "ArrowLeft" || code === "ArrowRight" || code === "ArrowUp" || code === "ArrowDown") {
        cycleUiFocus(1);
        return;
      }
      if (code === "Escape" || code === "KeyB" || code === "Backspace") {
        quitUiFocus();
        return;
      }
    }
    applyEmbedKey(code, down);
  }
  if (ev?.data?.type === "bifrost-pad") {
    const d = ev.data;
    const now = performance.now();
    const lx = Number(d.lx) || 0;
    const ly = Number(d.ly) || 0;
    // Pad: X/west = spin · Y = grapple · B/east = menu cancel only · LT = grapple.
    const spinHeld = !!(d.west || d.spin || d.x);
    const grappleHeld = !!(d.grapple || d.y || d.north);
    const menuCancel = !!(d.east || d.b);
    if (readyUpOpen()) {
      if (d.south && !lastPadConfirm) {
        pulseReadyJump();
      }
      if (menuCancel && !lastPadEast) {
        returnToMatchMenu();
      }
      lastPadConfirm = !!d.south;
      lastPadEast = menuCancel;
      window.__bifrostPad = {
        lx,
        ly,
        rx: Number(d.rx) || 0,
        ry: Number(d.ry) || 0,
        south: !!d.south,
        west: spinHeld,
        east: false,
        spin: spinHeld,
        grapple: grappleHeld,
      };
      return;
    }
    if (preMatchOpen || matchLatched) {
      if (Math.abs(lx) > 0.55 || Math.abs(ly) > 0.55) {
        if (now - lastPadNavAt > 220) {
          lastPadNavAt = now;
          cycleUiFocus(lx < -0.55 || ly < -0.55 ? -1 : 1);
        }
      }
      if (d.south && !lastPadConfirm) {
        confirmUiFocus();
      }
      if (menuCancel && !lastPadEast) {
        quitUiFocus();
      }
      lastPadConfirm = !!d.south;
      lastPadEast = menuCancel;
      return;
    }
    lastPadConfirm = !!d.south;
    lastPadEast = menuCancel;
    window.__bifrostPad = {
      lx,
      ly,
      rx: Number(d.rx) || 0,
      ry: Number(d.ry) || 0,
      south: !!d.south,
      west: spinHeld,
      east: false,
      spin: spinHeld,
      grapple: grappleHeld,
    };
  }
});
window.addEventListener("focus", () => focusPlaySurface());
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) focusPlaySurface();
});

function hydrateFromQuery() {
  const params = new URLSearchParams(window.location.search);
  const room = params.get("room");
  if (room) {
    showLobby();
    if (roomInput) roomInput.value = room;
    sessionState.room = room;
    sessionState.ticket = null;
    setStatus(`Room ${room} — join to get a guest ticket, then Launch.`);
    return true;
  }
  hideLobby();
  setStatus("Bot match — WASD / arrows / mouse / gamepad · R restart");
  return false;
}

/** Unlock + synthesize pitched hits (no asset file required). */
let bifrostAudioCtx = null;
function bifrostUnlockAudio() {
  try {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    if (!bifrostAudioCtx) bifrostAudioCtx = new AC();
    if (bifrostAudioCtx.state === "suspended") {
      void bifrostAudioCtx.resume();
    }
    return bifrostAudioCtx;
  } catch (_) {
    return null;
  }
}
// Capture-phase so Bevy's preventDefault doesn't block unlock; keep retrying until running.
["pointerdown", "keydown", "touchstart", "mousedown", "gamepadconnected"].forEach((ev) => {
  window.addEventListener(ev, () => bifrostUnlockAudio(), { capture: true, passive: true });
  document.addEventListener(ev, () => bifrostUnlockAudio(), { capture: true, passive: true });
});

function bifrostSynthTone(ctx, kind, volume) {
  const t0 = ctx.currentTime;
  const vol = Math.max(0.08, Math.min(0.9, (Number(volume) || 0.5) * 1.35));

  // Spin: sweeping sword whoosh (noise + falling tone).
  if (kind === "spin") {
    const dur = 0.38;
    const bufferSize = Math.floor(ctx.sampleRate * dur);
    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      const env = 1 - i / bufferSize;
      data[i] = (Math.random() * 2 - 1) * env * env;
    }
    const noise = ctx.createBufferSource();
    noise.buffer = buffer;
    const filter = ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.setValueAtTime(1800, t0);
    filter.frequency.exponentialRampToValueAtTime(280, t0 + dur);
    filter.Q.value = 1.2;
    const ng = ctx.createGain();
    ng.gain.setValueAtTime(vol * 0.55, t0);
    ng.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    noise.connect(filter);
    filter.connect(ng);
    ng.connect(ctx.destination);
    noise.start(t0);
    noise.stop(t0 + dur);

    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sawtooth";
    osc.frequency.setValueAtTime(520, t0);
    osc.frequency.exponentialRampToValueAtTime(90, t0 + dur);
    gain.gain.setValueAtTime(vol * 0.28, t0);
    gain.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + dur + 0.02);
    return;
  }

  // Grapple: zip + crackle.
  if (kind === "grapple") {
    const dur = 0.28;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sawtooth";
    osc.frequency.setValueAtTime(980, t0);
    osc.frequency.exponentialRampToValueAtTime(160, t0 + dur);
    gain.gain.setValueAtTime(vol * 0.32, t0);
    gain.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + dur + 0.02);

    const bufferSize = Math.floor(ctx.sampleRate * 0.18);
    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      const env = 1 - i / bufferSize;
      data[i] = (Math.random() * 2 - 1) * env * env * 0.7;
    }
    const noise = ctx.createBufferSource();
    noise.buffer = buffer;
    const filter = ctx.createBiquadFilter();
    filter.type = "highpass";
    filter.frequency.value = 1400;
    const ng = ctx.createGain();
    ng.gain.setValueAtTime(vol * 0.4, t0 + 0.04);
    ng.gain.exponentialRampToValueAtTime(0.001, t0 + 0.22);
    noise.connect(filter);
    filter.connect(ng);
    ng.connect(ctx.destination);
    noise.start(t0 + 0.04);
    noise.stop(t0 + 0.24);
    return;
  }

  // Corner trampoline: bright ping + springy thump.
  if (kind === "corner") {
    const dur = 0.22;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "triangle";
    osc.frequency.setValueAtTime(740, t0);
    osc.frequency.exponentialRampToValueAtTime(180, t0 + dur);
    gain.gain.setValueAtTime(vol * 0.55, t0);
    gain.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + dur + 0.02);

    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.type = "sine";
    osc2.frequency.setValueAtTime(1480, t0);
    osc2.frequency.exponentialRampToValueAtTime(420, t0 + 0.12);
    gain2.gain.setValueAtTime(vol * 0.28, t0);
    gain2.gain.exponentialRampToValueAtTime(0.001, t0 + 0.14);
    osc2.connect(gain2);
    gain2.connect(ctx.destination);
    osc2.start(t0);
    osc2.stop(t0 + 0.16);
    return;
  }

  const rates = {
    break: 0.55,
    chip: 1.35,
    tick: 1.8,
    wild: 0.9,
    knock: 0.7,
    neutral: 1.15,
    corner: 1.55,
    paddle: 1.05,
    goal: 0.45,
    win: 0.35,
    pound: 0.28,
    burst: 0.95,
  };
  const base = 220 * (rates[kind] || 1.0);
  const dur = kind === "win" || kind === "goal" || kind === "pound" ? 0.32 : 0.1;
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = kind === "break" || kind === "wild" ? "square" : "triangle";
  osc.frequency.setValueAtTime(base, t0);
  osc.frequency.linearRampToValueAtTime(Math.max(40, base * 0.45), t0 + dur);
  gain.gain.setValueAtTime(vol, t0);
  gain.gain.linearRampToValueAtTime(0.0001, t0 + dur);
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start(t0);
  osc.stop(t0 + dur + 0.02);
}

window.bifrostPlaySfx = function bifrostPlaySfx(kind, volume) {
  try {
    const ctx = bifrostUnlockAudio();
    if (!ctx) return;
    const go = () => {
      try {
        bifrostSynthTone(ctx, kind, volume);
      } catch (_) {}
    };
    if (ctx.state === "suspended") {
      ctx.resume().then(go).catch(() => {});
    } else {
      go();
    }
  } catch (_) {
    /* autoplay may still be blocked */
  }
};

/** Phone vibrate + gamepad rumble when available. */
window.bifrostHaptic = function bifrostHaptic(kind) {
  try {
    const patterns = {
      paddle: [14],
      shove: [10, 24, 16],
      knock: [10, 24, 16],
      break: [28, 35, 45],
      chip: [8],
      wild: [18, 22],
      goal: [40, 50, 70],
      win: [30, 40, 30, 40, 80],
      pound: [25, 30, 55],
      spin: [12, 18],
      grapple: [10, 16, 22],
      corner: [18, 22, 28],
    };
    const pulse = patterns[kind] || [12];
    if (typeof navigator !== "undefined" && typeof navigator.vibrate === "function") {
      navigator.vibrate(pulse);
    }
    const pads = navigator.getGamepads?.() || [];
    const duration = Array.isArray(pulse) ? pulse.reduce((a, b) => a + b, 0) : 20;
    const strong =
      kind === "break" || kind === "goal" || kind === "win" ? 0.85 : kind === "shove" || kind === "knock" ? 0.55 : 0.35;
    for (const pad of pads) {
      const actuator = pad?.vibrationActuator;
      if (actuator?.playEffect) {
        void actuator.playEffect("dual-rumble", {
          startDelay: 0,
          duration: Math.min(120, duration),
          weakMagnitude: strong * 0.55,
          strongMagnitude: strong,
        });
      }
    }
  } catch (_) {
    /* haptics unsupported / blocked */
  }
};

function markEmbedded() {
  document.documentElement.classList.add("embedded");
  if (document.body) document.body.classList.add("embedded");
}

function detectEmbedded() {
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.get("embed") === "1" || params.get("embedded") === "1") {
      markEmbedded();
      return;
    }
  } catch (_) {}
  try {
    if (window.self !== window.top) {
      markEmbedded();
    }
  } catch (_) {
    // Cross-origin frame access can throw — treat as embedded.
    markEmbedded();
  }
}

detectEmbedded();
document.addEventListener("DOMContentLoaded", () => {
  detectEmbedded();
  focusPlaySurface();
  bindButtonPressFeedback();
  bindTouchPad();
  bindHowtoCollapse();
  bindEmbedCollapse();
  void reclaimStaleOnlineSession();
});

window.addEventListener("pagehide", leaveOnlineSessionOnUnload);
window.addEventListener("beforeunload", leaveOnlineSessionOnUnload);

bindUi();

/** On-screen dual stick + faces — phones / coarse mobile only (never desktop). */
function wantsTouchPad() {
  const ua = navigator.userAgent || "";
  const mobileUa = /Android|iPhone|iPad|iPod|Mobile|webOS|BlackBerry|IEMobile|Opera Mini/i.test(
    ua
  );
  const coarsePhone =
    window.matchMedia("(pointer: coarse)").matches &&
    window.matchMedia("(hover: none)").matches;
  const fineDesktop =
    window.matchMedia("(hover: hover)").matches &&
    window.matchMedia("(pointer: fine)").matches;
  if (fineDesktop && !mobileUa) return false;
  return mobileUa || coarsePhone;
}

function bindTouchPad() {
  const root = $("touch-pad");
  const moveZone = $("touch-bisect-move") || $("touch-pad-move");
  const knob = $("touch-pad-knob");
  const aimZone = $("touch-bisect-aim") || $("touch-pad-aim");
  const aimKnob = $("touch-pad-aim-knob");
  const jumpBtn = $("touch-jump");
  const spinBtn = $("touch-spin");
  const grappleBtn = $("touch-grapple");
  if (!root || !moveZone || !knob) return;

  const showPad = () => {
    if (!wantsTouchPad()) {
      root.hidden = true;
      root.setAttribute("aria-hidden", "true");
      return false;
    }
    root.hidden = false;
    root.setAttribute("aria-hidden", "false");
    return true;
  };
  if (!showPad()) return;
  window.matchMedia("(pointer: fine)").addEventListener?.("change", showPad);
  window.matchMedia("(hover: hover)").addEventListener?.("change", showPad);
  window.matchMedia("(max-width: 900px)").addEventListener?.("change", showPad);

  const applySeatColors = () => {
    // P1 cyan / P2 orange — sticks + faces share the seat color.
    const seat = lobbyRole === "guest" ? 1 : 0;
    const accent = seat === 0 ? "#33d9f2" : "#f28c40";
    root.style.setProperty("--pad-accent", accent);
    root.style.setProperty("--pad-accent-2", accent);
  };
  applySeatColors();
  window.__bifrostApplyPadSeatColors = applySeatColors;

  const pad = () => {
    window.__bifrostPad = window.__bifrostPad || {
      lx: 0,
      ly: 0,
      rx: 0,
      ry: 0,
      south: false,
      west: false,
      east: false,
      spin: false,
      grapple: false,
    };
    return window.__bifrostPad;
  };

  const bindStick = (zone, knobEl, writeAxes, visualBase, deadzone) => {
    if (!zone || !knobEl) return;
    let activeId = null;
    let originX = 0;
    let originY = 0;
    const GATE = 52; // device px — short full-deflection travel (gacha-style)
    const DEADZONE = deadzone;
    const CURVE = 1.25;
    const homeLeft = visualBase ? visualBase.style.left : "";
    const homeTop = visualBase ? visualBase.style.top : "";
    const homeTransform = visualBase ? visualBase.style.transform : "";

    const setKnob = (nx, ny) => {
      knobEl.style.transform = `translate(${nx * GATE}px, ${ny * GATE}px)`;
    };
    const resetVisualBase = () => {
      if (!visualBase) return;
      visualBase.classList.remove("is-active");
      visualBase.style.left = homeLeft;
      visualBase.style.top = homeTop;
      visualBase.style.transform = homeTransform || "";
      visualBase.style.position = "";
    };
    const reset = () => {
      activeId = null;
      writeAxes(0, 0);
      setKnob(0, 0);
      resetVisualBase();
    };
    const onMove = (clientX, clientY) => {
      let dx = (clientX - originX) / GATE;
      let dy = (clientY - originY) / GATE;
      let mag = Math.hypot(dx, dy);
      if (mag > 1) {
        dx /= mag;
        dy /= mag;
        mag = 1;
      }
      // Deadzone + soft power curve for fine center / snappy rim.
      let outX = 0;
      let outY = 0;
      if (mag > DEADZONE) {
        const t = (mag - DEADZONE) / (1 - DEADZONE);
        const shaped = Math.pow(t, CURVE);
        const inv = mag > 0 ? shaped / mag : 0;
        outX = dx * inv;
        outY = dy * inv;
      }
      writeAxes(outX, outY);
      setKnob(dx, dy);
    };
    zone.addEventListener(
      "pointerdown",
      (e) => {
        if (activeId != null) return;
        activeId = e.pointerId;
        originX = e.clientX;
        originY = e.clientY;
        // Floating stick: park visual base under finger (clamped to pad).
        if (visualBase && root) {
          const padRect = root.getBoundingClientRect();
          const half = (visualBase.clientWidth || 84) / 2;
          const lx = Math.min(
            padRect.width - half - 8,
            Math.max(half + 8, e.clientX - padRect.left)
          );
          const ly = Math.min(
            padRect.height - half - 8,
            Math.max(half + 8, e.clientY - padRect.top)
          );
          visualBase.style.position = "fixed";
          visualBase.style.left = `${padRect.left + lx - half}px`;
          visualBase.style.top = `${padRect.top + ly - half}px`;
          visualBase.style.transform = "none";
          visualBase.style.zIndex = "6";
          visualBase.classList.add("is-active");
          originX = padRect.left + lx;
          originY = padRect.top + ly;
        }
        try {
          zone.setPointerCapture(e.pointerId);
        } catch (_) {}
        focusPlaySurface();
        onMove(e.clientX, e.clientY);
        e.preventDefault();
      },
      { passive: false }
    );
    zone.addEventListener(
      "pointermove",
      (e) => {
        if (e.pointerId !== activeId) return;
        onMove(e.clientX, e.clientY);
        e.preventDefault();
      },
      { passive: false }
    );
    const endMove = (e) => {
      if (e.pointerId !== activeId) return;
      reset();
    };
    zone.addEventListener("pointerup", endMove);
    zone.addEventListener("pointercancel", endMove);
    zone.addEventListener("lostpointercapture", () => {
      if (activeId != null) reset();
    });
  };

  // L/R stick Y: finger-up → negative → INPUT_UP (match Gamepad / R-stick).
  // R-stick deadzone +10% vs move stick (0.14 → 0.154).
  const moveBase = $("touch-pad-move");
  const aimBase = $("touch-pad-aim");
  bindStick(
    moveZone,
    knob,
    (dx, dy) => {
      const p = pad();
      p.lx = dx;
      p.ly = dy;
    },
    moveBase,
    0.14
  );
  bindStick(
    aimZone,
    aimKnob,
    (dx, dy) => {
      const p = pad();
      p.rx = dx;
      p.ry = dy;
    },
    aimBase,
    0.14 * 1.1
  );

  const holdButton = (btn, flag) => {
    if (!btn) return;
    const set = (on) => {
      const p = pad();
      p[flag] = on;
      // Protocol: west = spin (pad X); grapple flag = Grappleshot (pad Y / touch).
      if (flag === "west") p.spin = on;
      if (flag === "grapple") p.grapple = on;
      if (flag === "south") {
        window.__bifrostKeyJump = on;
        window.__bifrostKeys = window.__bifrostKeys || {};
        if (on) window.__bifrostKeys.Space = true;
        else delete window.__bifrostKeys.Space;
      }
      btn.classList.toggle("is-held", on);
    };
    btn.addEventListener(
      "pointerdown",
      (e) => {
        try {
          btn.setPointerCapture(e.pointerId);
        } catch (_) {}
        focusPlaySurface();
        set(true);
        e.preventDefault();
      },
      { passive: false }
    );
    const clear = () => set(false);
    btn.addEventListener("pointerup", clear);
    btn.addEventListener("pointercancel", clear);
    btn.addEventListener("lostpointercapture", clear);
  };
  holdButton(jumpBtn, "south");
  holdButton(spinBtn, "west");
  holdButton(grappleBtn, "grapple");

  const canvas = $("bevy-canvas");
  if (canvas) {
    canvas.addEventListener(
      "touchstart",
      () => {
        focusPlaySurface();
      },
      { passive: true }
    );
  }
}

/** Collapsible HOW TO WIN / CONTROLS cards — default collapsed on mobile. */
function bindHowtoCollapse() {
  const cards = document.querySelectorAll(".hud-howto");
  if (!cards.length) return;
  const mobile = window.matchMedia("(max-width: 1100px)");
  const syncDefault = () => {
    cards.forEach((card) => {
      const toggle = card.querySelector("[data-howto-toggle]");
      if (mobile.matches) {
        card.classList.add("is-collapsed");
        if (toggle) toggle.setAttribute("aria-expanded", "false");
      } else {
        card.classList.remove("is-collapsed");
        if (toggle) toggle.setAttribute("aria-expanded", "true");
      }
    });
  };
  syncDefault();
  mobile.addEventListener?.("change", syncDefault);
  cards.forEach((card) => {
    const toggle = card.querySelector("[data-howto-toggle]");
    if (!toggle) return;
    toggle.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      const next = !card.classList.contains("is-collapsed");
      card.classList.toggle("is-collapsed", next);
      toggle.setAttribute("aria-expanded", next ? "false" : "true");
    });
  });
}

/** Embed top game bar — collapse for playfield; auto-collapse on match start. */
function setEmbedBarCollapsed(collapsed) {
  const nav = $("embed-options");
  const btn = $("btn-embed-collapse");
  if (!nav) return;
  nav.classList.toggle("is-collapsed", !!collapsed);
  if (btn) {
    btn.setAttribute("aria-expanded", collapsed ? "false" : "true");
    btn.title = collapsed ? "Expand game bar" : "Collapse game bar";
  }
}

function collapseHowtoForMatch() {
  document.querySelectorAll(".hud-howto").forEach((card) => {
    card.classList.add("is-collapsed");
    const toggle = card.querySelector("[data-howto-toggle]");
    if (toggle) toggle.setAttribute("aria-expanded", "false");
  });
}

function collapseEmbedBarForMatch() {
  collapseHowtoForMatch();
  if (!isEmbedded()) return;
  setEmbedBarCollapsed(true);
}

function bindEmbedCollapse() {
  const nav = $("embed-options");
  const btn = $("btn-embed-collapse");
  if (!nav || !btn || btn.dataset.bound === "1") return;
  btn.dataset.bound = "1";
  btn.addEventListener("click", (e) => {
    e.preventDefault();
    setEmbedBarCollapsed(!nav.classList.contains("is-collapsed"));
  });
}

/** Press ripple on buttons (cyan/gold aurora). */
function spawnBifrostRipple(clientX, clientY) {
  try {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  } catch (_) {}
  const host = document.createElement("span");
  host.className = "bifrost-ripple";
  host.setAttribute("aria-hidden", "true");
  host.style.left = `${clientX}px`;
  host.style.top = `${clientY}px`;

  const core = document.createElement("span");
  core.className = "bifrost-ripple-core";
  const ring = document.createElement("span");
  ring.className = "bifrost-ripple-ring";
  const mark = document.createElement("span");
  mark.className = "bifrost-ripple-mark";
  const sparks = document.createElement("span");
  sparks.className = "bifrost-ripple-sparks";
  for (let i = 0; i < 7; i++) {
    const spark = document.createElement("span");
    spark.className = "bifrost-ripple-spark";
    const angle = (Math.PI * 2 * i) / 7 + (Math.random() - 0.5) * 0.55;
    const dist = 16 + Math.random() * 42;
    spark.style.setProperty("--sx", `${Math.cos(angle) * dist}px`);
    spark.style.setProperty("--sy", `${Math.sin(angle) * dist}px`);
    spark.style.animationDelay = `${i * 0.018}s`;
    sparks.appendChild(spark);
  }
  host.append(core, ring, mark, sparks);
  document.body.appendChild(host);
  const clear = () => host.remove();
  host.addEventListener("animationend", clear, { once: true });
  window.setTimeout(clear, 900);
}

function bindButtonPressFeedback() {
  if (window.__bifrostPressBound) return;
  window.__bifrostPressBound = true;
  document.addEventListener(
    "pointerdown",
    (event) => {
      if (event.button != null && event.button !== 0) return;
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (target.closest("[data-press-skip], #bevy-canvas, canvas")) return;
      const btn = target.closest("button");
      if (!btn || btn.disabled || btn.classList.contains("is-disabled")) return;
      spawnBifrostRipple(event.clientX, event.clientY);
    },
    true
  );
}

if (document.readyState !== "loading") {
  bindButtonPressFeedback();
  bindTouchPad();
  bindHowtoCollapse();
  bindEmbedCollapse();
}