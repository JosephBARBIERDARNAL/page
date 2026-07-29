//! A minimal, self-contained SFNT/TrueType byte builder for font fixtures.
//! Depends only on `std`; used by `super`'s font-related fixture builders.

pub fn minimal_truetype() -> Vec<u8> {
    minimal_truetype_with_cmap_count(1)
}

pub fn minimal_truetype_with_cmap_count(cmap_count: u16) -> Vec<u8> {
    minimal_truetype_with_cmap_count_and_mapping(cmap_count, 32)
}

pub fn minimal_truetype_with_cmap_mapping(code: u8) -> Vec<u8> {
    minimal_truetype_with_cmap_count_and_mapping(1, code)
}

pub fn minimal_truetype_with_glyph_count(glyph_count: u16) -> Vec<u8> {
    minimal_truetype_with_cmap_count_and_mapping_and_glyph_count(1, 32, glyph_count)
}

fn minimal_truetype_with_cmap_count_and_mapping(cmap_count: u16, code: u8) -> Vec<u8> {
    minimal_truetype_with_cmap_count_and_mapping_and_glyph_count(cmap_count, code, 2)
}

fn minimal_truetype_with_cmap_count_and_mapping_and_glyph_count(
    cmap_count: u16,
    code: u8,
    glyph_count: u16,
) -> Vec<u8> {
    let mut head = vec![0; 54];
    put_u32(&mut head, 0, 0x0001_0000);
    put_u32(&mut head, 4, 0x0001_0000);
    put_u32(&mut head, 12, 0x5F0F_3CF5);
    put_u16(&mut head, 18, 1000);
    put_i16(&mut head, 40, 500);
    put_i16(&mut head, 42, 700);
    put_u16(&mut head, 46, 8);
    put_i16(&mut head, 48, 2);

    let mut hhea = vec![0; 36];
    put_u32(&mut hhea, 0, 0x0001_0000);
    put_i16(&mut hhea, 4, 800);
    put_i16(&mut hhea, 6, -200);
    put_u16(&mut hhea, 10, 500);
    put_i16(&mut hhea, 18, 1);
    put_u16(&mut hhea, 34, 2);

    let mut maxp = vec![0; 32];
    put_u32(&mut maxp, 0, 0x0001_0000);
    put_u16(&mut maxp, 4, glyph_count);

    let cmap_header_length = 4 + usize::from(cmap_count) * 8;
    let mut cmap = vec![0; cmap_header_length + 262];
    put_u16(&mut cmap, 2, cmap_count);
    for index in 0..usize::from(cmap_count) {
        let record = 4 + index * 8;
        put_u16(&mut cmap, record, 3);
        put_u16(
            &mut cmap,
            record + 2,
            u16::try_from(index + 1).expect("small cmap count"),
        );
        put_u32(
            &mut cmap,
            record + 4,
            u32::try_from(cmap_header_length).expect("small cmap header"),
        );
    }
    put_u16(&mut cmap, cmap_header_length, 0);
    put_u16(&mut cmap, cmap_header_length + 2, 262);
    cmap[cmap_header_length + 6 + usize::from(code)] = 1;

    let family = utf16be("Mai Test");
    let postscript = utf16be("MaiTestFont");
    let mut name = vec![0; 30 + family.len() + postscript.len()];
    put_u16(&mut name, 2, 2);
    put_u16(&mut name, 4, 30);
    put_u16(&mut name, 6, 3);
    put_u16(&mut name, 8, 1);
    put_u16(&mut name, 10, 0x0409);
    put_u16(&mut name, 12, 1);
    put_u16(&mut name, 14, family.len() as u16);
    put_u16(&mut name, 18, 3);
    put_u16(&mut name, 20, 1);
    put_u16(&mut name, 22, 0x0409);
    put_u16(&mut name, 24, 6);
    put_u16(&mut name, 26, postscript.len() as u16);
    put_u16(&mut name, 28, family.len() as u16);
    name[30..30 + family.len()].copy_from_slice(&family);
    name[30 + family.len()..].copy_from_slice(&postscript);

    let mut os2 = vec![0; 78];
    put_u16(&mut os2, 2, 500);
    put_u16(&mut os2, 4, 400);
    put_u16(&mut os2, 6, 5);
    put_u16(&mut os2, 8, 0);
    put_i16(&mut os2, 68, 800);
    put_i16(&mut os2, 70, -200);
    put_u16(&mut os2, 74, 800);
    put_u16(&mut os2, 76, 200);

    let mut post = vec![0; 32];
    put_u32(&mut post, 0, 0x0003_0000);

    let mut hmtx = vec![0; 8];
    put_u16(&mut hmtx, 0, 500);
    put_u16(&mut hmtx, 4, 500);

    let tables = vec![
        (*b"OS/2", os2),
        (*b"cmap", cmap),
        (*b"glyf", vec![0; 4]),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", vec![0; 2 * (usize::from(glyph_count) + 1)]),
        (*b"maxp", maxp),
        (*b"name", name),
        (*b"post", post),
    ];
    build_sfnt(tables)
}

pub fn build_sfnt(tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let table_count = tables.len();
    let mut font = vec![0; 12 + 16 * table_count];
    put_u32(&mut font, 0, 0x0001_0000);
    put_u16(&mut font, 4, table_count as u16);
    put_u16(&mut font, 6, 128);
    put_u16(&mut font, 8, 3);
    put_u16(&mut font, 10, (table_count * 16 - 128) as u16);

    let mut head_offset = None;
    for (index, (tag, data)) in tables.iter().enumerate() {
        while !font.len().is_multiple_of(4) {
            font.push(0);
        }
        let offset = font.len();
        let directory = 12 + index * 16;
        font[directory..directory + 4].copy_from_slice(tag);
        put_u32(&mut font, directory + 4, table_checksum(data));
        put_u32(&mut font, directory + 8, offset as u32);
        put_u32(&mut font, directory + 12, data.len() as u32);
        font.extend_from_slice(data);
        if tag == b"head" {
            head_offset = Some(offset);
        }
    }
    while !font.len().is_multiple_of(4) {
        font.push(0);
    }
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(table_checksum(&font));
    put_u32(&mut font, head_offset.expect("head table") + 8, adjustment);
    font
}

pub fn table_checksum(bytes: &[u8]) -> u32 {
    bytes
        .chunks(4)
        .map(|chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            u32::from_be_bytes(word)
        })
        .fold(0u32, u32::wrapping_add)
}

pub fn utf16be(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

pub fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

pub fn put_i16(bytes: &mut [u8], offset: usize, value: i16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

pub fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
