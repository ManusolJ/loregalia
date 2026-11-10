#![cfg(windows)]

const MAIN_THREAD_STACK: u64 = 8 * 1024 * 1024;
const PE_HEADER_OFFSET: usize = 0x3c;
const COFF_HEADER_LENGTH: usize = 20;
const PE32_PLUS_MAGIC: u16 = 0x20b;
const STACK_RESERVE_OFFSET: usize = 72;

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn stack_reserve(executable: &[u8]) -> Option<u64> {
    let signature = usize::try_from(read_u32(executable, PE_HEADER_OFFSET)?).ok()?;
    if executable.get(signature..signature + 4)? != b"PE\0\0" {
        return None;
    }
    let optional_header = signature + 4 + COFF_HEADER_LENGTH;
    if read_u16(executable, optional_header)? != PE32_PLUS_MAGIC {
        return None;
    }

    read_u64(executable, optional_header + STACK_RESERVE_OFFSET)
}

#[test]
fn the_app_reserves_the_same_main_thread_stack_as_macos_and_linux() {
    let executable = std::fs::read(env!("CARGO_BIN_EXE_loregalia")).unwrap();

    assert_eq!(stack_reserve(&executable), Some(MAIN_THREAD_STACK));
}
