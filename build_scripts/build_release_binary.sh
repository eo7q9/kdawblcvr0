# This script takes care of building your crate and packaging it for release

set -ex

export TRAVIS_TAG=$GIT_TAG

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    # create binary
    cargo build --release --target $TARGET

    # pack it up
    cp target/$TARGET/release/bouncyquencer $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
