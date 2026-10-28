"""Write a synthetic Bruker 1H experiment (ethanol with CHCl3 and TMS) that
includes a digital filter delay, so the app can be tried without real data.

Usage: python synthetic_1d.py [experiment_dir]
"""

import sys
from math import comb
from pathlib import Path

import nmrglue as ng
import numpy as np

default = Path(__file__).resolve().parents[2] / "testdata" / "bruker" / "synthetic_ethanol" / "1"
out = Path(sys.argv[1]) if len(sys.argv) > 1 else default
td = 65536
bf1 = 400.13
o1p = 4.7
sw_ppm = 16.0
grpdly = 67.9841766357422
sfo1 = bf1 + o1p * bf1 * 1e-6
sw = sw_ppm * sfo1

# (shift ppm, J Hz, multiplicity, protons)
peaks = [(1.22, 7.0, 3, 3), (3.69, 7.0, 4, 2), (2.61, 0.0, 1, 1), (7.26, 0.0, 1, 0.1), (0.0, 0.0, 1, 0.3)]

t = np.arange(td // 2) / sw
fid = np.zeros(td // 2, dtype=complex)
for ppm, j, mult, protons in peaks:
    for k in range(mult):
        offset = (ppm - o1p) * sfo1 + (k - (mult - 1) / 2) * j
        weight = protons * comb(mult - 1, k) / 2 ** (mult - 1)
        # Bruker FIDs rotate negatively for positive offsets: nmrglue's fft then
        # puts higher shifts at lower indices.
        fid += weight * np.exp(-2j * np.pi * offset * t - t / 0.8)

# A broad background signal gives a baseline hump for baseline correction.
fid += 6 * np.exp(-2j * np.pi * (2.0 - o1p) * sfo1 * t - t / 0.004)

# Delay the FID by the group delay, the inverse of nmrglue's rm_dig_filter shift.
fid = ng.proc_base.fsh2(fid, -grpdly)
rng = np.random.default_rng(1)
fid = fid / np.abs(fid).max() * 2e7 + rng.normal(0, 400, fid.shape) + 1j * rng.normal(0, 400, fid.shape)
fid = np.round(fid.real) + 1j * np.round(fid.imag)

acqus = {
    "_coreheader": ["##TITLE= Synthetic data from openNMR tools/reference", "##JCAMPDX= 5.0"],
    "_comments": [],
    "TD": td, "SW_h": sw, "SW": sw_ppm, "SFO1": sfo1, "BF1": bf1, "O1": o1p * sfo1,
    "NUC1": "1H", "AQ_mod": 3, "DTYPA": 0, "BYTORDA": 0, "GRPDLY": grpdly,
    "DSPFVS": 20, "DECIM": 1024, "PULPROG": "zg30", "SOLVENT": "CDCl3",
}
procs = {
    "_coreheader": ["##TITLE= Synthetic data from openNMR tools/reference", "##JCAMPDX= 5.0"],
    "_comments": [],
    "SF": bf1, "SI": 65536, "OFFSET": o1p + sw_ppm / 2,
}

(out / "pdata" / "1").mkdir(parents=True, exist_ok=True)
ng.bruker.write(str(out), {"acqus": acqus}, fid, overwrite=True, write_prog=False)
ng.bruker.write_jcamp(procs, str(out / "pdata" / "1" / "procs"), overwrite=True)

dic, data = ng.bruker.read(str(out))
assert np.allclose(data[: td // 2], fid), "round trip failed"

# A processed spectrum in pdata/1, scaled to int32 with NC_proc.
spec = ng.bruker.rm_dig_filter(data[: td // 2], 1024, 20, grpdly, truncate_grpdly=False)
spec = ng.proc_base.fft(ng.proc_base.zf_size(ng.proc_base.em(spec, 0.3 / sw), 65536)).real
nc_proc = max(0, int(np.ceil(np.log2(np.abs(spec).max() / 2**30))))
uc = ng.fileiobase.unit_conversion(spec.size, True, sw, bf1, (sfo1 - bf1) * 1e6)
procs.update({"SI": spec.size, "SW_p": sw, "NC_proc": nc_proc, "OFFSET": uc.ppm(0),
              "BYTORDP": 0, "DTYPP": 0, "AXNUC": "1H"})
ng.bruker.write_jcamp(procs, str(out / "pdata" / "1" / "procs"), overwrite=True)
np.round(spec / 2**nc_proc).astype("<i4").tofile(out / "pdata" / "1" / "1r")
print(f"wrote {out}: {data.shape[0]} complex points, GRPDLY {dic['acqus']['GRPDLY']}")
