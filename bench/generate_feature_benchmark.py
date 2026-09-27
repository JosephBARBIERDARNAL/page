import zlib
from pathlib import Path

OUTPUT = Path(__file__).with_name("document-feature-heavy.pdf")
FONT_SOURCE = (
    Path(__file__).parents[1] / "crates/page_validation/tests/fixtures/fonts/usyr.pfa"
)
PAGE_COUNT = 10
IMAGE_WIDTH = 640
IMAGE_HEIGHT = 480
STRUCTURE_ELEMENT_COUNT = 2_048
OPTIONAL_CONTENT_GROUP_COUNT = 64
EMBEDDED_FILE_COUNT = 512


def reserve(objects: list[bytes | None], count: int) -> list[int]:
    first = len(objects)
    objects.extend([None] * count)
    return list(range(first, first + count))


def reference(object_id: int) -> bytes:
    return f"{object_id} 0 R".encode()


def array(values: list[bytes]) -> bytes:
    return b"[" + b" ".join(values) + b"]"


def dictionary(entries: list[tuple[bytes, bytes]]) -> bytes:
    return b"<< " + b" ".join(key + b" " + value for key, value in entries) + b" >>"


def stream(entries: list[tuple[bytes, bytes]], payload: bytes) -> bytes:
    return (
        dictionary(entries + [(b"/Length", str(len(payload)).encode())])
        + b"\nstream\n"
        + payload
        + b"\nendstream"
    )


def image_payload(page_index: int) -> bytes:
    pixels = bytearray()
    for y in range(IMAGE_HEIGHT):
        for x in range(IMAGE_WIDTH):
            seed = (x * 17 + y * 31 + page_index * 47) & 255
            pixels.extend((seed, (seed + x) & 255, (seed + y) & 255))
    return zlib.compress(bytes(pixels), level=6)


