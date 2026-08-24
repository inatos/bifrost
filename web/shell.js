const $ = (id) => document.getElementById(id);

const panel = $("panel");
const status = $("status");
const roomInput = $("room-code");
const inspector = $("inspector");

function setStatus(msg) {
  status.textContent = msg;
}

function launch(query) {
  const url = new URL(window.location.href);
  url.search = "";
  for (const [k, v] of Object.entries(query)) {
    if (v != null && v !== "") url.searchParams.set(k, v);
  }
  window.location.href = url.toString();
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

$("btn-bot").addEventListener("click", () => {
  panel.classList.remove("hidden");
  launch({ bot: "1" });
});

$("btn-create").addEventListener("click", async () => {
  panel.classList.remove("hidden");
  setStatus("Creating private room…");
  try {
    const data = await api("/api/rooms", { protocol_version: 1 });
    roomInput.value = data.room_code;
    sessionStorage.setItem("bifrost_ticket", data.host_ticket);
    setStatus(`Room ${data.room_code} ready — share code with guest.`);
    launch({ room: data.room_code, ticket: data.host_ticket });
  } catch (e) {
    setStatus(`Could not create room: ${e.message}`);
  }
});

$("btn-join").addEventListener("click", async () => {
  panel.classList.remove("hidden");
  const code = roomInput.value.trim().toUpperCase();
  if (!code) {
    setStatus("Enter a room code first.");
    return;
  }
  setStatus("Joining room…");
  try {
    const data = await api("/api/rooms/join", { protocol_version: 1, room_code: code });
    sessionStorage.setItem("bifrost_ticket", data.guest_ticket);
    setStatus(`Joined ${code}. Waiting for host…`);
    launch({ room: code, ticket: data.guest_ticket });
  } catch (e) {
    setStatus(`Join failed: ${e.message}`);
  }
});

$("btn-copy").addEventListener("click", async () => {
  const code = roomInput.value.trim();
  if (!code) return;
  const link = `${window.location.origin}${window.location.pathname}?room=${code}`;
  await navigator.clipboard.writeText(link);
  setStatus("Invite link copied.");
});

$("btn-launch").addEventListener("click", () => {
  const code = roomInput.value.trim();
  const ticket = sessionStorage.getItem("bifrost_ticket");
  launch({ room: code || undefined, ticket: ticket || undefined, bot: code ? undefined : "1" });
});

$("btn-inspector").addEventListener("click", (e) => {
  const on = e.currentTarget.getAttribute("aria-pressed") !== "true";
  e.currentTarget.setAttribute("aria-pressed", on ? "true" : "false");
  inspector.classList.toggle("hidden", !on);
});

$("lag-forge").addEventListener("change", (e) => {
  const frames = e.target.value;
  const url = new URL(window.location.href);
  if (frames === "0") url.searchParams.delete("lag_frames");
  else url.searchParams.set("lag_frames", frames);
  window.history.replaceState({}, "", url);
});

// Hydrate from query string
const params = new URLSearchParams(window.location.search);
if (params.get("room")) {
  panel.classList.remove("hidden");
  roomInput.value = params.get("room");
  setStatus(`Room ${params.get("room")} — launch when ready.`);
}
