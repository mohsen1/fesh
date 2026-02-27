use byteorder::{ByteOrder, LittleEndian};
use iced_x86::{Decoder, DecoderOptions};
use object::{Architecture, Object, ObjectSection, ObjectSegment, SectionKind};
use rayon::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::time::Instant;
use xz2::stream::{Check, Filters, LzmaOptions, Stream};

const MAGIC: &[u8; 4] = b"FESC";

const CAT_OTHER: u8 = 0;
const CAT_CODE: u8 = 1;
const CAT_STR: u8 = 2;
const CAT_S2: u8 = 3;
const CAT_S4: u8 = 4;
const CAT_S8: u8 = 5;
const CAT_RELR8: u8 = 6;
const CAT_S16: u8 = 7;
const CAT_REL16: u8 = 8;
const CAT_DYNAMIC16: u8 = 9;
const CAT_S24: u8 = 10;
const CAT_RELA24: u8 = 11;
const CAT_SYM24: u8 = 12;
const CAT_EH: u8 = 13;
const CAT_JT4: u8 = 14;
const CAT_COUNT: usize = 15;

const XZ_CHECK: Check = Check::None;
const PRESET_EXTREME: u32 = 1u32 << 31;

fn choose_pb(cat: usize) -> u32 {
    match cat {
        c if c == CAT_CODE as usize => 2,
        c if c == CAT_EH as usize => 2,
        c if c == CAT_OTHER as usize => 2,
        _ => 0, // All numeric / transposed streams benefit from pb=0
    }
}

fn choose_dict_size(stream_len: usize) -> u32 {
    let min_ds: usize = 1 << 16;
    let max_ds: usize = 1 << 26;
    let mut ds = stream_len.max(min_ds);
    ds = ds.next_power_of_two();
    ds = ds.clamp(min_ds, max_ds);
    ds as u32
}

fn compress_xz_tuned(data: &[u8], preset: u32, pb: u32, dict_size: u32) -> Vec<u8> {
    if data.is_empty() { return Vec::new(); }
    let mut opts = LzmaOptions::new_preset(preset).expect("bad preset");
    opts.position_bits(pb).dict_size(dict_size);
    let mut filters = Filters::new();
    filters.lzma2(&opts);
    let stream = Stream::new_stream_encoder(&filters, XZ_CHECK).expect("xz encoder");
    let mut enc = xz2::write::XzEncoder::new_stream(Vec::new(), stream);
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn decompress_xz(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() { return Ok(Vec::new()); }
    let mut decoder = xz2::read::XzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn write_varint(buf: &mut Vec<u8>, mut val: u64) {
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
            buf.push(byte);
        } else {
            buf.push(byte);
            break;
        }
    }
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut val = 0u64;
    let mut shift: u32 = 0;
    loop {
        if *pos >= buf.len() { return Err("varint eof".into()); }
        let byte = buf[*pos];
        *pos += 1;
        val |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 { return Ok(val); }
        shift += 7;
        if shift >= 64 { return Err("varint overflow".into()); }
    }
}

fn shuffle_bytes(data: &[u8], stride: usize) -> Vec<u8> {
    if data.is_empty() || stride <= 1 { return data.to_vec(); }
    let mut out = vec![0u8; data.len()];
    let count = data.len() / stride;
    let end = count * stride;
    for i in 0..count {
        for j in 0..stride {
            out[j * count + i] = data[i * stride + j];
        }
    }
    for i in end..data.len() { out[i] = data[i]; }
    out
}

fn unshuffle_bytes(data: &[u8], stride: usize) -> Vec<u8> {
    if data.is_empty() || stride <= 1 { return data.to_vec(); }
    let mut out = vec![0u8; data.len()];
    let count = data.len() / stride;
    let end = count * stride;
    for i in 0..count {
        for j in 0..stride {
            out[i * stride + j] = data[j * count + i];
        }
    }
    for i in end..data.len() { out[i] = data[i]; }
    out
}

fn bswap_u32_array(data: &mut [u8]) {
    for chunk in data.chunks_exact_mut(4) {
        let val = LittleEndian::read_u32(chunk);
        LittleEndian::write_u32(chunk, val.swap_bytes());
    }
}

fn bswap_u64_array(data: &mut [u8]) {
    for chunk in data.chunks_exact_mut(8) {
        let val = LittleEndian::read_u64(chunk);
        LittleEndian::write_u64(chunk, val.swap_bytes());
    }
}