def build_pdf() -> bytes:
    objects: list[bytes | None] = [None]
    catalog_id, pages_id, font_file_id, font_descriptor_id, font_id = reserve(
        objects, 5
    )
    struct_root_id = reserve(objects, 1)[0]
    names_id, embedded_names_id, optional_content_id, optional_config_id = reserve(
        objects, 4
    )
    page_ids = reserve(objects, PAGE_COUNT)
    content_ids = reserve(objects, PAGE_COUNT)
    image_ids = reserve(objects, PAGE_COUNT)
    optional_group_ids = reserve(objects, OPTIONAL_CONTENT_GROUP_COUNT)
    structure_ids = reserve(objects, STRUCTURE_ELEMENT_COUNT)
    file_spec_ids = reserve(objects, EMBEDDED_FILE_COUNT)
    embedded_stream_ids = reserve(objects, EMBEDDED_FILE_COUNT)

    font_payload = FONT_SOURCE.read_bytes()
    objects[font_file_id] = stream(
        [(b"/Length1", str(len(font_payload)).encode())],
        font_payload,
    )
    objects[font_descriptor_id] = dictionary(
        [
            (b"/Type", b"/FontDescriptor"),
            (b"/FontName", b"/StandardSymL"),
            (b"/Flags", b"4"),
            (b"/FontBBox", b"[-180 -293 1090 1010]"),
            (b"/ItalicAngle", b"0"),
            (b"/Ascent", b"1010"),
            (b"/Descent", b"-293"),
            (b"/CapHeight", b"710"),
            (b"/StemV", b"80"),
            (b"/FontFile", reference(font_file_id)),
        ]
    )
    objects[font_id] = dictionary(
        [
            (b"/Type", b"/Font"),
            (b"/Subtype", b"/Type1"),
            (b"/BaseFont", b"/StandardSymL"),
            (b"/Encoding", b"/WinAnsiEncoding"),
            (b"/FontDescriptor", reference(font_descriptor_id)),
        ]
    )

    for index, (page_id, content_id, image_id) in enumerate(
        zip(page_ids, content_ids, image_ids)
    ):
        image = image_payload(index)
        objects[image_id] = stream(
            [
                (b"/Type", b"/XObject"),
                (b"/Subtype", b"/Image"),
                (b"/Width", str(IMAGE_WIDTH).encode()),
                (b"/Height", str(IMAGE_HEIGHT).encode()),
                (b"/ColorSpace", b"/DeviceRGB"),
                (b"/BitsPerComponent", b"8"),
                (b"/Filter", b"/FlateDecode"),
            ],
            image,
        )
        content = (
            f"BT /F1 18 Tf 72 740 Td (Feature benchmark page {index + 1}) Tj ET\n"
            f"q 540 0 0 360 36 320 cm /Im{index} Do Q"
        ).encode()
        objects[content_id] = stream([], content)
        objects[page_id] = dictionary(
            [
                (b"/Type", b"/Page"),
                (b"/Parent", reference(pages_id)),
                (b"/MediaBox", b"[0 0 612 792]"),
                (
                    b"/Resources",
                    dictionary(
                        [
                            (b"/Font", dictionary([(b"/F1", reference(font_id))])),
                            (
                                b"/XObject",
                                dictionary(
                                    [(f"/Im{index}".encode(), reference(image_id))]
                                ),
                            ),
                        ]
                    ),
                ),
                (b"/Contents", reference(content_id)),
                (b"/StructParents", str(index).encode()),
            ]
        )
    objects[pages_id] = dictionary(
        [
            (b"/Type", b"/Pages"),
            (b"/Kids", array([reference(object_id) for object_id in page_ids])),
            (b"/Count", str(PAGE_COUNT).encode()),
        ]
    )

    for index, object_id in enumerate(optional_group_ids):
        objects[object_id] = dictionary(
            [
                (b"/Type", b"/OCG"),
                (b"/Name", f"(optional-group-{index:03})".encode()),
            ]
        )
    optional_groups = array([reference(object_id) for object_id in optional_group_ids])
    objects[optional_config_id] = dictionary(
        [
            (b"/Name", b"(feature-benchmark)"),
            (b"/OCGs", optional_groups),
            (b"/ON", optional_groups),
            (b"/Order", optional_groups),
        ]
    )
    objects[optional_content_id] = dictionary(
        [
            (b"/OCGs", optional_groups),
            (b"/D", reference(optional_config_id)),
            (b"/Configs", array([reference(optional_config_id)])),
        ]
    )

    for index, object_id in enumerate(structure_ids):
        objects[object_id] = dictionary(
            [
                (b"/Type", b"/StructElem"),
                (b"/S", b"/P"),
                (b"/P", reference(struct_root_id)),
                (b"/Pg", reference(page_ids[index % PAGE_COUNT])),
                (b"/K", b"[]"),
            ]
        )
    objects[struct_root_id] = dictionary(
        [
            (b"/Type", b"/StructTreeRoot"),
            (b"/K", array([reference(object_id) for object_id in structure_ids])),
            (b"/Lang", b"(en-US)"),
        ]
    )

    file_names: list[bytes] = []
    embedded_payload = b"feature benchmark payload\n"
    for index, (file_spec_id, stream_id) in enumerate(
        zip(file_spec_ids, embedded_stream_ids)
    ):
        filename = f"feature-{index:04}.bin".encode()
        file_names.extend([b"(" + filename + b")", reference(file_spec_id)])
        payload = (
            b"%PDF-1.7\nnot a complete embedded PDF\n"
            if index < 8
            else embedded_payload
        )
        objects[stream_id] = stream(
            [
                (b"/Type", b"/EmbeddedFile"),
                (b"/Subtype", b"/application#2Foctet-stream"),
            ],
            payload,
        )
        objects[file_spec_id] = dictionary(
            [
                (b"/Type", b"/Filespec"),
                (b"/F", b"(" + filename + b")"),
                (b"/UF", b"(" + filename + b")"),
                (b"/AFRelationship", b"/Data"),
                (
                    b"/EF",
                    dictionary(
                        [(b"/F", reference(stream_id)), (b"/UF", reference(stream_id))]
                    ),
                ),
            ]
        )
    objects[embedded_names_id] = dictionary([(b"/Names", array(file_names))])
    objects[names_id] = dictionary([(b"/EmbeddedFiles", reference(embedded_names_id))])
    objects[catalog_id] = dictionary(
        [
            (b"/Type", b"/Catalog"),
            (b"/Pages", reference(pages_id)),
            (b"/Lang", b"(en-US)"),
            (b"/StructTreeRoot", reference(struct_root_id)),
            (b"/Names", reference(names_id)),
            (b"/OCProperties", reference(optional_content_id)),
            (b"/AF", array([reference(object_id) for object_id in file_spec_ids])),
        ]
    )

    if any(object_data is None for object_data in objects[1:]):
        raise RuntimeError("feature benchmark PDF has unassigned objects")

    header = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"
    output = bytearray(header)
    offsets = [0]
    for object_id, object_data in enumerate(objects[1:], start=1):
        offsets.append(len(output))
        output.extend(f"{object_id} 0 obj\n".encode())
        output.extend(object_data)
        output.extend(b"\nendobj\n")
    xref_offset = len(output)
    output.extend(f"xref\n0 {len(objects)}\n".encode())
    output.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        output.extend(f"{offset:010} 00000 n \n".encode())
    output.extend(b"trailer\n")
    output.extend(
        dictionary(
            [(b"/Size", str(len(objects)).encode()), (b"/Root", reference(catalog_id))]
        )
    )
    output.extend(b"\nstartxref\n")
    output.extend(str(xref_offset).encode())
    output.extend(b"\n%%EOF\n")
    return bytes(output)


if __name__ == "__main__":
    OUTPUT.write_bytes(build_pdf())
    print(f"wrote {OUTPUT} ({OUTPUT.stat().st_size} bytes)")
