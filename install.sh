#!/bin/bash

cargo build --release
sudo cp target/release/2048 /usr/local/bin