fn bswap_cat(data: &mut [u8], cat: usize) {
    match cat {
        c if c == CAT_S4 as usize => bswap_u32_array(data),
        c if c == CAT_JT4 as usize => bswap_u32_array(data),
        c if c == CAT_S8 as usize => bswap_u64_array(data),
        c if c == CAT_RELR8 as usize => bswap_u64_array(data),
        c if c == CAT_S16 as usize => {
            for chunk in data.chunks_exact_mut(16) {
                let v1 = LittleEndian::read_u64(&chunk[0..8]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                LittleEndian::write_u64(&mut chunk[0..8], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
            }
        },
        c if c == CAT_REL16 as usize => {
            for chunk in data.chunks_exact_mut(16) {
                let v1 = LittleEndian::read_u64(&chunk[0..8]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                LittleEndian::write_u64(&mut chunk[0..8], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
            }
        },
        c if c == CAT_DYNAMIC16 as usize => {
            for chunk in data.chunks_exact_mut(16) {
                let v1 = LittleEndian::read_u64(&chunk[0..8]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                LittleEndian::write_u64(&mut chunk[0..8], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
            }
        },
        c if c == CAT_S24 as usize => {
            for chunk in data.chunks_exact_mut(24) {
                let v1 = LittleEndian::read_u64(&chunk[0..8]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                let v3 = LittleEndian::read_u64(&chunk[16..24]);
                LittleEndian::write_u64(&mut chunk[0..8], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
                LittleEndian::write_u64(&mut chunk[16..24], v3.swap_bytes());
            }
        },
        c if c == CAT_RELA24 as usize => {
            for chunk in data.chunks_exact_mut(24) {
                let v1 = LittleEndian::read_u64(&chunk[0..8]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                let v3 = LittleEndian::read_u64(&chunk[16..24]);
                LittleEndian::write_u64(&mut chunk[0..8], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
                LittleEndian::write_u64(&mut chunk[16..24], v3.swap_bytes());
            }
        },
        c if c == CAT_SYM24 as usize => {
            for chunk in data.chunks_exact_mut(24) {
                let v1 = LittleEndian::read_u32(&chunk[0..4]);
                let v2 = LittleEndian::read_u64(&chunk[8..16]);
                let v3 = LittleEndian::read_u64(&chunk[16..24]);
                LittleEndian::write_u32(&mut chunk[0..4], v1.swap_bytes());
                LittleEndian::write_u64(&mut chunk[8..16], v2.swap_bytes());
                LittleEndian::write_u64(&mut chunk[16..24], v3.swap_bytes());
            }
        },
        _ => {}
    }
}

// ---------------- Struct Delta Typed Processing ----------------

fn process_elf_tables(file_data: &[u8], is_compress: bool) -> Vec<u8> {
    let mut out = file_data.to_vec();
    let obj = match object::File::parse(file_data) {
        Ok(o) => o,
        Err(_) => return out,
    };
    if obj.architecture() != Architecture::X86_64 || !obj.is_little_endian() || !obj.is_64() {
        return out;
    }

    for sec in obj.sections() {
        let name = sec.name().unwrap_or("");
        let (file_off, size) = match sec.file_range() {
            Some(r) => r,
            None => continue,
        };
        let file_off = file_off as usize;
        let size = size as usize;
        if file_off + size > out.len() { continue; }
        
        let slice = &mut out[file_off .. file_off + size];

        if name.starts_with(".rela") {
            transform_rela24(slice, is_compress);
        } else if name.starts_with(".rel") && !name.starts_with(".relr") {
            transform_rel16(slice, is_compress);
        } else if name == ".dynsym" || name == ".symtab" {
            transform_sym24(slice, is_compress);
        } else if name.starts_with(".relr") {
            transform_relr8(slice, is_compress);
        } else if name == ".dynamic" {
            transform_dynamic16(slice, is_compress);
        }
    }
    out
}

fn transform_rela24(buf: &mut [u8], is_compress: bool) {
    if buf.len() % 24 != 0 { return; }
    let n = buf.len() / 24;
    let mut prev_off: u64 = 0;
    let mut prev_sym: u32 = 0;
    let mut prev_add: i64 = 0;

    for i in 0..n {
        let p = i * 24;
        let off = LittleEndian::read_u64(&buf[p..p + 8]);
        let info = LittleEndian::read_u64(&buf[p + 8..p + 16]);
        let add = LittleEndian::read_i64(&buf[p + 16..p + 24]);
        let sym = (info >> 32) as u32;
        let typ = (info & 0xFFFF_FFFF) as u32;

        if is_compress {
            let off_d = if i == 0 { off } else { off.wrapping_sub(prev_off) };
            let sym_d = if i == 0 { sym } else { sym.wrapping_sub(prev_sym) };
            let add_d = if i == 0 { add } else { add.wrapping_sub(prev_add) };

            LittleEndian::write_u64(&mut buf[p..p + 8], off_d);
            let info2 = ((sym_d as u64) << 32) | (typ as u64);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], info2);
            let zz_add = ((add_d << 1) ^ (add_d >> 63)) as u64;
            LittleEndian::write_u64(&mut buf[p + 16..p + 24], zz_add);

            prev_off = off;
            prev_sym = sym;
            prev_add = add;
        } else {
            let off_v = if i == 0 { off } else { prev_off.wrapping_add(off) };
            let sym_d = (info >> 32) as u32;
            let sym_v = if i == 0 { sym_d } else { prev_sym.wrapping_add(sym_d) };
            let u = add as u64; 
            let add_d = ((u >> 1) as i64) ^ (-((u & 1) as i64));
            let add_v = if i == 0 { add_d } else { prev_add.wrapping_add(add_d) };

            LittleEndian::write_u64(&mut buf[p..p + 8], off_v);
            let info2 = ((sym_v as u64) << 32) | (typ as u64);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], info2);
            LittleEndian::write_i64(&mut buf[p + 16..p + 24], add_v);

            prev_off = off_v;
            prev_sym = sym_v;
            prev_add = add_v;
        }
    }
}

fn transform_rel16(buf: &mut [u8], is_compress: bool) {
    if buf.len() % 16 != 0 { return; }
    let n = buf.len() / 16;
    let mut prev_off: u64 = 0;
    let mut prev_sym: u32 = 0;

    for i in 0..n {
        let p = i * 16;
        let off = LittleEndian::read_u64(&buf[p..p + 8]);
        let info = LittleEndian::read_u64(&buf[p + 8..p + 16]);
        let sym = (info >> 32) as u32;
        let typ = (info & 0xFFFF_FFFF) as u32;

        if is_compress {
            let off_d = if i == 0 { off } else { off.wrapping_sub(prev_off) };
            let sym_d = if i == 0 { sym } else { sym.wrapping_sub(prev_sym) };

            LittleEndian::write_u64(&mut buf[p..p + 8], off_d);
            let info2 = ((sym_d as u64) << 32) | (typ as u64);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], info2);

            prev_off = off;
            prev_sym = sym;
        } else {
            let off_v = if i == 0 { off } else { prev_off.wrapping_add(off) };
            let sym_d = (info >> 32) as u32;
            let sym_v = if i == 0 { sym_d } else { prev_sym.wrapping_add(sym_d) };

            LittleEndian::write_u64(&mut buf[p..p + 8], off_v);
            let info2 = ((sym_v as u64) << 32) | (typ as u64);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], info2);

            prev_off = off_v;
            prev_sym = sym_v;
        }
    }
}

fn transform_sym24(buf: &mut [u8], is_compress: bool) {
    if buf.len() % 24 != 0 { return; }
    let n = buf.len() / 24;
    let mut prev_name: u32 = 0;
    let mut prev_val: u64 = 0;
    let mut prev_sz: u64 = 0;

    for i in 0..n {
        let p = i * 24;
        let name = LittleEndian::read_u32(&buf[p..p + 4]);
        let val = LittleEndian::read_u64(&buf[p + 8..p + 16]);
        let sz = LittleEndian::read_u64(&buf[p + 16..p + 24]);

        if is_compress {
            let name_d = if i == 0 { name } else { name.wrapping_sub(prev_name) };
            let val_d  = if i == 0 { val } else { val.wrapping_sub(prev_val) };
            let sz_d   = if i == 0 { sz } else { sz.wrapping_sub(prev_sz) };

            LittleEndian::write_u32(&mut buf[p..p + 4], name_d);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], val_d);
            LittleEndian::write_u64(&mut buf[p + 16..p + 24], sz_d);

            prev_name = name;
            prev_val = val;
            prev_sz = sz;
        } else {
            let name_v = if i == 0 { name } else { prev_name.wrapping_add(name) };
            let val_v  = if i == 0 { val } else { prev_val.wrapping_add(val) };
            let sz_v   = if i == 0 { sz } else { prev_sz.wrapping_add(sz) };

            LittleEndian::write_u32(&mut buf[p..p + 4], name_v);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], val_v);
            LittleEndian::write_u64(&mut buf[p + 16..p + 24], sz_v);

            prev_name = name_v;
            prev_val = val_v;
            prev_sz = sz_v;
        }
    }
}

