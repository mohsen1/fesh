# FESH (Fast ELF Semantic Heuristics)

FESH is a specialized compression pre-processor for x86_64 ELF binaries. It leverages native binary structure to vastly improve traditional LZMA (XZ) dictionary chains.

By deterministically lifting structural mechanics (e.g. Near Branches, RIP-relative addressing, and ELF Relocation structures) into absolute, fixed-width delta domains, FESH achieves **zero-metadata exact reversibility** while compressing executable artifacts deeper than standard `xz -9e` and `xz --x86`.

## Architecture: USASE vB
**USASE** (Unified Semantic Address Space Extraction) is the core engine driving FESH. It has three main pillars:

1. **Big-Endian MoE Target Mapping:** It disassembles `.text` locally and overwrites relative offsets (`disp32`) with absolute Virtual Addresses globally. FESH uses a Mixture of Experts (MoE) evaluation gate to convert and test the resulting addresses dynamically into standard Little-Endian or reversed Big-Endian layouts, capitalizing on LZMA's anchor chaining when high-order stability bytes are front-loaded against opcodes. It extends this exact same `pcrel` Absolute Mapping to the `.eh_frame_hdr` headers without requiring explicit DWARF reconstruction layers!
2. **N-Stream Entropy Separation:** It rips the transformed execution skeleton into natively disjoint semantic pipes (e.g., Code, Strings, `.eh_frame`, `.rela`). These chunks exhibit drastically different Shannon characteristics. LZMA models each boundary independently in parallel, generating tightly packed dictionaries without cross-pollution. To prevent LZMA from over-modeling random numeric permutations, parameter vectors strictly assign `lzma_literal_context_bits = 0`.
3. **In-Place ZigZag Struct Deltas:** Complex ELF table structures (like `.rela.dyn`, `.symtab`, `.relr`, and `.dynamic`) contain massive structs. Instead of generic shuffling, FESH precisely targets individual struct fields based strictly on the ELF spec (`r_offset`, `r_addend`, `st_size`) and performs in-place column-wise delta mathematics. FESH utilizes `ZigZag` encoders to prevent signed 64-bit deltas (like jumping backwards in memory) from bleeding `0xFF` trails across the sequence. The changes are implicitly synchronized and natively reversible without a single metadata byte.

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

## Massive 25-Package Linux Benchmark

To definitively prove FESH is the #1 algorithm for Linux package distribution binaries, we benchmarked it against 25 of the most popular packages across 6 major compression configurations. 

FESH won **25 out of 25** benchmarks, establishing a new state-of-the-art compression ceiling for ELF artifacts.

