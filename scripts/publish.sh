#!/bin/bash

set -ex

DIRS=$(find ./packages -type d | sort -r)

for dir in $DIRS
do
  if [ -f "$dir/package.json" ]; then
    echo "Publishing $dir"
    (cd $dir && npm publish --access public)
  fi
done
