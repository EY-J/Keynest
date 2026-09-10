use std::{
    io,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null_mut,
};

use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        RemoteDesktop::ProcessIdToSessionId,
        Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken},
    },
};

// Read/write data, EA/attributes, READ_CONTROL, SYNCHRONIZE. No append/create
// instance, WRITE_DAC, WRITE_OWNER, generic rights, Everyone or anonymous ACEs.
pub(super) const CLIENT_ACL_RIGHTS: u32 = 0x0012_019b;

pub(super) struct Identity {
    pub sid: String,
    pub session: u32,
}

impl Identity {
    pub fn current() -> io::Result<Self> {
        // SAFETY: handles/buffers stay owned until each synchronous API returns.
        // usize storage provides alignment for TOKEN_USER and its embedded SID.
        unsafe {
            let mut raw = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = OwnedHandle::from_raw_handle(raw);
            let mut length = 0;
            GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut length);
            if length < size_of::<TOKEN_USER>() as u32 || length > 65536 {
                return Err(io::Error::other("Windows identity unavailable"));
            }
            let mut storage = vec![0_usize; (length as usize).div_ceil(size_of::<usize>())];
            if GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                storage.as_mut_ptr().cast(),
                length,
                &mut length,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*storage.as_ptr().cast::<TOKEN_USER>();
            let mut text = null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let allocation = LocalAllocation(text.cast());
            let mut count = 0;
            while *text.add(count) != 0 {
                count += 1;
            }
            let sid = String::from_utf16(std::slice::from_raw_parts(text, count))
                .map_err(|_| io::Error::other("Windows identity unavailable"))?;
            drop(allocation);
            let mut session = 0;
            if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { sid, session })
        }
    }

    pub fn pipe_name(&self) -> String {
        format!(
            r"\\.\pipe\keynest-autofill-v1-{}-{}",
            self.sid, self.session
        )
    }

    pub fn descriptor(&self) -> io::Result<LocalAllocation> {
        let sddl = wide(&format!("D:P(A;;0x{CLIENT_ACL_RIGHTS:08x};;;{})", self.sid));
        let mut descriptor = null_mut();
        // SAFETY: NUL-terminated SDDL lives through the call; LocalFree owns output.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(LocalAllocation(descriptor))
    }
}

pub(super) struct LocalAllocation(pub *mut std::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: pointer came from a successful LocalAlloc-family Windows API.
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
