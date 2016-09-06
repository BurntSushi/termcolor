# `before_deploy` phase: here we package the build artifacts

set -ex

. $(dirname $0)/utils.sh

# Generate artifacts for release
mk_artifacts() {
    RUSTFLAGS="-C target-feature=+ssse3" cargo build --target $TARGET --release --features simd-accel
}

mk_tarball() {
    # create a "staging" directory
    local td=$(mktempd)
    local out_dir=$(pwd)

    # TODO update this part to copy the artifacts that make sense for your project
    # NOTE All Cargo build artifacts will be under the 'target/$TARGET/{debug,release}'
    cp target/$TARGET/release/xrep $td

    pushd $td

    # release tarball will look like 'rust-everywhere-v1.2.3-x86_64-unknown-linux-gnu.tar.gz'
    tar czf $out_dir/${PROJECT_NAME}-${TRAVIS_TAG}-${TARGET}.tar.gz *

    popd
    rm -r $td
}

main() {
    mk_artifacts
    mk_tarball
}

main
