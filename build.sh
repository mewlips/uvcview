#!/bin/sh

mkdir -p bin lib

gcc -shared -fpic -o lib/libxioctl.so src/uvcview/xioctl.c

BS=
old_ifs="$IFS"
IFS=":"
for pkg_path in $RUST_PATH; do
    LIBS="$LIBS -L $pkg_path/lib"
done
IFS="$old_ifs"

rustc -L lib $LIBS  src/uvcview/main.rs -o bin/uvcview
