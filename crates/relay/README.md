# Loregalia relay

The relay holds the project's TMDB and IGDB credentials so the app ships no keys. It is a separate
binary and is not part of the Windows installer.

## Configuration

| Variable                  | Default              | Meaning                              |
| ------------------------- | -------------------- | ------------------------------------ |
| `LOREGALIA_RELAY_BIND`    | `127.0.0.1:8080`     | Address the relay listens on          |
| `LOREGALIA_RELAY_DATABASE`| `relay-cache.db`     | SQLite cache file                     |
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
```

`/healthz` reports the relay version and whether TMDB and IGDB credentials are configured. TMDB and
IGDB search endpoints arrive in the next provider steps.

## Cache

The cache is a `STRICT` SQLite table with one body per key and an expiry time. TMDB entries must not
live longer than six months; IGDB entries must not live longer than 24 hours. The provider steps set
those limits.

## Tests

```sh
cargo test -p loregalia-relay
```

Tests use in-memory databases, fake inputs and explicit clocks. They never call TMDB, IGDB or Twitch.
