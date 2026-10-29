# Automatic phase correction

Phase correction adjusts how the measured signal is displayed, helping distorted peaks return to their expected shape. For this check, we deliberately misaligned copies of the same measurements and ran Auto phase again. The raw measurements were unchanged.

## What the percentages mean

The graph shows how closely Auto returned to its starting result. Lower is better. These percentages measure consistency, not sample purity or absolute accuracy. Broad single-signal spectra remain difficult.

For each version, we compared the corrected trace with that version’s own automatically corrected starting trace. We tried four added phase changes and reported the largest relative RMS difference: the size of the difference between the traces, expressed as a percentage of the starting trace’s size. No rescaling or interpolation was used.

The added phase changes (PH0, PH1), in degrees, were (50, −80), (90, 0), (−150, 45), and (10, 250). The data come from the project’s public test fixtures. Measurements are in `measurements.csv`; `python3 plot.py` regenerates the graph with Matplotlib.

## Before and after Auto

The two images are direct plot exports from the updated app using the same carbon spectrum. The first has a deliberately added 90° phase misalignment; the second follows Auto phase. Neither has baseline correction applied. They demonstrate the correction workflow, rather than compare application versions.
