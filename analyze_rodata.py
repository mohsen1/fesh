import struct
from elftools.elf.elffile import ELFFile

def analyze(filepath):
    with open(filepath, 'rb') as f:
        elf = ELFFile(f)
        text_sec = elf.get_section_by_name('.text')
        rodata_sec = elf.get_section_by_name('.rodata')
        
        if not text_sec or not rodata_sec:
            return
            
        text_va = text_sec['sh_addr']
        text_size = text_sec['sh_size']
        
        rodata_data = rodata_sec.data()
        rodata_va = rodata_sec['sh_addr']
        
        # Look for entry_va + rel32
        runs = []
        current_run = 0
        
        for i in range(0, len(rodata_data) - 3, 4):
            val = struct.unpack('<i', rodata_data[i:i+4])[0]
            entry_va = rodata_va + i
            
            target_1 = entry_va + val
            # target_2 = table_base_va + val (harder to guess base without context, usually start of run)
            
            if text_va <= target_1 < text_va + text_size:
                current_run += 1
            else:
                if current_run >= 3:
                    runs.append(current_run)
                current_run = 0
                
        if current_run >= 3:
            runs.append(current_run)
            
        print(f"File: {filepath}")
        print(f"Found {len(runs)} jump tables (entry_va + rel32).")
        print(f"Total entries: {sum(runs)}")

for f in ["massive_bench/gcc_elf", "massive_bench/ld_elf", "massive_bench/objdump_elf", "massive_bench/bash_elf", "massive_bench/git_elf"]:
    try:
        analyze(f)
    except Exception as e:
        print(f"Error on {f}: {e}")