fn transform_relr8(buf: &mut [u8], is_compress: bool) {
    if buf.len() % 8 != 0 { return; }
    let n = buf.len() / 8;
    let mut prev_base = 0u64;

    for i in 0..n {
        let p = i * 8;
        let val = LittleEndian::read_u64(&buf[p..p + 8]);

        if (val & 1) == 0 { 
            if is_compress {
                let delta = if i == 0 { val } else { val.wrapping_sub(prev_base) };
                LittleEndian::write_u64(&mut buf[p..p + 8], delta);
                prev_base = val;
            } else {
                let base = if i == 0 { val } else { prev_base.wrapping_add(val) };
                LittleEndian::write_u64(&mut buf[p..p + 8], base);
                prev_base = base;
            }
        }
    }
}

fn transform_dynamic16(buf: &mut [u8], is_compress: bool) {
    if buf.len() % 16 != 0 { return; }
    let n = buf.len() / 16;
    let mut prev_tag: u64 = 0;
    let mut prev_val: u64 = 0;

    for i in 0..n {
        let p = i * 16;
        let tag = LittleEndian::read_u64(&buf[p..p + 8]);
        let val = LittleEndian::read_u64(&buf[p + 8..p + 16]);

        if is_compress {
            let tag_d = if i == 0 { tag } else { tag.wrapping_sub(prev_tag) };
            let val_d = if i == 0 { val } else { val.wrapping_sub(prev_val) };

            LittleEndian::write_u64(&mut buf[p..p + 8], tag_d);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], val_d);

            prev_tag = tag;
            prev_val = val;
        } else {
            let tag_v = if i == 0 { tag } else { prev_tag.wrapping_add(tag) };
            let val_v = if i == 0 { val } else { prev_val.wrapping_add(val) };

            LittleEndian::write_u64(&mut buf[p..p + 8], tag_v);
            LittleEndian::write_u64(&mut buf[p + 8..p + 16], val_v);

            prev_tag = tag_v;
            prev_val = val_v;
        }
    }
}

