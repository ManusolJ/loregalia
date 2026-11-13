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

The relay binds to localhost by default and stops cleanly on Ctrl+C or `SIGTERM`. Put it behind an
HTTPS reverse proxy before exposing it; see [Deploy](#deploy). The app reaches it by domain name, with
a local default of `http://127.0.0.1:8080` for development.

Current endpoints:

```text
GET /healthz
GET /v1/movies/search?q=...
GET /v1/movies/{id}
GET /v1/games/search?q=...
GET /v1/games/{id}
```

`/healthz` reports the relay version and whether TMDB and IGDB credentials are configured.

## Deploy

`deploy/relay` runs the relay behind [Caddy](https://caddyserver.com/), which gets and renews an
HTTPS certificate on its own. The relay shares Caddy's network, so it sees Caddy as a loopback peer
and rate-limits each user's real address. It works the same on a home server (D-26) or a VPS.

You need:

- A Linux machine with Docker and the Compose plugin
- A domain name pointing at that machine, such as `relay.example.com`. At home, a dynamic DNS name
  works if your address changes
- Ports 80 and 443 reachable from the internet. At home, forward both from the router to the machine
- The project's TMDB read access token, and the Twitch client ID and secret for IGDB

Then, on that machine:

```sh
git clone https://github.com/ManusolJ/loregalia.git
cd loregalia/deploy/relay
cp relay.env.example relay.env
chmod 600 relay.env
nano relay.env
docker compose --env-file relay.env up -d --build
curl https://relay.example.com/healthz
```

Fill in `relay.env` with the domain and the keys. It is ignored by Git and never leaves the machine.
`/healthz` should report both sources as configured. To update, run `git pull` and the same
`docker compose` command again. The cache and certificates live in Docker volumes, so they survive
updates.

Once it answers over HTTPS, set the repository variable `LOREGALIA_RELAY_URL` to
`https://relay.example.com` under Settings, Secrets and variables, Actions, Variables. The release
workflow builds the app with it.

## Cache

The cache is a `STRICT` SQLite table with one body per key and an expiry time. TMDB searches live
for one hour and TMDB details for 30 days, both within TMDB's six-month limit. IGDB searches and
details live for 24 hours.

## Tests

```sh
cargo test -p loregalia-relay
```

Tests use in-memory databases, fake inputs and explicit clocks. They never call TMDB, IGDB or Twitch.
