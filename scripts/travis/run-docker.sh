#!/bin/bash

set -euxo pipefail

main() {
    docker pull projectserum/build:v0.17.0
    #
    # Bind the relevant host directories to the docker image so that the
    # files are synced.
    #
    docker volume create --driver local \
           --opt type=none \
           --opt device=$TRAVIS_BUILD_DIR \
           --opt o=bind \
           workdir
    #
    # Start the container.
    #
    docker run -it -d --net host --name dev \
           -v workdir:/workdir \
           projectserum/build:v0.17.0 bash
}

main
