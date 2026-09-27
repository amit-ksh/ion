use anyhow::{Context as _, Result, bail};
use std::ffi::c_void;
use std::ptr::null_mut;

const TYPE_GENERIC: u32 = 1;
const PERSIST_LOCAL_MACHINE: u32 = 2;
const ERROR_NOT_FOUND: i32 = 1168;

#[repr(C)]
struct Credential {
    flags: u32,
    kind: u32,
    target: *mut u16,
    comment: *mut u16,
    last_written: [u32; 2],
    blob_size: u32,
    blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    target_alias: *mut u16,
    user_name: *mut u16,
}

#[link(name = "Advapi32")]
unsafe extern "system" {
    fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
    fn CredReadW(
        target: *const u16,
        kind: u32,
        flags: u32,
        credential: *mut *mut Credential,
    ) -> i32;
    fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
    fn CredFree(buffer: *mut c_void);
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn target(account: &str) -> Vec<u16> {
    wide(&format!("ion/{account}"))
}

pub fn set(account: &str, value: &str) -> Result<()> {
    let mut target = target(account);
    let mut user = wide("ion");
    let mut blob: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let credential = Credential {
        flags: 0,
        kind: TYPE_GENERIC,
        target: target.as_mut_ptr(),
        comment: null_mut(),
        last_written: [0; 2],
        blob_size: blob.len() as u32,
        blob: blob.as_mut_ptr(),
        persist: PERSIST_LOCAL_MACHINE,
        attribute_count: 0,
        attributes: null_mut(),
        target_alias: null_mut(),
        user_name: user.as_mut_ptr(),
    };
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        bail!("Windows Credential Manager unavailable")
    }
    Ok(())
}

pub fn get(account: &str) -> Result<Option<String>> {
    let target = target(account);
    let mut pointer: *mut Credential = null_mut();
    if unsafe { CredReadW(target.as_ptr(), TYPE_GENERIC, 0, &mut pointer) } == 0 {
        if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_NOT_FOUND) {
            return Ok(None);
        }
        bail!("Windows Credential Manager unavailable");
    }
    let credential = unsafe { &*pointer };
    if credential.blob_size % 2 != 0 {
        unsafe { CredFree(pointer.cast()) };
        bail!("invalid vault entry");
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(credential.blob, credential.blob_size as usize) };
    let value = String::from_utf16(
        &bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>(),
    );
    unsafe { CredFree(pointer.cast()) };
    Ok(Some(value.context("invalid vault entry")?))
}

pub fn delete(account: &str) -> Result<()> {
    let target = target(account);
    if unsafe { CredDeleteW(target.as_ptr(), TYPE_GENERIC, 0) } == 0
        && std::io::Error::last_os_error().raw_os_error() != Some(ERROR_NOT_FOUND)
    {
        bail!("Windows Credential Manager unavailable");
    }
    Ok(())
}
