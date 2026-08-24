# Privacy & security

## Data collected

- **None persisted** — room codes and tickets are ephemeral in memory
- Low-cardinality metrics (`bifrost_active_rooms`) on `/metrics`

## P2P exposure

WebRTC peers may learn each other's network addresses. Game inputs travel peer-to-peer after signaling; the signaling server does not store gameplay.

## TURN

When enabled, relayed traffic passes through your Coturn instance. Credentials are short-lived HMAC-SHA1 per Coturn's REST API style.

## Browser requirements

Modern Chromium or Firefox with WebRTC. The shell surfaces failures instead of hanging when signaling is unreachable.

## Reporting

See [SECURITY.md](../SECURITY.md).
