#!/bin/bash


DIRS=$(find ./packages -type d -maxdepth 1 | sort -r)


for dir in $DIRS
do
  if [ -f "$dir/package.json" ]; then
    echo "Publishing $dir"
    (cd $dir && npm publish --access public)
  fi
done
