#!/bin/bash

cd client
cargo make build_release
mkdir -p ../server/public
cp -r pkg ../server/public/pkg
#cp index.html ../server/public/index.html
cd ..

cd server
mkdir -p target/release/public
cd -r public target/release/public
cargo build --release
cd ..

cd server/target/release
./server