// ---------------- Jump Table Discovery ----------------
#[derive(Debug, Clone, Copy)]
struct JumpTable {
    fo: usize,
    count: usize,
}

fn process_jump_tables(file_data: &[u8], is_compress: bool, use_be: bool, jt_meta_in: Option<&[u8]>) -> Result<(Vec<u8>, Vec<u8>, Vec<JumpTable>), String> {
    let mut out = file_data.to_vec();
    let obj = match object::File::parse(file_data) {
        Ok(o) => o,
        Err(_) => return Ok((out, Vec::new(), Vec::new())),
    };

    if obj.architecture() != Architecture::X86_64 { return Ok((out, Vec::new(), Vec::new())); }

    let mut image_base = u64::MAX;
    for sec in obj.segments() {
        if sec.address() < image_base { image_base = sec.address(); }
    }
    if image_base == u64::MAX { image_base = 0; }


    let mut text_va = 0u64;
    let mut text_size = 0u64;
    for sec in obj.sections() {
        if sec.name().unwrap_or("") == ".text" {
            text_va = sec.address();
            text_size = sec.size();
            break;
        }
    }

    if text_size == 0 { return Ok((out, Vec::new(), Vec::new())); }

    let mut tables = Vec::new();

    if is_compress {
        for sec in obj.sections() {
            let name = sec.name().unwrap_or("");
            if name != ".rodata" && name != ".data.rel.ro" { continue; }
            
            let (file_off, sec_size) = match sec.file_range() { Some(r) => r, None => continue };
            let data = match sec.data() { Ok(d) => d, Err(_) => continue };
            if data.len() != sec_size as usize { continue; }
            
            let mut current_run_start = 0;
            let mut current_run_len = 0;
            
            for i in (0..data.len().saturating_sub(3)).step_by(4) {
                let val = LittleEndian::read_i32(&data[i..i+4]);
                let entry_va = sec.address() + i as u64;
                let target_va = entry_va.wrapping_add(val as i64 as u64);
                
                if target_va >= text_va && target_va < text_va + text_size {
                    if current_run_len == 0 { current_run_start = file_off as usize + i; }
                    current_run_len += 1;
                } else {
                    if current_run_len >= 4 { tables.push(JumpTable { fo: current_run_start, count: current_run_len }); }
                    current_run_len = 0;
                }
            }
            if current_run_len >= 4 { tables.push(JumpTable { fo: current_run_start, count: current_run_len }); }
        }
    } else {
        let meta = jt_meta_in.unwrap_or(&[]);
        let mut pos = 0;
        let num_tables = match read_varint(meta, &mut pos) {
            Ok(v) => v as usize,
            Err(_) => return Ok((out, Vec::new(), Vec::new())), 
        };
        
        let mut prev_fo = 0;
        for _ in 0..num_tables {
            let delta_fo = read_varint(meta, &mut pos)? as usize;
            let count = read_varint(meta, &mut pos)? as usize;
            let fo = prev_fo + delta_fo;
            prev_fo = fo;
            tables.push(JumpTable { fo, count });
        }
    }

    let mut meta_out = Vec::new();
    if is_compress {
        write_varint(&mut meta_out, tables.len() as u64);
        let mut prev_fo = 0;
        for t in &tables {
            write_varint(&mut meta_out, (t.fo - prev_fo) as u64);
            write_varint(&mut meta_out, t.count as u64);
            prev_fo = t.fo;
        }
    }

    let file_to_va = |offset: u64| -> Option<u64> {
        for sec in obj.sections() {
            if let Some((fo, size)) = sec.file_range() {
                if offset >= fo && offset < fo + size { return Some(sec.address() + (offset - fo)); }
            }
        }
        None
    };

    for t in &tables {
        for i in 0..t.count {
            let p = t.fo + (i * 4);
            let entry_va = file_to_va(p as u64).unwrap_or(0);
            
            if is_compress {
                let val = LittleEndian::read_i32(&out[p..p+4]);
                let target_va = entry_va.wrapping_add(val as i64 as u64).wrapping_sub(image_base) as u32;
                if use_be { out[p..p+4].copy_from_slice(&target_va.to_be_bytes()); } 
                else { out[p..p+4].copy_from_slice(&target_va.to_le_bytes()); }
            } else {
                let target_va = if use_be { u32::from_be_bytes(out[p..p+4].try_into().unwrap()) } 
                else { LittleEndian::read_u32(&out[p..p+4]) };
                let orig_rel = (target_va as u64).wrapping_add(image_base).wrapping_sub(entry_va) as u32;
                LittleEndian::write_u32(&mut out[p..p+4], orig_rel);
            }
        }
    }

    Ok((out, meta_out, tables))
}

