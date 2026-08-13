# Kaitai Struct schema for the Londo Save format.
#
# filekind reads only the `meta` block and the first `seq` field of this file,
# and only to check that it and londo-save.filekind agree about the extension
# and the magic bytes. Everything else here is Kaitai's business.
meta:
  id: londo_save
  title: Londo Save
  file-extension: londo
  endian: le

doc: |
  A Londo project save. Header, then a table of chunks.

seq:
  - id: magic
    contents: [LNDO, 0x01]
  - id: version_major
    type: u2
  - id: version_minor
    type: u2
  - id: chunk_count
    type: u4
  - id: chunks
    type: chunk
    repeat: expr
    repeat-expr: chunk_count

types:
  chunk:
    seq:
      - id: kind
        type: u4
      - id: len_body
        type: u4
      - id: body
        size: len_body
