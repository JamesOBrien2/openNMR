#!/bin/sh
# Download public Bruker datasets into testdata/bruker/cheminfo (not committed):
# cheminfo/bruker-data-test (MIT), pinned so the golden files stay comparable.
# Ibuprofen 1H/13C/COSY/HSQC/HMBC, cyclosporin (DIGMOD 3), NUS 2Ds, and a
# range of 1D acquisitions.
set -eu
COMMIT=89a722d085f87e4bc56f7bee9f74b007181d4bdc
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEST="$ROOT/testdata/bruker/cheminfo"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -sSL "https://github.com/cheminfo/bruker-data-test/archive/$COMMIT.tar.gz" | tar -xz -C "$TMP"
cd "$TMP"/bruker-data-test-*/data/zipped
find . -name '*.zip' | while read -r zip; do
  out="$DEST/${zip#./}"
  out="${out%.zip}"
  rm -rf "$out"
  mkdir -p "$out"
  unzip -q -o "$zip" -d "$out" -x '__MACOSX/*' 2>/dev/null || unzip -q -o "$zip" -d "$out"
  rm -rf "$out/__MACOSX"
done
echo "Datasets in $DEST"