// ---------------- EH Frame PC-Rel Normalization ----------------


fn eh_pe_fixed_size(enc: u8, ptr_size: usize) -> Option<usize> {
    if enc == 0xFF { return Some(0); } // DW_EH_PE_omit
    match enc & 0x0F {
        0x00 => Some(ptr_size),      
        0x02 | 0x0A => Some(2),      
        0x03 | 0x0B => Some(4),      
        0x04 | 0x0C => Some(8),      
        _ => None,                   
    }
}

#[derive(Debug, Clone, Copy)]
struct EhPatch {
    fo: usize,
    field_va: u64,
}

fn process_eh_frame_hdr(file_data: &[u8], is_compress: bool, use_be: bool) -> Vec<u8> {
    let mut out = file_data.to_vec();
    let obj = match object::File::parse(file_data) {
        Ok(o) => o,
        Err(_) => return out,
    };
    
    let mut image_base = u64::MAX;
    for sec in obj.segments() {
        if sec.address() < image_base { image_base = sec.address(); }
    }
    if image_base == u64::MAX { image_base = 0; }

    let mut patches = Vec::new();

    for sec in obj.sections() {
        if sec.name().unwrap_or("") != ".eh_frame_hdr" { continue; }
        
        let (file_off, sec_size) = match sec.file_range() { Some(r) => r, None => continue };
        let file_off = file_off as usize;
        let data = match sec.data() { Ok(d) => d, Err(_) => continue };
        if data.len() != sec_size as usize || data.len() < 8 { continue; }
        
        let version = data[0];
        let eh_frame_ptr_enc = data[1];
        let fde_count_enc = data[2];
        let table_enc = data[3];

        if version != 1 { continue; }
        if table_enc != 0x1b && table_enc != 0x3b { continue; }

        let mut pos = 4;
        let skip_sz = match eh_pe_fixed_size(eh_frame_ptr_enc, 8) {
            Some(sz) => sz,
            None => continue,
        };
        pos += skip_sz;
        
        let fde_count_sz = match eh_pe_fixed_size(fde_count_enc, 8) {
            Some(sz) => sz,
            None => continue,
        };
        
        if fde_count_sz == 4 {
            if pos + 4 > data.len() { continue; }
            let fde_count = LittleEndian::read_u32(&data[pos..pos+4]) as usize;
            pos += 4;
            
            let table_bytes = fde_count * 8;
            if pos + table_bytes <= data.len() {
                for i in 0..(fde_count * 2) {
                    let field_fo = file_off + pos + (i * 4);
                    let field_va = sec.address() + (pos as u64) + (i as u64 * 4);
                    let base_va = if table_enc == 0x1b { field_va } else { sec.address() };
                    patches.push(EhPatch { fo: field_fo, field_va: base_va });
                }
            }
        }
    }

    for p in patches {
        if is_compress {
            let cur_rel = LittleEndian::read_i32(&out[p.fo..p.fo + 4]);
            let mut abs_va = p.field_va.wrapping_add(cur_rel as i64 as u64);
            if abs_va >= image_base { abs_va -= image_base; }
            if abs_va > u32::MAX as u64 { continue; }
            let abs_va32 = abs_va as u32;
            if use_be { out[p.fo..p.fo + 4].copy_from_slice(&abs_va32.to_be_bytes()); } 
            else { out[p.fo..p.fo + 4].copy_from_slice(&abs_va32.to_le_bytes()); }
        } else {
            let abs_va32 = if use_be { u32::from_be_bytes(out[p.fo..p.fo + 4].try_into().unwrap()) } 
            else { LittleEndian::read_u32(&out[p.fo..p.fo + 4]) };
            let orig_rel = (abs_va32 as u64).wrapping_add(image_base).wrapping_sub(p.field_va) as u32;
            LittleEndian::write_u32(&mut out[p.fo..p.fo + 4], orig_rel);
        }
    }

    out
}

