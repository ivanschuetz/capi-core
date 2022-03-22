use data_encoding::BASE64;

/// a general (capi) note prefix
/// for now used only here, so here. we might use it for other kind of notes in the future.
pub fn capi_note_prefix() -> [u8; 4] {
    // Just an arbitrary byte sequence (here specifically: control char, space, tilde, control char)
    // (we don't need this to be human readable)
    [0x8, 0x20, 0x7e, 0x82]
}

pub fn dao_setup_prefix() -> [u8; 8] {
    let left: [u8; 4] = capi_note_prefix();
    let right: [u8; 4] = [0x2, 0x19, 0x20, 0x0A];

    let mut arr: [u8; 8] = [0; 8];
    let (first, last) = arr.split_at_mut(4);
    first.copy_from_slice(&left);
    last.copy_from_slice(&right);
    arr
}

pub fn dao_setup_prefix_base64() -> String {
    let prefix = dao_setup_prefix();
    BASE64.encode(&prefix)
}
