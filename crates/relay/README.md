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

`deploy/relay` runs the relay in Docker on the host's network, listening on `127.0.0.1:8082` only.
A [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
on the same machine publishes it over HTTPS, so no router port needs to be opened. Because the
tunnel connects over loopback, the relay trusts the client address Cloudflare adds to
`X-Forwarded-For` and rate-limits each user separately. With a published Docker port instead, every
user would arrive from Docker's bridge and share one limit.

The container runs as an unprivileged user with a read-only filesystem, no capabilities,
`no-new-privileges`, 128 MB of memory, rotated logs and a healthcheck on `/healthz`.

You need:

- A Linux machine with Docker, the Compose plugin and a running `cloudflared` tunnel
- A hostname in a Cloudflare zone, such as `relay.example.com`
- The project's TMDB read access token, and the Twitch client ID and secret for IGDB

On that machine:

```sh
git clone https://github.com/ManusolJ/loregalia.git
cd loregalia/deploy/relay
cp relay.env.example relay.env
chmod 600 relay.env
nano relay.env
docker compose --env-file relay.env up -d --build
docker builder prune -f --filter until=168h
curl -s http://127.0.0.1:8082/healthz
```

`relay.env` holds the keys and the port. It is ignored by Git and never leaves the machine.
`/healthz` should report both sources as configured. The Rust build leaves a large Docker build
cache, which is why the prune follows it.

Then publish it through the tunnel:

1. In the Cloudflare dashboard, add a proxied `CNAME` from the hostname to
   `<tunnel-id>.cfargotunnel.com`.
2. Add an ingress rule to the tunnel's `config.yml`, before the catch-all `http_status:404`:

   ```yaml
   - hostname: relay.example.com
     service: http://127.0.0.1:8082
   ```

   Use `127.0.0.1`, not `localhost`, because the relay does not listen on IPv6.
3. Run `cloudflared tunnel ingress validate`, then restart `cloudflared`.
4. From another network, check that the app's requests get through, not only a browser's:

   ```sh
   curl -s https://relay.example.com/healthz
   curl -s -A "Loregalia/1.0.0 (+https://github.com/ManusolJ/loregalia)" "https://relay.example.com/v1/movies/search?q=dune" | head -c 200
   ```

   If either returns a Cloudflare challenge page instead of JSON, the zone's bot protection is
   blocking API clients. Turn off Bot Fight Mode for the zone, or on a paid plan add a rule that
   skips it for this hostname.

Finally, set the repository variable `LOREGALIA_RELAY_URL` to `https://relay.example.com` under
Settings, Secrets and variables, Actions, Variables. The release workflow builds the app with it.

To update the relay, run `git pull` and the same `docker compose` and prune commands. The cache lives
in the `loregalia-relay_relay-data` volume. It needs no backup: it refills itself. Back up
`relay.env`, or keep the keys somewhere you can recreate it from.

## Cache

The cache is a `STRICT` SQLite table with one body per key and an expiry time. TMDB searches live
for one hour and TMDB details for 30 days, both within TMDB's six-month limit. IGDB searches and
details live for 24 hours.

## Tests

```sh
cargo test -p loregalia-relay
```

Tests use in-memory databases, fake inputs and explicit clocks. They never call TMDB, IGDB or Twitch.