// ---------------- USASE Patching ----------------

#[derive(Debug, Clone, Copy)]
struct Patch {
    fo: usize,
    next_ip: u32,
}

fn process_binary(file_data: &[u8], is_compress: bool, use_be: bool) -> Vec<u8> {
    let mut skel = file_data.to_vec();
    let obj = match object::File::parse(file_data) { Ok(o) => o, Err(_) => return skel };
    let mut image_base = u64::MAX;
    for sec in obj.segments() {
        if sec.address() < image_base { image_base = sec.address(); }
    }
    if image_base == u64::MAX { image_base = 0; }
    if obj.architecture() != Architecture::X86_64 || !obj.is_little_endian() || !obj.is_64() { return skel; }

    let mut patches: Vec<Patch> = Vec::new();

    for sec in obj.sections() {
        if sec.kind() != SectionKind::Text { continue; }
        let (file_off, file_size) = match sec.file_range() { Some(r) => r, None => continue };
        let file_off = file_off as usize;
        let file_size = file_size as usize;
        let data = match sec.data() { Ok(d) => d, Err(_) => continue };

        if data.len() != file_size { continue; }
        if file_off + data.len() > skel.len() { continue; }

        let va = sec.address();
        let mut decoder = Decoder::with_ip(64, data, va, DecoderOptions::NONE);

        while decoder.can_decode() {
            let inst = decoder.decode();
            let inst_ip = inst.ip();
            let inst_len = inst.len();
            let next_ip = inst_ip.wrapping_add(inst_len as u64) as u32;

            let off_in_sec = (inst_ip - va) as usize;
            if off_in_sec + inst_len > data.len() { break; }
            let inst_fo = file_off + off_in_sec;

            let co = decoder.get_constant_offsets(&inst);

            if inst.is_ip_rel_memory_operand() && co.has_displacement() && co.displacement_size() == 4 {
                let fo = inst_fo + co.displacement_offset();
                if fo + 4 <= skel.len() { patches.push(Patch { fo, next_ip }); }
            }

            if (inst.is_call_near() || inst.is_jmp_near() || inst.is_jcc_short_or_near()) && co.has_immediate() && co.immediate_size() == 4 {
                let fo = inst_fo + co.immediate_offset();
                if fo + 4 <= skel.len() { patches.push(Patch { fo, next_ip }); }
            }
        }
    }

    for p in &patches {
        if is_compress {
            let cur = LittleEndian::read_u32(&skel[p.fo..p.fo + 4]);
            let dest = cur.wrapping_add(p.next_ip);
            let norm = dest.wrapping_sub(image_base as u32);
            if use_be { skel[p.fo..p.fo + 4].copy_from_slice(&norm.to_be_bytes()); } 
            else { skel[p.fo..p.fo + 4].copy_from_slice(&norm.to_le_bytes()); }
        } else {
            let norm = if use_be { u32::from_be_bytes(skel[p.fo..p.fo + 4].try_into().unwrap()) } 
            else { LittleEndian::read_u32(&skel[p.fo..p.fo + 4]) };
            let dest = norm.wrapping_add(image_base as u32);
            let orig = dest.wrapping_sub(p.next_ip);
            LittleEndian::write_u32(&mut skel[p.fo..p.fo + 4], orig);
        }
    }

    skel
}

// ---------------- Routing ----------------

