#!/bin/bash

echo "Downloading Extended Linux x86_64 Binaries for Benchmark..."
mkdir -p linux_bench
cd linux_bench

curl -sL "https://dl-cdn.alpinelinux.org/alpine/v3.15/main/x86_64/gcc-10.3.1_git20211027-r0.apk" > gcc.apk
tar -zxf gcc.apk usr/bin/gcc 2>/dev/null && mv usr/bin/gcc ./gcc_elf || true
tar -zxf gcc.apk usr/bin/g++ 2>/dev/null && mv usr/bin/g++ ./g++_elf || true

curl -sL "https://dl-cdn.alpinelinux.org/alpine/v3.15/main/x86_64/binutils-2.37-r3.apk" > binutils.apk
tar -zxf binutils.apk usr/bin/ld 2>/dev/null && mv usr/bin/ld ./ld_elf || true
tar -zxf binutils.apk usr/bin/objdump 2>/dev/null && mv usr/bin/objdump ./objdump_elf || true
tar -zxf binutils.apk usr/bin/nm 2>/dev/null && mv usr/bin/nm ./nm_elf || true

curl -sL "https://dl-cdn.alpinelinux.org/alpine/v3.15/main/x86_64/musl-1.2.2-r7.apk" > musl.apk
tar -zxf musl.apk lib/libc.musl-x86_64.so.1 2>/dev/null && mv lib/libc.musl-x86_64.so.1 ./libc_elf || true

curl -sL "https://dl-cdn.alpinelinux.org/alpine/v3.15/main/x86_64/bash-5.1.16-r0.apk" > bash.apk
tar -zxf bash.apk bin/bash 2>/dev/null && mv bin/bash ./bash_elf || true

curl -sL "https://dl-cdn.alpinelinux.org/alpine/v3.15/main/x86_64/python3-3.9.18-r0.apk" > python3.apk
tar -zxf python3.apk usr/bin/python3.9 2>/dev/null && mv usr/bin/python3.9 ./python_elf || true

cd ..
echo "--- Running Benchmarks ---"
cat << 'PYEOF' > bench_xz.py
import sys
import lzma
import os
import subprocess
import gzip

def get_best_xz(data):
    c1 = lzma.compress(data, preset=9)
    c1e = lzma.compress(data, preset=9 | lzma.PRESET_EXTREME)
    filters = [{"id": lzma.FILTER_X86}, {"id": lzma.FILTER_LZMA2, "preset": 9}]
    c2 = lzma.compress(data, format=lzma.FORMAT_XZ, filters=filters)
    filters_e = [{"id": lzma.FILTER_X86}, {"id": lzma.FILTER_LZMA2, "preset": 9 | lzma.PRESET_EXTREME}]
    c2e = lzma.compress(data, format=lzma.FORMAT_XZ, filters=filters_e)
    return len(c1), len(c1e), len(c2), len(c2e)

def get_zstd(file_path):
    try:
        # Assumes zstd is installed.
        r = subprocess.run(["zstd", "-19", "-f", file_path, "-o", file_path + ".zst"], capture_output=True)
        size = os.path.getsize(file_path + ".zst")
        os.remove(file_path + ".zst")
        return size
    except Exception:
        return 0

def get_gzip(data):
    return len(gzip.compress(data, compresslevel=9))

if __name__ == "__main__":
    if len(sys.argv) > 1:
        p = sys.argv[1]
        data = open(p, "rb").read()
        s1, s1e, s2, s2e = get_best_xz(data)
        zstd_s = get_zstd(p)
        gz_s = get_gzip(data)
        print(f"GZIP -9:                  {gz_s} bytes")
        print(f"ZSTD -19:                 {zstd_s} bytes")
        print(f"XZ -9:                    {s1} bytes")
        print(f"XZ -9e:                   {s1e} bytes")
        print(f"XZ -9e + BCJ:             {s2e} bytes")
PYEOF

for file in linux_bench/*_elf; do
    if [ -f "$file" ]; then
        echo "=============================================="
        echo "Benchmarking: $file"
        python3 bench_xz.py "$file" || true
        ./fesh_comp/target/release/fesh_comp compare "$file"
    fi
done

# Cleanup bench files
rm -rf linux_bench bench_xz.py
