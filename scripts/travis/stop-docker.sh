#!/bin/bash

set -euxo pipefail

main() {
    docker stop dev
}

main
