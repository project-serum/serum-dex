#!/bin/bash

set -euxo pipefail

main() {
    docker pull projectserum/development:latest
    #
    # Bind the relevant host directories to the docker image so that the
    # relevant files are synced.
    #
    docker volume create --driver local \
           --opt type=none \
           --opt device=$TRAVIS_HOME/.cargo \
           --opt o=bind \
           cargodir
    docker volume create --driver local \
           --opt type=none \
           --opt device=$TRAVIS_HOME/.rustup \
           --opt o=bind \
           rustupdir
    docker volume create --driver local \
           --opt type=none \
           --opt device=$TRAVIS_HOME/.cache \
           --opt o=bind \
           cachedir
    docker volume create --driver local \
           --opt type=none \
           --opt device=$TRAVIS_BUILD_DIR \
           --opt o=bind \
           workdir
    #
    # Start the container.
    #
    docker run -it -d --net host --name dev \
           -v cargodir:/root/.cargo \
           -v rustupdir:/root/.rustup \
           -v cachedir:/root/.cache \
           -v workdir:/workdir \
           projectserum/development:latest bash
}

main
