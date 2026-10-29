# Automatic phase correction — reviewer figures

Code baseline: `b041838cf529f138d2488435b2d50d37c104bd97`.
Updated code: `48a891a0ccd64580c4a8465b0a56e4328548095e`.

## Consistency comparison

For each version and input, apply its automatic phase result to produce a reference real array. Then add each known phase error `(PH0, PH1)` in degrees: `(50, -80)`, `(90, 0)`, `(-150, 45)`, `(10, 250)`. Run automatic correction again and compare the corrected real array with that version's reference. Report `100 × sqrt(sum((actual-reference)^2) / sum(reference^2))`, taking the worst of the four errors. There is no fitted rescaling or interpolation.

This measures consistency, not absolute scientific accuracy. The broad single-signal example remains unreliable. The graph includes it to make that limitation visible. Values are in `measurements.csv`; regenerate the plot with `python3 plot.py` (requires Matplotlib).

The public input corpus is fetched by the project's `tools/reference/fetch_testdata.sh`, pinned at `89a722d085f87e4bc56f7bee9f74b007181d4bdc`. Raw FID SHA-256 identifiers:

- Ibuprofen carbon: `3bbbdfba014a6d188b8d3e9aaa827a8ce0e8b4464c36c1d8f940a6f2aa4ac772`
- Aspirin proton: `d9a91d9fc8a140a0725ffbd1ccd65727c4f6202b901c0b211b5541193339ec8c`
- Reference carbon: `b8b340f32ae4ba718c8faf4857653d6f298aa94999319294c997e75cc7283193`
- Broad rubidium: `053ffdeb59a77f990434c293c15b7b22884bbbb81b1aac3f050ba77790327624`

## Native screenshots

These show the updated implementation, not a comparison of application versions. Both use the same public carbon input with 65,536 points, no window, no baseline correction, and pivot 0.

- Before: deliberately add 90° to PH0, giving approximately PH0 −45.3°, PH1 −259.2°.
- After: choose Auto phase, giving approximately PH0 −135.2°, PH1 −259.3°.

The screenshots are direct, unmodified captures. Processing is reversible and the raw input is unchanged. This demonstrates the workflow, not independent accuracy.
