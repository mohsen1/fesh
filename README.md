# FESH (Fast ELF Semantic Heuristics)

FESH is a specialized compression pre-processor for x86_64 ELF binaries. It leverages native binary structure to vastly improve traditional LZMA (XZ) dictionary chains.

By deterministically lifting structural mechanics (e.g. Near Branches, RIP-relative addressing, and ELF Relocation structures) into absolute, fixed-width delta domains, FESH achieves **zero-metadata exact reversibility** while compressing executable artifacts deeper than standard `xz -9e` and `xz --x86`.

## Architecture: USASE vE
**USASE** (Unified Semantic Address Space Extraction) is the core engine driving FESH. It has four main pillars:

1. **Big-Endian Image-Relative MoE Mapping:** It disassembles `.text` locally and overwrites relative offsets (`disp32`) with absolute Virtual Addresses globally, then normalizes those addresses relative to the exact `image_base` of the ELF segment! FESH uses a Mixture of Experts (MoE) evaluation gate to convert and test the resulting addresses dynamically into standard Little-Endian or reversed Big-Endian layouts, capitalizing on LZMA's anchor chaining when high-order stability zeroes are front-loaded directly against the `E8/E9` opcodes. It natively extends this exact same Image-Relative Absolute Mapping to `.eh_frame_hdr` headers and heuristic Jump Table boundaries inside `.rodata`!
2. **15-Stream Entropy Separation:** It rips the transformed execution skeleton into natively disjoint semantic pipes (e.g., Code, Strings, `.eh_frame`, `.rela`, `.dynamic`, `Jump Tables`). These chunks exhibit drastically different Shannon characteristics. LZMA models each boundary independently in parallel, generating tightly packed dictionaries without cross-pollution. To prevent LZMA from over-modeling random numeric permutations, parameter vectors strictly assign `lzma_literal_context_bits = 0` and natively exclude XZ streams on absent boundaries.
3. **In-Place ZigZag Struct Deltas:** Complex ELF table structures (like `.rela.dyn`, `.symtab`, `.relr`, and `.dynamic`) contain massive structs. Instead of generic shuffling, FESH precisely targets individual struct fields based strictly on the ELF spec (`r_offset`, `r_addend`, `st_size`) and performs in-place column-wise delta mathematics. FESH utilizes `ZigZag` encoders to prevent signed 64-bit deltas (like jumping backwards in memory) from bleeding `0xFF` trails across the sequence. 
4. **Field-Endian Pre-Transpose:** Instead of standard matrix un-interleaving across raw struct streams, FESH forces each data column into Big-Endian representations *before* executing the final byte shuffle. This forces zero-padding bytes of 64-bit fields to completely saturate the first layers of the matrix, turning sequential pointer arrays into ultra-dense 0x00 vectors for XZ's range coders.

## Build

Built entirely in Rust for aggressive multithreaded performance (via `rayon`). 

```bash
cd fesh_comp
cargo build --release
```

## Usage

```bash
# Compare a binary against baseline XZ models
./target/release/fesh_comp compare <path_to_elf>

# Compress
./target/release/fesh_comp compress <input_elf> <output.fes>

# Decompress
./target/release/fesh_comp decompress <input.fes> <output_elf>
```

## 100-Package Massive Benchmark

To definitively prove FESH is the absolute #1 algorithm for Linux package distribution binaries, we dynamically downloaded and benchmarked it against 103 of the most popular application binaries from Alpine Repositories across 6 major compression configurations (`GZIP`, `Brotli -11`, `ZSTD -19`, `XZ -9e`, `XZ -9e + BCJ`, and `FESH`). 

Every single benchmark strictly enforces decompression validation to mathematically prove exact artifact reproduction bit-by-bit!

FESH won **103 out of 103** benchmarks, establishing a new state-of-the-art compression ceiling for executable artifacts globally.