| Binary | Orig Size | GZIP -9 | BZIP2 -9 | ZSTD -19 | XZ -9e | XZ -9e + BCJ | **FESH** | Winner |
|:---|---:|---:|---:|---:|---:|---:|---:|:---|
| `bash` | 760,096 | 372,995 (49.1%) | 360,663 (47.4%) | 333,558 (43.9%) | 315,952 (41.6%) | 295,228 (38.8%) | **273,868 (36.0%)** | **FESH** |
| `binutils` | 1,469,904 | 227,901 (15.5%) | 231,365 (15.7%) | 189,395 (12.9%) | 174,588 (11.9%) | 164,588 (11.2%) | **150,406 (10.2%)** | **FESH** |
| `bzip2` | 78,744 | 35,139 (44.6%) | 35,970 (45.7%) | 32,702 (41.5%) | 30,564 (38.8%) | 29,596 (37.6%) | **28,422 (36.1%)** | **FESH** |
| `curl` | 272,400 | 153,733 (56.4%) | 161,651 (59.3%) | 144,235 (52.9%) | 138,392 (50.8%) | 135,532 (49.8%) | **131,953 (48.4%)** | **FESH** |
| `gawk` | 477,208 | 241,951 (50.7%) | 235,327 (49.3%) | 219,530 (46.0%) | 206,860 (43.3%) | 192,856 (40.4%) | **176,756 (37.0%)** | **FESH** |
| `gcc` | 1,111,336 | 415,123 (37.4%) | 411,430 (37.0%) | 369,183 (33.2%) | 343,740 (30.9%) | 327,424 (29.5%) | **317,360 (28.6%)** | **FESH** |
| `git` | 2,867,080 | 1,410,850 (49.2%) | 1,282,730 (44.7%) | 1,238,061 (43.2%) | 1,173,824 (40.9%) | 1,083,716 (37.8%) | **1,019,700 (35.6%)** | **FESH** |
| `grep` | 182,320 | 96,055 (52.7%) | 92,997 (51.0%) | 88,853 (48.7%) | 83,920 (46.0%) | 80,520 (44.2%) | **78,518 (43.1%)** | **FESH** |
| `gzip` | 76,664 | 37,859 (49.4%) | 38,060 (49.6%) | 34,974 (45.6%) | 32,956 (43.0%) | 31,944 (41.7%) | **29,479 (38.5%)** | **FESH** |
| `jq` | 272,456 | 117,575 (43.2%) | 116,081 (42.6%) | 107,169 (39.3%) | 100,212 (36.8%) | 89,732 (32.9%) | **86,400 (31.7%)** | **FESH** |
| `less` | 165,984 | 75,408 (45.4%) | 75,458 (45.5%) | 68,330 (41.2%) | 64,260 (38.7%) | 61,092 (36.8%) | **57,275 (34.5%)** | **FESH** |
| `make` | 214,352 | 106,724 (49.8%) | 103,769 (48.4%) | 97,405 (45.4%) | 91,668 (42.8%) | 87,096 (40.6%) | **82,570 (38.5%)** | **FESH** |
| `nano` | 283,152 | 144,358 (51.0%) | 141,025 (49.8%) | 132,135 (46.7%) | 125,088 (44.2%) | 116,976 (41.3%) | **108,260 (38.2%)** | **FESH** |
| `nginx` | 1,006,328 | 435,515 (43.3%) | 409,646 (40.7%) | 378,881 (37.6%) | 353,996 (35.2%) | 339,812 (33.8%) | **325,056 (32.3%)** | **FESH** |
| `nmap` | 2,641,392 | 716,038 (27.1%) | 717,107 (27.1%) | 624,013 (23.6%) | 593,300 (22.5%) | 565,196 (21.4%) | **546,388 (20.7%)** | **FESH** |
| `redis` | 1,285,880 | 595,815 (46.3%) | 565,135 (43.9%) | 528,640 (41.1%) | 497,480 (38.7%) | 464,004 (36.1%) | **441,154 (34.3%)** | **FESH** |
| `rsync` | 407,464 | 205,140 (50.3%) | 197,424 (48.5%) | 186,436 (45.8%) | 176,432 (43.3%) | 166,660 (40.9%) | **152,644 (37.5%)** | **FESH** |
| `sed` | 153,816 | 78,438 (51.0%) | 76,729 (49.9%) | 73,309 (47.7%) | 68,972 (44.8%) | 66,168 (43.0%) | **64,289 (41.8%)** | **FESH** |
| `sqlite` | 1,147,600 | 603,890 (52.6%) | 563,129 (49.1%) | 549,178 (47.9%) | 519,116 (45.2%) | 489,108 (42.6%) | **476,920 (41.6%)** | **FESH** |
| `strace` | 1,583,520 | 485,639 (30.7%) | 493,115 (31.1%) | 378,120 (23.9%) | 336,836 (21.3%) | 315,736 (19.9%) | **274,392 (17.3%)** | **FESH** |
| `tar` | 427,680 | 214,295 (50.1%) | 207,486 (48.5%) | 194,610 (45.5%) | 182,908 (42.8%) | 172,504 (40.3%) | **163,272 (38.2%)** | **FESH** |
| `tmux` | 795,304 | 337,685 (42.5%) | 327,656 (41.2%) | 297,276 (37.4%) | 275,880 (34.7%) | 257,440 (32.4%) | **245,101 (30.8%)** | **FESH** |
| `tree` | 89,872 | 30,437 (33.9%) | 30,816 (34.3%) | 26,280 (29.2%) | 24,372 (27.1%) | 22,148 (24.6%) | **19,772 (22.0%)** | **FESH** |
| `xz` | 71,832 | 29,874 (41.6%) | 29,713 (41.4%) | 27,367 (38.1%) | 25,660 (35.7%) | 24,400 (34.0%) | **23,812 (33.1%)** | **FESH** |
| `zstd` | 1,361,880 | 558,805 (41.0%) | 524,021 (38.5%) | 471,436 (34.6%) | 439,516 (32.3%) | 421,640 (31.0%) | **377,027 (27.7%)** | **FESH** |


*(Note: Tiny binaries under 50KB were excluded as container headers dominate small files. All tests execute FESH compression natively in sub-200ms using Rayon multithreading).*
