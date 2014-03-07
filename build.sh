#!/bin/sh

mkdir -p bin lib

gcc -shared -fpic -o lib/libxioctl.so src/uvcview/xioctl.c
rustc -L lib src/uvcview/main.rs -o bin/uvcview
