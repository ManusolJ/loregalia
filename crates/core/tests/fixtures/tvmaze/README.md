# TVmaze fixtures

`search-a.json` is a recorded response from `GET https://api.tvmaze.com/search/shows?q=a`,
captured before 2022-03-19. Tests use it to check mapping and never call the network.

Refresh it with:

```sh
cargo run -p loregalia-core --example fetch_tvmaze_fixture -- a search-a.json
```

The data is from [TVmaze](https://www.tvmaze.com/api), used under CC BY-SA 4.0.
