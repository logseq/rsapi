#!/bin/bash

set -ex
set -o pipefail

find ./packages -iname '*.node' -exec rm -rfv {} \;
