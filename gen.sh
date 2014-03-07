#!/bin/sh

mkdir -p gen
cd gen

GEN_C_SOURCE=gen_constants.c

echo '#include <stdio.h>
#include <sys/ioctl.h>
#include <asm/types.h>
#include <linux/videodev2.h>

#define PRINT_CONST(c) printf("pub static %s: c_ulong = 0x%x;\n", #c, c)

int main(void) {
    printf("#[allow(dead_code)];\n\nuse libc::c_ulong;\n\n");
' > $GEN_C_SOURCE

IFS=$'\n'
for defineline in $(grep '^#define[ 	]\+[_a-zA-Z0-9]\+[ 	]\+' /usr/include/linux/videodev2.h |
                    sed 's/[ 	]\+/ /g' | sed 's/\"/\\\"/g' | sed 's/\\$/ \.\.\./g'); do
    echo "    printf(\"//${defineline}\n\");" >> $GEN_C_SOURCE
    const=`echo "$defineline" | cut -d ' ' -f 2`
    echo "    PRINT_CONST($const);" >> $GEN_C_SOURCE
done
unset IFS

echo "
    return 0;
};" >> $GEN_C_SOURCE

gcc -o gen_constants gen_constants.c && ./gen_constants > constants.rs

echo '#[feature(globs)];' > videodev2.rs_
echo '#[allow(non_camel_case_types)];' >> videodev2.rs_
bindgen /usr/include/linux/videodev2.h -match videodev2.h >> videodev2.rs_
sed -e 's/__u8/u8/g' \
    -e 's/__u16/u16/g' \
    -e 's/__u32/u32/g' \
    -e 's/__u64/u64/g' \
    -e 's/__s32/i32/g' \
    -e 's/__s64/i64/g' \
    -e 's/__le32/u32/g' \
    -e 's/__syscall_slong_t/c_slong/g' \
    videodev2.rs_ > videodev2.rs
