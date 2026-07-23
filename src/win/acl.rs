//! Lock down the plaintext engine config with a restrictive DACL so ordinary
//! Users (and network access via them) cannot read it. Windows-only,
//! best-effort: on any failure we leave the default ACL -- the file is shredded
//! ~1.5s after the engine starts anyway.
//!
//! DACL = { LocalSystem: full, BuiltinAdministrators: full }, PROTECTED (does
//! not inherit the permissive %ProgramData% ACL). The elevated engine runs as
//! an admin, so Administrators covers it. This does NOT stop a determined local
//! admin (they can take ownership) -- it removes casual Users/network reads.
#![cfg(windows)]

use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{LocalFree, GENERIC_ALL, HLOCAL};
use windows::Win32::Security::Authorization::{
    SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE,
    SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_WELL_KNOWN_GROUP, TRUSTEE_W,
};
use windows::Win32::Security::{
    CreateWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, ACE_FLAGS, ACL,
    DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID, SECURITY_MAX_SID_SIZE,
    WELL_KNOWN_SID_TYPE,
};

use crate::win::wide;

pub fn restrict_file(path: &Path) {
    let _ = try_restrict(path);
}

fn make_sid(kind: WELL_KNOWN_SID_TYPE, buf: &mut [u8]) -> Result<(), ()> {
    let mut cb = buf.len() as u32;
    unsafe {
        CreateWellKnownSid(
            kind,
            PSID::default(),
            PSID(buf.as_mut_ptr() as *mut _),
            &mut cb,
        )
        .map_err(|_| ())
    }
}

fn explicit_access(sid: &mut [u8]) -> EXPLICIT_ACCESS_W {
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_ALL.0,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: ACE_FLAGS(0), // NO_INHERITANCE
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_WELL_KNOWN_GROUP,
            // For a SID trustee, ptstrName carries the PSID pointer.
            ptstrName: PWSTR(sid.as_mut_ptr() as *mut u16),
        },
    }
}

fn try_restrict(path: &Path) -> Result<(), ()> {
    let mut sys = [0u8; SECURITY_MAX_SID_SIZE as usize];
    let mut adm = [0u8; SECURITY_MAX_SID_SIZE as usize];
    make_sid(WinLocalSystemSid, &mut sys)?;
    make_sid(WinBuiltinAdministratorsSid, &mut adm)?;

    let entries = [explicit_access(&mut sys), explicit_access(&mut adm)];

    unsafe {
        let mut pacl: *mut ACL = std::ptr::null_mut();
        if SetEntriesInAclW(Some(&entries), None, &mut pacl).0 != 0 {
            return Err(());
        }

        let mut wpath = wide(&path.to_string_lossy());
        let info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
        let err = SetNamedSecurityInfoW(
            PWSTR(wpath.as_mut_ptr()),
            SE_FILE_OBJECT,
            info,
            PSID::default(),
            PSID::default(),
            Some(pacl),
            None,
        );
        let _ = LocalFree(HLOCAL(pacl as *mut _));
        if err.0 != 0 {
            return Err(());
        }
    }
    Ok(())
}
