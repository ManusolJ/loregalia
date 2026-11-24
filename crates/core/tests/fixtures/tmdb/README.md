# TMDB fixtures

`search-dune.json` and `details-dune.json` are live responses. `details-dune-credits.json` is the same
response with a `credits` object added by hand, in the shape of
<https://developer.themoviedb.org/reference/movie-credits>, because it was written without a key.
Replace it with a live `append_to_response=credits` response when one is at hand.
