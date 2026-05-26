#!/bin/bash

source .env

if [ -z "DIRECTORY" ]; then
    echo "Set environment variable DIRECTORY to location of website definition."
    echo "It can be set in a .env file in the current directory"
    exit 1
fi

inotifywait -m -r -e modify,create,delete,move \
	  --exclude "serve" "$DIRECTORY" | while read -r event; do
    echo "Changes detected! $event"
    cargo run -- --config $DIRECTORY/config.toml
done
