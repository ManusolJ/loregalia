# Metadata sources

Findings from each API's official pages, checked on 2026-11-05 and 2026-11-07. Terms change, so
recheck a source before building its provider. The choices are decisions D-09 to D-12, D-20 and
D-28.

## Recommended

| Media | Source                                       | Credentials            | Rate limit                                  | Credit required                         | Storing data                                                    |
| ----- | -------------------------------------------- | ---------------------- | ------------------------------------------- | --------------------------------------- | --------------------------------------------------------------- |
| Shows | [TVmaze](https://www.tvmaze.com/api)         | None                   | At least 20 calls per 10 s, per IP          | Credit and link back (CC BY-SA 4.0)     | Images may be cached indefinitely                               |
| Movies | [TMDB](https://developer.themoviedb.org/docs/faq) | Free key, per account | About 40 requests per second, may change    | Approved logo plus a notice in About or Credits | [Six months at most](https://www.themoviedb.org/api-terms-of-use) |
| Books | [Open Library](https://openlibrary.org/developers/api) | None             | 1 per second, or 3 with User-Agent and email | Courtesy link for covers                | "Cache responses whenever possible"; no bulk downloads          |
| Games | [IGDB](https://api-docs.igdb.com/)           | Client ID and secret   | 4 per second, 8 open requests               | Visible credit (formally for partners)  | "We prefer if you store" it; see the conflict below             |

All four are free for a non-commercial app. TVmaze's license is the only one here that clearly allows
commercial use as well. **If Loregalia ever makes money, recheck TMDB and IGDB first:** TMDB needs a
commercial license, and IGDB needs a partnership.

## What this means for the design

- **Every provider is optional.** None of these sources promises uptime: TMDB says it offers no SLA,
  and the others link no status page. Typing an item by hand must always work, and saved items must
  work offline.
- **TMDB and IGDB go through the Loregalia relay (D-11)**, so users set nothing up. A user may enter
  their own keys in Settings, and the app then calls those sources directly. Shows and books need no
  key at all.
  - **IGDB** only accepts application credentials from a
    [Confidential](https://api-docs.igdb.com/#account-creation) Twitch client. Twitch describes that
    flow as being for
    [apps that can securely store a secret](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/),
    and a desktop installer cannot. This is why the secret stays on the relay.
  - **TMDB's** pages do not say whether one key may serve many users through a relay. The project
    assumes it may (see Assumptions).
- **TMDB data must be refreshed or removed within six months.** Store its ID, refresh details and
  images before they expire, and never let the user's own fields depend on them.
- **Cover shapes:** TVmaze has poster images, and IGDB's `cover_big` is 264 x 374, both close to the
  2:3 tile in `DESIGN.md`. Open Library and TMDB do not state a shape on the pages read.
- **Open Library covers:** request by cover ID (`cover_i`), because ISBN lookups are limited to 100
  requests per 5 minutes per IP. Add `?default=false` so a missing cover returns 404 instead of a
  blank image.
- **User-Agent:** send a descriptive one to TVmaze, Open Library and Wikidata. Open Library's higher
  limit needs a contact email in it (D-21). Never use a user's personal address.

## Anime (checked 2026-11-07)

TVmaze returned six well-known anime with images, Japanese language and seasons. One search for
"Mob Psycho 100" returned the live-action show instead, so the UI must show a list of matches, not
the top result. Anime films come from TMDB.

| Source | Finding |
| ------ | ------- |
| [AniList](https://docs.anilist.co/guide/terms-of-use) | Bans use in "anime and manga list or tracker services", including media data. Currently [30 requests a minute](https://docs.anilist.co/guide/rate-limiting) |
| [MyAnimeList](https://myanimelist.net/static/apiagreement.html) | Free apps allowed, but the Client ID may not be shared, so each user would need their own |
| [Jikan](https://docs.api.jikan.moe/) | Scrapes MyAnimeList, whose agreement bans scraping by API users |
| [Kitsu](https://kitsu.app/terms) | Public API, but its terms grant a personal license; not cleared |
| AniDB | Has an HTTP API, but needs a registered client name and version, allows about one request every two seconds, and bans clients that go over. It asks clients to [cache locally](https://wiki.anidb.net/Udp), and search usually works from its title dump, downloaded at most once a day. Its wiki blocks automated reads, so these come from its UDP spec and [user reports](https://www.filebot.net/forums/viewtopic.php?p=51839) |

## Games without IGDB (checked 2026-11-07)

| Source | Finding |
| ------ | ------- |
| Steam store | Answers search and details with no key, with 600x900 portrait covers, but it is not an official API, and the [Steam Subscriber Agreement](https://store.steampowered.com/subscriber_agreement/) bans scripts and bots |
| [Steam Web API](https://steamcommunity.com/dev/apiterms) | Official, but keys may not be shared, and each key needs a domain |
| GOG catalog | Answers search with no key and has portrait covers; not an official API, and its terms were not read |
| Epic store, itch.io | Blocked (403) or rate-limited (429) on the first request |
| [Hasheous](https://hasheous.org/index.html?page=termsofservice) | Relays IGDB, needs an app-level client key, and is a free volunteer service; credit the original source |
| [SteamGridDB](https://www.steamgriddb.com/terms) | Terms say personal, non-commercial use and restrict automated use |

## How similar apps handle keys (checked 2026-11-07)

| Strategy | Examples |
| -------- | -------- |
| Own relay server holds the keys | [Playnite](https://github.com/JosefNemec/PlayniteExtensions) for IGDB, [Sonarr](https://github.com/Sonarr/Sonarr) and [Radarr](https://github.com/Radarr/Radarr) for TV and movie data |
| Keys built into the app, users may override | [Tonkatsu Box](https://github.com/hacan359/tonkatsu_box) adds TMDB and IGDB keys at release from CI secrets; [Yamtrack](https://github.com/FuzzyGrim/Yamtrack) and [Jellyfin](https://github.com/jellyfin/jellyfin) ship default keys |
| Users bring their own keys | [Ryot](https://github.com/IgnisDa/ryot), [MediaTracker](https://github.com/bonukai/MediaTracker) |
| A community relay | [RomM](https://docs.romm.app/latest/getting-started/metadata-providers/) through Hasheous |

Radarr users have complained when its server was down, so the app must keep working without the
relay (see `ARCHITECTURE.md`).

## Unresolved

- **IGDB storage:** IGDB's FAQ prefers local storage, but the
  [Twitch agreement](https://legal.twitch.com/legal/developer-agreement/) that governs its API limits
  caching to 24 hours without authorization. Follow the stricter rule: 24 hours.
- **Open Library's license:** it asserts no new rights, but warns that individual records may have
  existing rights issues. Its pages do not state CC0.
- **Local AI use:** TMDB's terms prohibit training AI systems on its content, so never put its data
  into training sets.

## Assumptions

The project is built on these. If a source says otherwise, the design changes then.

- **Donations:** a free app with an optional Ko-fi link is non-commercial for TMDB and IGDB (D-25).
  TMDB calls a project commercial if "the primary purpose is to create revenue", but its terms also
  ban revenue "directly or indirectly", and neither source mentions donations.
- **Relays:** one key may serve every user through the relay (D-11). Neither TMDB nor IGDB says.

## Rejected for now

| Source                                                | Why                                                                                                                                |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| [Giant Bomb](https://www.giantbomb.com/api/)          | Its API is currently unavailable after the site's move from Fandom                                                                  |
| [RAWG](https://rawg.io/apidocs)                       | Its site and docs timed out on every attempt on 2026-11-05, so its terms could not be confirmed. Third parties report 20,000 requests a month with a required link |
| [OMDb](https://www.omdbapi.com/legal.htm)             | Terms limit use to personal use, conflict with its CC BY-NC notice, and restrict storing data; its poster API is for patrons only   |
| [Hardcover](https://docs.hardcover.app/api/getting-started/) | Still in beta: it "may change or break at any time", and tokens may be reset without notice                                 |

## Possible fallbacks

| Source                                                                  | Use                 | Note                                                                                       |
| ----------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------ |
| [Google Books](https://developers.google.com/books/terms)               | Books               | Needs a key; quota not stated on the pages read; terms dated 2012; apps may not charge users |
| [Wikidata](https://www.wikidata.org/wiki/Wikidata:Data_access)          | Games, without setup | CC0 with no key, but interfaces aren't guaranteed stable, and cover art is unverified      |
| [TheGamesDB](https://thegamesdb.net/)                                   | Games               | Has portrait box art; key and limits sit behind a login, so its terms are unverified       |
