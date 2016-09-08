# `before_deploy` phase: here we package the build artifacts

set -ex

. $(dirname $0)/utils.sh

# Generate artifacts for release
mk_artifacts() {
    RUSTFLAGS="-C target-feature=+ssse3" \
      cargo build --target $TARGET --release --features simd-accel
}

mk_tarball() {
    # create a "staging" directory
    local td=$(mktempd)
    local out_dir=$(pwd)
    local name="${PROJECT_NAME}-${TRAVIS_TAG}-${TARGET}"
    mkdir "$td/$name"

    cp target/$TARGET/release/rg "$td/$name/"
    cp {README,UNLICENSE,COPYING,LICENSE_MIT} "$td/$name/"

    pushd $td
    tar czf "$out_dir/$name.tar.gz" *
    popd
    rm -r $td
}

main() {
    mk_artifacts
    mk_tarball
}

main
