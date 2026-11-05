# Metadata sources

Findings from each API's official pages, checked on 2026-11-05. Terms change, so recheck a source
before building its provider. The final choices are decisions D-09 to D-12 and D-20.

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
- **Users bring their own TMDB and IGDB credentials**, entered in Settings and kept out of the repo
  and the installer. Shows and books work with no setup.
  - **IGDB** only accepts application credentials from a
    [Confidential](https://api-docs.igdb.com/#account-creation) Twitch client. Twitch describes that
    flow as being for
    [apps that can securely store a secret](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/),
    and a desktop installer cannot. A shared secret would also need a backend, which this app does
    not have. Getting credentials requires a Twitch account with two-factor authentication.
  - **TMDB's** pages do not say whether a key may ship in a distributed app, so this is the cautious
    reading. Ask TMDB before changing it.
- **TMDB data must be refreshed or removed within six months.** Store its ID, refresh details and
  images before they expire, and never let the user's own fields depend on them.
- **Cover shapes:** TVmaze has poster images, and IGDB's `cover_big` is 264 x 374, both close to the
  2:3 tile in `DESIGN.md`. Open Library and TMDB do not state a shape on the pages read.
- **Open Library covers:** request by cover ID (`cover_i`), because ISBN lookups are limited to 100
  requests per 5 minutes per IP. Add `?default=false` so a missing cover returns 404 instead of a
  blank image.
- **User-Agent:** send a descriptive one to TVmaze, Open Library and Wikidata. Open Library's higher
  limit needs a contact email in it (D-21). Never use a user's personal address.

## Unresolved

- **IGDB storage:** IGDB's FAQ prefers local storage, but the
  [Twitch agreement](https://legal.twitch.com/legal/developer-agreement/) that governs its API limits
  caching to 24 hours without authorization. Ask IGDB, or follow the stricter rule, before release.
- **Open Library's license:** it asserts no new rights, but warns that individual records may have
  existing rights issues. Its pages do not state CC0.
- **Local AI use:** TMDB's terms prohibit training AI systems on its content, so never put its data
  into training sets.

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