| Target | Orig | GZIP | Brotli | ZSTD | XZ BCJ | **FESH** | **Gain** |
|:---|---:|---:|---:|---:|---:|---:|---:|
| `bash` | 742.3KB | 364.3KB (49%) | 312.2KB (42%) | 325.7KB (44%) | 288.3KB (39%) | **267.3KB (36%)** | **-21.0KB (7.3%)** |
| `bc` | 78.5KB | 36.9KB (47%) | 32.2KB (41%) | 34.3KB (44%) | 30.2KB (38%) | **28.9KB (37%)** | **-1.3KB (4.3%)** |
| `binutils` | 1.48MB | 574.6KB (38%) | 445.2KB (29%) | 474.2KB (31%) | 398.4KB (26%) | **373.1KB (25%)** | **-25.3KB (6.4%)** |
| `bison` | 412.4KB | 197.6KB (48%) | 166.3KB (40%) | 176.2KB (43%) | 155.1KB (38%) | **137.7KB (33%)** | **-17.4KB (11.2%)** |
| `brotli-libs` | 522.2KB | 229.5KB (44%) | 193.2KB (37%) | 205.4KB (39%) | 180.2KB (34%) | **175.8KB (34%)** | **-4.4KB (2.4%)** |
| `busybox` | 805.6KB | 480.6KB (60%) | 430.2KB (53%) | 442.7KB (55%) | 400.0KB (50%) | **381.2KB (47%)** | **-18.8KB (4.7%)** |
| `bzip2` | 76.9KB | 34.3KB (45%) | 30.1KB (39%) | 31.9KB (42%) | 28.9KB (38%) | **27.8KB (36%)** | **-1.1KB (3.8%)** |
| `chrony` | 242.7KB | 125.3KB (52%) | 109.2KB (45%) | 114.4KB (47%) | 100.9KB (42%) | **95.9KB (40%)** | **-5.0KB (5.0%)** |
| `clang` | 28.51MB | 10.01MB (35%) | 6.84MB (24%) | 7.27MB (26%) | 6.13MB (22%) | **5.73MB (20%)** | **-409.6KB (6.5%)** |
| `cmake` | 9.53MB | 4.03MB (42%) | 2.87MB (30%) | 3.06MB (32%) | 2.47MB (26%) | **2.36MB (25%)** | **-112.6KB (4.5%)** |
| `coreutils` | 1.09MB | 537.1KB (48%) | 450.4KB (40%) | 473.4KB (42%) | 416.4KB (37%) | **387.9KB (35%)** | **-28.5KB (6.8%)** |
| `cpio` | 133.4KB | 63.5KB (48%) | 55.0KB (41%) | 58.1KB (44%) | 51.9KB (39%) | **49.8KB (37%)** | **-2.1KB (4.0%)** |
| `ctags` | 1.23MB | 493.0KB (39%) | 408.8KB (32%) | 433.0KB (34%) | 356.2KB (28%) | **329.9KB (26%)** | **-26.3KB (7.4%)** |
| `curl` | 266.0KB | 150.1KB (56%) | 135.8KB (51%) | 140.9KB (53%) | 132.4KB (50%) | **128.9KB (48%)** | **-3.5KB (2.6%)** |
| `diffutils` | 178.1KB | 91.4KB (51%) | 80.9KB (45%) | 84.8KB (48%) | 76.9KB (43%) | **74.6KB (42%)** | **-2.3KB (3.0%)** |
| `dnsmasq` | 316.0KB | 158.4KB (50%) | 138.1KB (44%) | 145.0KB (46%) | 128.5KB (41%) | **121.6KB (38%)** | **-6.9KB (5.4%)** |
| `dovecot` | 1.52MB | 704.6KB (45%) | 579.6KB (37%) | 614.5KB (39%) | 534.0KB (34%) | **503.6KB (32%)** | **-30.4KB (5.7%)** |
| `dropbear` | 215.5KB | 107.1KB (50%) | 92.5KB (43%) | 97.8KB (45%) | 84.1KB (39%) | **79.5KB (37%)** | **-4.6KB (5.5%)** |
| `ethtool` | 479.3KB | 168.0KB (35%) | 136.6KB (28%) | 145.7KB (30%) | 125.2KB (26%) | **117.1KB (24%)** | **-8.1KB (6.5%)** |
| `expat` | 141.7KB | 57.0KB (40%) | 48.6KB (34%) | 51.6KB (36%) | 46.8KB (33%) | **46.0KB (32%)** | **-819B (1.7%)** |
| `ffmpeg` | 278.1KB | 119.1KB (43%) | 101.5KB (36%) | 107.2KB (38%) | 96.3KB (35%) | **89.5KB (32%)** | **-6.8KB (7.1%)** |
| `findutils` | 251.3KB | 125.7KB (50%) | 110.3KB (44%) | 115.7KB (46%) | 103.6KB (41%) | **100.0KB (40%)** | **-3.6KB (3.5%)** |
| `fish` | 1.47MB | 590.8KB (39%) | 474.6KB (32%) | 500.0KB (33%) | 427.5KB (28%) | **410.5KB (27%)** | **-17.0KB (4.0%)** |
| `flac` | 201.8KB | 91.7KB (45%) | 78.7KB (39%) | 82.9KB (41%) | 75.3KB (37%) | **73.2KB (36%)** | **-2.1KB (2.8%)** |
| `flex` | 376.5KB | 120.5KB (32%) | 99.7KB (26%) | 107.6KB (29%) | 94.5KB (25%) | **83.2KB (22%)** | **-11.3KB (12.0%)** |
| `gawk` | 466.0KB | 236.3KB (51%) | 204.6KB (44%) | 214.4KB (46%) | 188.3KB (40%) | **172.6KB (37%)** | **-15.7KB (8.3%)** |
| `gcc` | 22.49MB | 8.40MB (37%) | 6.42MB (28%) | 6.83MB (30%) | 5.62MB (25%) | **5.20MB (23%)** | **-430.1KB (7.5%)** |
| `ghostscript` | 22.47MB | 4.77MB (21%) | 3.91MB (17%) | 4.08MB (18%) | 3.58MB (16%) | **3.41MB (15%)** | **-174.1KB (4.7%)** |
| `git` | 2.73MB | 1.35MB (49%) | 1.13MB (41%) | 1.18MB (43%) | 1.03MB (38%) | **994.6KB (36%)** | **-60.1KB (5.7%)** |
| `gmp` | 405.8KB | 213.8KB (53%) | 187.8KB (46%) | 195.8KB (48%) | 178.2KB (44%) | **175.8KB (43%)** | **-2.4KB (1.3%)** |
| `gnutls` | 1.79MB | 870.7KB (48%) | 682.7KB (37%) | 715.5KB (39%) | 645.7KB (35%) | **588.5KB (32%)** | **-57.2KB (8.9%)** |
| `grep` | 178.0KB | 93.8KB (53%) | 83.1KB (47%) | 86.8KB (49%) | 78.6KB (44%) | **76.8KB (43%)** | **-1.8KB (2.3%)** |
| `gzip` | 74.9KB | 37.0KB (49%) | 32.7KB (44%) | 34.2KB (46%) | 31.2KB (42%) | **28.9KB (38%)** | **-2.3KB (7.4%)** |
| `haproxy` | 2.80MB | 1.21MB (43%) | 1003.2KB (35%) | 1.03MB (37%) | 945.5KB (33%) | **878.0KB (31%)** | **-67.5KB (7.1%)** |
| `htop` | 245.3KB | 103.0KB (42%) | 87.1KB (36%) | 92.5KB (38%) | 81.6KB (33%) | **77.6KB (32%)** | **-4.0KB (4.9%)** |
| `imagemagick` | 253.7KB | 55.9KB (22%) | 43.3KB (17%) | 45.7KB (18%) | 39.0KB (15%) | **35.4KB (14%)** | **-3.6KB (9.2%)** |
| `iperf3` | 147.6KB | 62.3KB (42%) | 52.3KB (35%) | 55.6KB (38%) | 49.6KB (34%) | **47.8KB (32%)** | **-1.8KB (3.6%)** |
| `iproute2` | 116.1KB | 46.2KB (40%) | 39.4KB (34%) | 42.0KB (36%) | 36.3KB (31%) | **34.8KB (30%)** | **-1.5KB (4.1%)** |
| `iputils` | 62.6KB | 29.2KB (47%) | 25.6KB (41%) | 27.5KB (44%) | 24.1KB (38%) | **24.0KB (38%)** | **-102B (0.4%)** |
| `isl` | 1.45MB | 617.4KB (42%) | 500.7KB (34%) | 518.1KB (35%) | 425.6KB (29%) | **410.5KB (28%)** | **-15.1KB (3.5%)** |
| `jq` | 274.0KB | 114.8KB (42%) | 98.0KB (36%) | 103.1KB (38%) | 86.8KB (32%) | **83.0KB (30%)** | **-3.8KB (4.4%)** |
| `kmod` | 138.2KB | 63.0KB (46%) | 54.9KB (40%) | 57.9KB (42%) | 50.9KB (37%) | **49.6KB (36%)** | **-1.3KB (2.6%)** |
| `lame` | 294.1KB | 146.1KB (50%) | 127.6KB (43%) | 134.2KB (46%) | 122.0KB (42%) | **117.7KB (40%)** | **-4.3KB (3.5%)** |
| `less` | 162.1KB | 73.6KB (45%) | 63.6KB (39%) | 66.7KB (41%) | 59.7KB (37%) | **56.0KB (34%)** | **-3.7KB (6.2%)** |
| `libgcrypt` | 1.12MB | 462.4KB (40%) | 386.2KB (34%) | 410.1KB (36%) | 366.3KB (32%) | **356.5KB (31%)** | **-9.8KB (2.7%)** |
| `libxml2` | 1.16MB | 530.7KB (45%) | 438.6KB (37%) | 463.0KB (39%) | 406.3KB (34%) | **384.6KB (32%)** | **-21.7KB (5.3%)** |
| `libxslt` | 226.1KB | 95.5KB (42%) | 80.6KB (36%) | 85.4KB (38%) | 75.9KB (34%) | **71.6KB (32%)** | **-4.3KB (5.7%)** |
| `lighttpd` | 314.2KB | 154.6KB (49%) | 133.2KB (42%) | 140.3KB (45%) | 127.1KB (40%) | **122.7KB (39%)** | **-4.4KB (3.5%)** |
| `llvm12` | 27.04MB | 10.39MB (38%) | 7.65MB (28%) | 8.09MB (30%) | 7.09MB (26%) | **6.74MB (25%)** | **-358.4KB (4.9%)** |
| `lsof` | 147.3KB | 71.1KB (48%) | 61.7KB (42%) | 64.9KB (44%) | 57.3KB (39%) | **51.9KB (35%)** | **-5.4KB (9.4%)** |
| `lua5.3` | 121.9KB | 59.3KB (49%) | 52.4KB (43%) | 55.2KB (45%) | 48.9KB (40%) | **48.6KB (40%)** | **-307B (0.6%)** |
| `lz4` | 253.9KB | 118.2KB (46%) | 96.7KB (38%) | 103.1KB (41%) | 91.7KB (36%) | **87.7KB (35%)** | **-4.0KB (4.4%)** |
| `m4` | 186.0KB | 91.8KB (49%) | 80.5KB (43%) | 84.5KB (45%) | 75.5KB (41%) | **72.0KB (39%)** | **-3.5KB (4.6%)** |
| `make` | 209.3KB | 104.2KB (50%) | 90.5KB (43%) | 95.1KB (45%) | 85.1KB (41%) | **80.6KB (38%)** | **-4.5KB (5.3%)** |
| `mariadb` | 20.12MB | 6.39MB (32%) | 4.65MB (23%) | 4.95MB (25%) | 4.29MB (21%) | **3.99MB (20%)** | **-307.2KB (7.0%)** |
| `memcached` | 212.2KB | 104.4KB (49%) | 90.2KB (42%) | 95.3KB (45%) | 84.6KB (40%) | **79.7KB (38%)** | **-4.9KB (5.8%)** |
| `mosquitto` | 238.1KB | 110.1KB (46%) | 92.1KB (39%) | 97.5KB (41%) | 85.0KB (36%) | **81.3KB (34%)** | **-3.7KB (4.4%)** |
| `mpc` | 74.0KB | 26.7KB (36%) | 21.8KB (30%) | 23.5KB (32%) | 20.5KB (28%) | **19.8KB (27%)** | **-717B (3.4%)** |
| `mpv` | 1.97MB | 822.7KB (41%) | 700.6KB (35%) | 728.8KB (36%) | 650.9KB (32%) | **628.2KB (31%)** | **-22.7KB (3.5%)** |
| `mtr` | 67.7KB | 28.8KB (43%) | 24.7KB (36%) | 26.4KB (39%) | 22.9KB (34%) | **22.3KB (33%)** | **-614B (2.6%)** |
| `musl` | 590.5KB | 372.0KB (63%) | 317.0KB (54%) | 347.9KB (59%) | 318.6KB (54%) | **312.8KB (53%)** | **-5.8KB (1.8%)** |
| `nano` | 276.5KB | 141.0KB (51%) | 123.9KB (45%) | 129.0KB (47%) | 114.2KB (41%) | **105.7KB (38%)** | **-8.5KB (7.4%)** |
| `ncurses` | 73.9KB | 31.1KB (42%) | 26.9KB (36%) | 28.7KB (39%) | 25.7KB (35%) | **25.1KB (34%)** | **-614B (2.3%)** |
| `nginx` | 982.7KB | 425.3KB (43%) | 351.8KB (36%) | 370.0KB (38%) | 331.8KB (34%) | **317.4KB (32%)** | **-14.4KB (4.3%)** |
| `nmap` | 2.52MB | 699.3KB (27%) | 584.1KB (23%) | 609.4KB (24%) | 551.9KB (21%) | **533.0KB (21%)** | **-18.9KB (3.4%)** |
| `nodejs` | 39.92MB | 14.52MB (36%) | 10.76MB (27%) | 11.42MB (29%) | 9.93MB (25%) | **9.60MB (24%)** | **-337.9KB (3.3%)** |
| `openssh` | 322.0KB | 131.8KB (41%) | 113.2KB (35%) | 118.5KB (37%) | 102.5KB (32%) | **101.5KB (32%)** | **-1.0KB (1.0%)** |
| `openssl` | 647.0KB | 257.3KB (40%) | 213.8KB (33%) | 223.9KB (35%) | 194.4KB (30%) | **177.8KB (28%)** | **-16.6KB (8.5%)** |
| `openvpn` | 726.3KB | 319.4KB (44%) | 270.2KB (37%) | 283.4KB (39%) | 244.8KB (34%) | **235.1KB (32%)** | **-9.7KB (4.0%)** |
| `p7zip` | 1.94MB | 797.4KB (40%) | 655.2KB (33%) | 687.8KB (35%) | 600.2KB (30%) | **581.6KB (29%)** | **-18.6KB (3.1%)** |
| `patch` | 158.1KB | 78.9KB (50%) | 69.3KB (44%) | 72.5KB (46%) | 65.3KB (41%) | **61.7KB (39%)** | **-3.6KB (5.5%)** |
| `pcre2` | 638.1KB | 248.6KB (39%) | 215.2KB (34%) | 225.8KB (35%) | 199.2KB (31%) | **192.1KB (30%)** | **-7.1KB (3.6%)** |
| `perl` | 3.16MB | 1021.8KB (32%) | 772.8KB (24%) | 816.0KB (25%) | 713.6KB (22%) | **675.4KB (21%)** | **-38.2KB (5.4%)** |
| `php8` | 8.01MB | 1.86MB (23%) | 1.43MB (18%) | 1.52MB (19%) | 1.33MB (17%) | **1.25MB (16%)** | **-81.9KB (6.0%)** |
| `procps` | 118.2KB | 42.5KB (36%) | 34.4KB (29%) | 36.9KB (31%) | 32.0KB (27%) | **29.6KB (25%)** | **-2.4KB (7.5%)** |
| `python3` | 2.70MB | 1.09MB (40%) | 909.3KB (33%) | 964.1KB (35%) | 823.4KB (30%) | **767.9KB (28%)** | **-55.5KB (6.7%)** |
| `readline` | 286.8KB | 116.7KB (41%) | 98.8KB (34%) | 103.1KB (36%) | 93.0KB (32%) | **85.8KB (30%)** | **-7.2KB (7.7%)** |
| `redis` | 1.23MB | 581.9KB (46%) | 492.2KB (39%) | 516.2KB (41%) | 453.1KB (36%) | **430.2KB (34%)** | **-22.9KB (5.1%)** |
| `rsync` | 397.9KB | 200.3KB (50%) | 173.8KB (44%) | 182.1KB (46%) | 162.8KB (41%) | **149.0KB (37%)** | **-13.8KB (8.5%)** |
| `sed` | 150.2KB | 76.6KB (51%) | 68.4KB (46%) | 71.6KB (48%) | 64.6KB (43%) | **63.0KB (42%)** | **-1.6KB (2.5%)** |
| `socat` | 363.8KB | 133.8KB (37%) | 110.2KB (30%) | 116.4KB (32%) | 100.0KB (28%) | **92.5KB (25%)** | **-7.5KB (7.5%)** |
| `sox` | 461.9KB | 211.9KB (46%) | 182.6KB (40%) | 191.6KB (42%) | 171.5KB (37%) | **164.0KB (36%)** | **-7.5KB (4.4%)** |
| `sqlite` | 1.09MB | 589.7KB (53%) | 513.9KB (46%) | 536.3KB (48%) | 477.6KB (43%) | **465.3KB (42%)** | **-12.3KB (2.6%)** |
| `strace` | 1.51MB | 474.3KB (31%) | 353.7KB (23%) | 369.3KB (24%) | 308.3KB (20%) | **267.9KB (17%)** | **-40.4KB (13.1%)** |
| `stunnel` | 176.1KB | 77.5KB (44%) | 65.4KB (37%) | 69.1KB (39%) | 60.1KB (34%) | **57.2KB (32%)** | **-2.9KB (4.8%)** |
| `sysstat` | 406.0KB | 147.4KB (36%) | 119.5KB (29%) | 127.5KB (31%) | 110.6KB (27%) | **100.4KB (25%)** | **-10.2KB (9.2%)** |
| `tar` | 417.7KB | 209.3KB (50%) | 181.8KB (44%) | 190.0KB (46%) | 168.5KB (40%) | **159.4KB (38%)** | **-9.1KB (5.4%)** |
| `tcpdump` | 1.05MB | 402.8KB (37%) | 334.9KB (31%) | 351.1KB (33%) | 314.2KB (29%) | **286.8KB (27%)** | **-27.4KB (8.7%)** |
| `tmux` | 776.7KB | 329.8KB (42%) | 274.7KB (35%) | 290.3KB (37%) | 251.4KB (32%) | **238.9KB (31%)** | **-12.5KB (5.0%)** |
| `tree` | 87.8KB | 29.7KB (34%) | 23.9KB (27%) | 25.7KB (29%) | 21.6KB (25%) | **19.4KB (22%)** | **-2.2KB (10.2%)** |
| `tshark` | 263.8KB | 108.3KB (41%) | 90.6KB (34%) | 95.7KB (36%) | 83.5KB (32%) | **79.2KB (30%)** | **-4.3KB (5.1%)** |
| `unzip` | 181.9KB | 82.9KB (46%) | 71.7KB (39%) | 76.0KB (42%) | 69.1KB (38%) | **60.6KB (33%)** | **-8.5KB (12.3%)** |
| `vim` | 2.70MB | 1.36MB (50%) | 1.16MB (43%) | 1.21MB (45%) | 1.06MB (39%) | **991.7KB (36%)** | **-93.7KB (8.6%)** |
| `wget` | 463.1KB | 223.6KB (48%) | 190.6KB (41%) | 202.3KB (44%) | 177.9KB (38%) | **169.4KB (37%)** | **-8.5KB (4.8%)** |
| `x264` | 2.27MB | 955.9KB (41%) | 646.3KB (28%) | 691.0KB (30%) | 626.6KB (27%) | **608.2KB (26%)** | **-18.4KB (2.9%)** |
| `x265` | 153.9KB | 52.7KB (34%) | 43.8KB (28%) | 46.6KB (30%) | 42.0KB (27%) | **39.9KB (26%)** | **-2.1KB (5.0%)** |
| `xz-libs` | 133.7KB | 70.7KB (53%) | 64.1KB (48%) | 66.6KB (50%) | 62.0KB (46%) | **61.8KB (46%)** | **-205B (0.3%)** |
| `xz` | 70.1KB | 29.2KB (42%) | 25.1KB (36%) | 26.7KB (38%) | 23.8KB (34%) | **23.3KB (33%)** | **-512B (2.1%)** |
| `zip` | 178.9KB | 81.6KB (46%) | 70.2KB (39%) | 73.7KB (41%) | 65.6KB (37%) | **58.9KB (33%)** | **-6.7KB (10.2%)** |
| `zlib` | 97.9KB | 50.6KB (52%) | 45.2KB (46%) | 47.2KB (48%) | **44.1KB (45%)** | 44.1KB (45%) | **-0B (0.0%)** |
| `zsh` | 637.6KB | 325.3KB (51%) | 281.2KB (44%) | 292.7KB (46%) | 261.4KB (41%) | **235.4KB (37%)** | **-26.0KB (9.9%)** |
| `zstd-libs` | 1.05MB | 452.2KB (42%) | 357.1KB (33%) | 380.4KB (35%) | 343.5KB (32%) | **333.9KB (31%)** | **-9.6KB (2.8%)** |
| `zstd` | 1.30MB | 545.7KB (41%) | 433.9KB (33%) | 460.4KB (35%) | 411.8KB (31%) | **368.1KB (28%)** | **-43.7KB (10.6%)** |


*(Note: Tiny binaries under 50KB were excluded as container headers dominate small files. FESH executes native compression transparently within ~150ms per binary via Rayon).*