fn split_streams(file_data: &[u8], jump_tables: &[JumpTable]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut labels = vec![CAT_OTHER; file_data.len()];
    let ptr_prefixes = [".got", ".got.plt", ".data.rel.ro", ".init_array", ".fini_array", ".plt.got"];

    if let Ok(obj) = object::File::parse(file_data) {
        for sec in obj.sections() {
            let (fo, size) = match sec.file_range() { Some(r) => r, None => continue };
            let fo = fo as usize;
            let size = size as usize;
            if fo + size > file_data.len() { continue; }

            let mut cat = CAT_OTHER;
            let name = sec.name().unwrap_or("");

            if sec.kind() == SectionKind::Text {
                cat = CAT_CODE;
            } else if name == ".strtab" || name == ".dynstr" || name.contains("str") {
                cat = CAT_STR;
            } else if name.contains("eh_frame") || name.contains("gcc_except") {
                cat = CAT_EH;
            } else if name.starts_with(".relr") {
                cat = CAT_RELR8;
            } else if name.starts_with(".rela") {
                cat = CAT_RELA24; 
            } else if name == ".symtab" || name == ".dynsym" {
                cat = CAT_SYM24;
            } else if name.starts_with(".rel") {
                cat = CAT_REL16; 
            } else if name == ".dynamic" {
                cat = CAT_DYNAMIC16; 
            } else if name.contains("cst16") {
                cat = CAT_S16;
            } else if name == ".gnu.version" {
                cat = CAT_S2;
            } else if ptr_prefixes.iter().any(|p| name.starts_with(p)) || name.contains("array") || name.contains("cst8") {
                cat = CAT_S8; 
            } else if name.contains("hash") || name.contains("cst4") {
                cat = CAT_S4; 
            }

            for i in fo..fo + size { labels[i] = cat; }
        }
    }

    for t in jump_tables {
        for i in t.fo .. t.fo + (t.count * 4) {
            if i < labels.len() { labels[i] = CAT_JT4; }
        }
    }
    let mut runs = Vec::new();
    if !labels.is_empty() {
        let mut cur_cat = labels[0];
        let mut count = 1u64;
        for &cat in &labels[1..] {
            if cat == cur_cat { count += 1; } 
            else {
                write_varint(&mut runs, (count << 4) | (cur_cat as u64));
                cur_cat = cat;
                count = 1;
            }
        }
        write_varint(&mut runs, (count << 4) | (cur_cat as u64));
    }

    let mut streams = vec![Vec::new(); CAT_COUNT];
    for (i, &cat) in labels.iter().enumerate() { streams[cat as usize].push(file_data[i]); }
    (runs, streams)
}

fn compress_with_mode(file_data: &[u8], use_be: bool) -> Vec<u8> {
    let skel = process_binary(file_data, true, use_be);
    let skel = process_eh_frame_hdr(&skel, true, use_be);
    let (skel, jt_meta, jump_tables) = process_jump_tables(&skel, true, use_be, None).unwrap();
    let skel = process_elf_tables(&skel, true);
    
    let (runs, mut streams) = split_streams(&skel, &jump_tables);

    let preset = 9 | PRESET_EXTREME;
    let strides = [
        (CAT_S2, 2usize), (CAT_S4, 4usize), (CAT_S8, 8usize), (CAT_RELR8, 8usize),
        (CAT_S16, 16usize), (CAT_REL16, 16usize), (CAT_DYNAMIC16, 16usize), 
        (CAT_S24, 24usize), (CAT_RELA24, 24usize), (CAT_SYM24, 24usize),
        (CAT_JT4, 4usize)
    ];
    for (cat, stride) in strides {
        let s = &mut streams[cat as usize];
        bswap_cat(s, cat as usize);
        *s = shuffle_bytes(s, stride);
    }

    let compressed_streams: Vec<Vec<u8>> = streams.par_iter().enumerate().map(|(cat, s)| {
        if s.is_empty() { return vec![]; }
        let pb = choose_pb(cat);
        let dict = choose_dict_size(s.len());
        
        if cat != CAT_CODE as usize && cat != CAT_EH as usize && cat != CAT_OTHER as usize {
            let mut opts_lc3 = LzmaOptions::new_preset(preset).unwrap();
            opts_lc3.position_bits(pb).dict_size(dict).literal_context_bits(3);
            let mut f3 = Filters::new(); f3.lzma2(&opts_lc3);
            let mut enc3 = xz2::write::XzEncoder::new_stream(Vec::new(), Stream::new_stream_encoder(&f3, XZ_CHECK).unwrap());
            enc3.write_all(s).unwrap();
            let mut c_best = enc3.finish().unwrap();

            let mut opts_lc0 = LzmaOptions::new_preset(preset).unwrap();
            opts_lc0.position_bits(pb).dict_size(dict).literal_context_bits(0);
            let mut f0 = Filters::new(); f0.lzma2(&opts_lc0);
            let mut enc0 = xz2::write::XzEncoder::new_stream(Vec::new(), Stream::new_stream_encoder(&f0, XZ_CHECK).unwrap());
            enc0.write_all(s).unwrap();
            let c0 = enc0.finish().unwrap();
            if c0.len() < c_best.len() { c_best = c0; }

            c_best
        } else {
            compress_xz_tuned(s, preset, pb, dict)
        }
    }).collect();

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);

    let mut orig_len_buf = [0u8; 8];
    LittleEndian::write_u64(&mut orig_len_buf, file_data.len() as u64);
    out.extend_from_slice(&orig_len_buf);

    out.push(if use_be { 1 } else { 0 });

    write_varint(&mut out, runs.len() as u64);
    out.extend_from_slice(&runs);

    for cs in compressed_streams {
        write_varint(&mut out, cs.len() as u64);
        out.extend_from_slice(&cs);
    }
    
    write_varint(&mut out, jt_meta.len() as u64);
    out.extend_from_slice(&jt_meta);

    out
}

