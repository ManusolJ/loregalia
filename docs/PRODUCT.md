# Product

Loregalia is a local-first desktop app for planning what to watch, read and play next, and for
keeping a record of what is finished. It covers movies, shows, books and games in one place.

It is a planner, not a progress tracker. It helps decide what to pick up next, so it does not record
which episode or page you stopped at.

## First version

- **Lists:** create lists with custom names. One item can belong to several lists.
- **Tags:** label lists and items with your own tags.
- **Tracked media:** movies, books and games are single items. Shows are tracked at show and season
  level, with no episode tracking.
- **Statuses:** move an item between states such as To watch, Watched, In progress, Dropped and On
  hold. The set is data, so it can grow.
- **Metadata lookup:** search public APIs to add an item with its title, cover and details filled
  in, instead of typing them by hand.

## Metadata sources

Each source sits behind a common provider interface, so more can be added later. Lookup is optional:
typing an item by hand always works. Sources and their limits are in `METADATA.md`, and the final
choices are open in `DECISIONS.md`.

## Out of scope for now

The data model should leave room for these, but do not build them.

- Episode-level tracking
- Ratings, reviews and notes
- Statistics, such as books read this year
- JSON or CSV export and import
- Import from services like Goodreads or Letterboxd
- Sync across devices, and a mobile version
