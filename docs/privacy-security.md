# Privacy & security

## Data collected

- **No accounts / no match DB** — room codes and tickets are ephemeral in the signal process (`DashMap`) and in the browser tab for the session
- Lobby reclaim may use a short-lived `sessionStorage` blob so a refresh can still `leave` a room if `sendBeacon` was missed — not a durable identity
- Tickets are **not** put in shareable URLs (avoids desktop “save password” prompts and leaking join tokens)
- Low-cardinality metrics (`bifrost_active_rooms`) on `/metrics`

## P2P exposure

WebRTC peers may learn each other's network addresses. Game inputs travel peer-to-peer after signaling; the signaling server does not store gameplay.

## TURN

When enabled, relayed traffic passes through your Coturn instance. Credentials are short-lived HMAC-SHA1 per Coturn's REST API style (`GET /api/turn`). Without TURN, many cross-NAT pairs fail ICE and never reach Ready Up.

## Browser requirements

Modern Chromium or Firefox with WebRTC. The shell surfaces failures instead of hanging when signaling is unreachable.

## Reporting

See [SECURITY.md](../SECURITY.md).
