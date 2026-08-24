# Room protocol

Version: `protocol_version = 1` (see `bifrost_protocol::PROTOCOL_VERSION`).

## Create room

`POST /api/rooms`

```json
{ "protocol_version": 1 }
```

Response:

```json
{
  "room_code": "AB12CD34",
  "host_ticket": "<url-safe token>",
  "signal_url": "https://bifrost.arathyll.com/signal/AB12CD34/<token>",
  "expires_at": "2026-08-24T12:00:00Z"
}
```

## Join room

`POST /api/rooms/join`

```json
{ "protocol_version": 1, "room_code": "AB12CD34" }
```

Returns `guest_ticket` and `signal_url`. Third join attempts receive **409 room full**.

## TURN credentials

`GET /api/turn` returns short-lived Coturn credentials when `TURN_SECRET` and `TURN_URLS` are configured server-side.

## TTL

Rooms expire after `ROOM_TTL_SECS` (default 900). No match history is stored.
