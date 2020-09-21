#!/bin/bash

set -euxo pipefail

main() {
    #
    # Files created within a docker container can't be used outside
    # by the host without root. So Change file permissions so that
    # travis can create a cache archive.
    #
    sudo chown -R travis:travis $TRAVIS_HOME/.cargo
    sudo chown -R travis:travis $TRAVIS_HOME/.rustup
    sudo chown -R travis:travis $TRAVIS_HOME/.cache
    sudo chown -R travis:travis $TRAVIS_BUILD_DIR/dex/target
    sudo chown -R travis:travis $TRAVIS_BUILD_DIR/crank/target

    docker stop dev
}

main
