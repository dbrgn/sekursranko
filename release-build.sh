#!/usr/bin/env bash

set -euo pipefail

VERSION=$(grep '^version = ' Cargo.toml | sed 's/.*"\([0-9\.]*\)".*/\1/')
BIN=sekursranko
RUSTC=1.32

declare -a targets=(
    "stretch"
)

declare -a dockerimages=(
    "rust:$RUSTC-stretch"
)

function docker-download {
    echo "==> Downloading Docker image: $1"
    docker pull "$1"
}

function docker-build {
    echo "==> Building target $1 in image: $2"
    docker run --rm -it -v "$(pwd)":/code -w /code "$2" cargo build --release
    echo "==> Copying target $1"
    cp "target/release/$BIN" "dist-$VERSION/$BIN-$1"
}

echo -e "==> Version $VERSION\n"

rm -rf "dist-$VERSION"
mkdir "dist-$VERSION"

for image in "${dockerimages[@]}"; do docker-download "$image"; done
echo ""
for i in "${!targets[@]}"; do docker-build "${targets[$i]}" "${dockerimages[$i]}"; done
echo ""

for target in "${targets[@]}"; do
    echo "==> Stripping $target"
    cp "dist-$VERSION/$BIN-$target" "dist-$VERSION/$BIN-$target-debugsymbols"
    strip -s "dist-$VERSION/$BIN-$target"
done
echo ""

for target in "${targets[@]}"; do
    echo "==> Signing $target"
    gpg -a --output "dist-$VERSION/$BIN-$target.sig" --detach-sig "dist-$VERSION/$BIN-$target"
    gpg -a --output "dist-$VERSION/$BIN-$target-debugsymbols.sig" --detach-sig "dist-$VERSION/$BIN-$target-debugsymbols"
done
echo ""

echo "Done."
