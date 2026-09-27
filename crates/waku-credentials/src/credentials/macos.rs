use anyhow::{Context as _, Result, bail};
use std::ffi::c_void;
use std::ptr::{null, null_mut};

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainAddGenericPassword(
        keychain: *const c_void,
        service_len: u32,
        service: *const u8,
        account_len: u32,
        account: *const u8,
        password_len: u32,
        password: *const u8,
        item: *mut *mut c_void,
    ) -> i32;
    fn SecKeychainFindGenericPassword(
        keychain: *const c_void,
        service_len: u32,
        service: *const u8,
        account_len: u32,
        account: *const u8,
        password_len: *mut u32,
        password: *mut *mut c_void,
        item: *mut *mut c_void,
    ) -> i32;
    fn SecKeychainItemModifyAttributesAndData(
        item: *mut c_void,
        attrs: *const c_void,
        password_len: u32,
        password: *const u8,
    ) -> i32;
    fn SecKeychainItemDelete(item: *mut c_void) -> i32;
    fn SecKeychainItemFreeContent(attrs: *mut c_void, data: *mut c_void) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *mut c_void);
}
const SERVICE: &[u8] = b"ion.credentials";
fn find(account: &str, length: *mut u32, data: *mut *mut c_void, item: *mut *mut c_void) -> i32 {
    unsafe {
        SecKeychainFindGenericPassword(
            null(),
            SERVICE.len() as u32,
            SERVICE.as_ptr(),
            account.len() as u32,
            account.as_ptr(),
            length,
            data,
            item,
        )
    }
}
pub fn set(account: &str, value: &str) -> Result<()> {
    let mut item = null_mut();
    if find(account, null_mut(), null_mut(), &mut item) == 0 {
        let status = unsafe {
            SecKeychainItemModifyAttributesAndData(item, null(), value.len() as u32, value.as_ptr())
        };
        unsafe { CFRelease(item) };
        if status != 0 {
            bail!("macOS Keychain unavailable")
        }
    } else if unsafe {
        SecKeychainAddGenericPassword(
            null(),
            SERVICE.len() as u32,
            SERVICE.as_ptr(),
            account.len() as u32,
            account.as_ptr(),
            value.len() as u32,
            value.as_ptr(),
            null_mut(),
        )
    } != 0
    {
        bail!("macOS Keychain unavailable")
    }
    Ok(())
}
pub fn get(account: &str) -> Result<Option<String>> {
    let mut length = 0;
    let mut data = null_mut();
    let status = find(account, &mut length, &mut data, null_mut());
    if status == -25300 {
        return Ok(None);
    }
    if status != 0 {
        bail!("macOS Keychain unavailable")
    }
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length as usize) };
    let value = String::from_utf8(bytes.to_vec());
    unsafe { SecKeychainItemFreeContent(null_mut(), data) };
    Ok(Some(value.context("invalid vault entry")?))
}
pub fn delete(account: &str) -> Result<()> {
    let mut item = null_mut();
    let status = find(account, null_mut(), null_mut(), &mut item);
    if status == -25300 {
        return Ok(());
    }
    if status != 0 {
        bail!("macOS Keychain unavailable")
    }
    let deleted = unsafe { SecKeychainItemDelete(item) };
    unsafe { CFRelease(item) };
    if deleted != 0 {
        bail!("macOS Keychain unavailable")
    }
    Ok(())
}