fn compress(file_data: &[u8]) -> Vec<u8> {
    let (c_le, c_be) = rayon::join(|| compress_with_mode(file_data, false), || compress_with_mode(file_data, true));
    if c_be.len() < c_le.len() { c_be } else { c_le }
}

fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 13 { return Err("input too short".into()); }
    if &data[0..4] != MAGIC { return Err("bad magic".into()); }
    let orig_len = LittleEndian::read_u64(&data[4..12]) as usize;
    let mut pos = 12usize;
    let use_be = data[pos] == 1;
    pos += 1;

    let runs_len = read_varint(data, &mut pos)? as usize;
    if pos + runs_len > data.len() { return Err("runs block out of range".into()); }
    let runs_data = &data[pos..pos + runs_len];
    pos += runs_len;

    let mut compressed_streams = Vec::with_capacity(CAT_COUNT);
    for _ in 0..CAT_COUNT {
        let cs_len = read_varint(data, &mut pos)? as usize;
        if pos + cs_len > data.len() { return Err("stream block out of range".into()); }
        compressed_streams.push(&data[pos..pos + cs_len]);
        pos += cs_len;
    }
    
    let jt_meta_len = read_varint(data, &mut pos)? as usize;
    if pos + jt_meta_len > data.len() { return Err("jt block out of range".into()); }
    let jt_meta = &data[pos..pos + jt_meta_len];

    let mut decompressed_streams: Vec<Vec<u8>> = compressed_streams.par_iter()
        .map(|cs| decompress_xz(cs)).collect::<Result<Vec<_>, _>>()?;

    let strides = [
        (CAT_S2, 2usize), (CAT_S4, 4usize), (CAT_S8, 8usize), (CAT_RELR8, 8usize),
        (CAT_S16, 16usize), (CAT_REL16, 16usize), (CAT_DYNAMIC16, 16usize), 
        (CAT_S24, 24usize), (CAT_RELA24, 24usize), (CAT_SYM24, 24usize),
        (CAT_JT4, 4usize)
    ];
    for (cat, stride) in strides {
        let s = &mut decompressed_streams[cat as usize];
        *s = unshuffle_bytes(s, stride);
        bswap_cat(s, cat as usize);
    }

    let mut skel = vec![0u8; orig_len];
    let mut cursors = vec![0usize; CAT_COUNT];
    let mut run_pos = 0usize;
    let mut skel_pos = 0usize;

    while run_pos < runs_data.len() {
        let val = read_varint(runs_data, &mut run_pos)?;
        let cat = (val & 15) as usize;
        let count = (val >> 4) as usize;

        if cat >= CAT_COUNT { return Err("bad category".into()); }
        if skel_pos + count > skel.len() { return Err("runs exceed output length".into()); }
        let c = cursors[cat];
        if c + count > decompressed_streams[cat].len() { return Err("stream underflow while reconstructing".into()); }

        skel[skel_pos..skel_pos + count].copy_from_slice(&decompressed_streams[cat][c..c + count]);
        cursors[cat] += count;
        skel_pos += count;
    }

    for cat in 0..CAT_COUNT {
        if cursors[cat] != decompressed_streams[cat].len() {
            return Err(format!("stream {} has extra bytes: used {} / {}", cat, cursors[cat], decompressed_streams[cat].len()));
        }
    }

    let skel = process_elf_tables(&skel, false);
    let (skel, _, _) = process_jump_tables(&skel, false, use_be, Some(jt_meta))?;
    let skel = process_eh_frame_hdr(&skel, false, use_be);
    Ok(process_binary(&skel, false, use_be))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 { std::process::exit(2); }
    let cmd = &args[1];
    let path = &args[2];

    match cmd.as_str() {
        "compare" => {
            let data = fs::read(path).unwrap();
            let start = Instant::now();
            let compressed = compress(&data);
            let c_time = start.elapsed();
            let start = Instant::now();
            let decompressed = decompress(&compressed).unwrap();
            let d_time = start.elapsed();
            assert_eq!(data, decompressed, "Mismatch!");

            println!("====== FESH USASE vE (EH_FRAME_HDR + Jump Tables + LC0 MoE) ======");
            println!("Target File: {}", path);
            println!("Input:       {} bytes", data.len());
            let ratio = (compressed.len() as f64 / data.len() as f64) * 100.0;
            println!("FESH (Rust): {} bytes ({:.2}%)", compressed.len(), ratio);
            println!("Comp Time:   {:?}", c_time);
            println!("Decomp Time: {:?}", d_time);
        }
        "compress" => {
            let data = fs::read(path).unwrap();
            fs::write(&args[3], compress(&data)).unwrap();
        }
        "decompress" => {
            let data = fs::read(path).unwrap();
            fs::write(&args[3], decompress(&data).unwrap()).unwrap();
        }
        _ => { std::process::exit(2); }
    }
}
