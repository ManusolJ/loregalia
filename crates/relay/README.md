# Loregalia relay

The relay holds the project's TMDB and IGDB credentials so the app ships no keys. It is a separate
binary and is not part of the Windows installer.

## Configuration

| Variable                  | Default              | Meaning                              |
| ------------------------- | -------------------- | ------------------------------------ |
| `LOREGALIA_RELAY_BIND`    | `127.0.0.1:8080`     | Address the relay listens on          |
| `LOREGALIA_RELAY_DATABASE`| `relay-cache.db`     | SQLite cache file                     |
| `LOREGALIA_RELAY_TRUST_PROXY` | unset            | Trust `X-Forwarded-For` from loopback |
| `TMDB_TOKEN`              | unset                | TMDB v4 read access token             |
| `IGDB_CLIENT_ID`          | unset                | Twitch application client ID          |
| `IGDB_CLIENT_SECRET`      | unset                | Twitch application client secret      |

Keys come from the process environment only. They are never written to the repository, the database
or a log.

## Run it

```sh
cargo run -p loregalia-relay
```

The relay binds to localhost by default. Put it behind an HTTPS reverse proxy such as Caddy before
exposing it. The app reaches it by domain name, with a local default of `http://127.0.0.1:8080` for
development.

Current endpoints:

```text
GET /healthz
GET /v1/movies/search?q=...
GET /v1/movies/{id}
GET /v1/games/search?q=...
GET /v1/games/{id}
```

`/healthz` reports the relay version and whether TMDB and IGDB credentials are configured.

## Cache

The cache is a `STRICT` SQLite table with one body per key and an expiry time. TMDB searches live
for one hour and TMDB details for 30 days, both within TMDB's six-month limit. IGDB searches and
details live for 24 hours.

## Tests

```sh
cargo test -p loregalia-relay
```

Tests use in-memory databases, fake inputs and explicit clocks. They never call TMDB, IGDB or Twitch.
