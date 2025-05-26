use crate::{
    error::{Error, MajorFlags},
    util::{Buf, BufRef},
    oid::Oid,
};
use libgssapi_sys::{gss_OID, gss_OID_desc, gss_canonicalize_name, gss_display_name, gss_duplicate_name, gss_import_name, gss_name_struct, gss_name_t, gss_release_name, gss_export_name, OM_uint32, GSS_S_COMPLETE, gss_compare_name};
#[cfg(feature = "localname")]
use libgssapi_sys::gss_localname;
#[cfg(feature = "localname")]
use crate::oid::NO_OID;
use std::{ptr, fmt};
use std::os::raw::c_int;

pub struct Name(gss_name_t);

unsafe impl Send for Name {}
unsafe impl Sync for Name {}

impl Drop for Name {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let mut _minor = GSS_S_COMPLETE;
            let _major = unsafe {
                gss_release_name(
                    &mut _minor as *mut OM_uint32,
                    &mut self.0 as *mut gss_name_t,
                )
            };
        }
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Ok(buf) = self.display_name() {
            if let Ok(s) = std::str::from_utf8(&buf) {
                write!(f, "{}", s)
            } else {
                write!(f, "<name can't be decoded>")
            }
        } else {
            write!(f, "<name can't be displayed>")
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        let mut minor = GSS_S_COMPLETE;
        let mut eq = 0;
        let _major = unsafe {
            gss_compare_name(
                &mut minor as *mut OM_uint32,
                self.0,
                other.0,
                &mut eq as *mut c_int
            )
        };
        eq == 1
    }
}

impl Name {
    pub(crate) unsafe fn to_c(&self) -> gss_name_t {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn from_c(ptr: gss_name_t) -> Self {
        Name(ptr)
    }
    
    /// parse the specified bytes as a gssapi name, with optional
    /// `kind` e.g. `GSS_NT_HOSTBASED_SERVICE` or
    /// `GSS_NT_KRB5_PRINCIPAL`.
    pub fn new(s: &[u8], kind: Option<&Oid>) -> Result<Self, Error> {
        let mut buf = BufRef::from(s);
        let mut minor = GSS_S_COMPLETE;
        let mut name = ptr::null_mut::<gss_name_struct>();
        let major = unsafe {
            gss_import_name(
                &mut minor as *mut OM_uint32,
                buf.to_c(),
                match kind {
                    None => ptr::null_mut::<gss_OID_desc>(),
                    Some(kind) => kind.to_c(),
                },
                &mut name as *mut gss_name_t,
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(Name(name))
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }

    /// canonicalize a name for the specified mechanism (or the
    /// default mechanism if not specified). This makes a copy of the
    /// name.
    pub fn canonicalize(&self, mech: Option<&Oid>) -> Result<Self, Error> {
        let mut out = ptr::null_mut::<gss_name_struct>();
        let mut minor = GSS_S_COMPLETE;
        let major = unsafe {
            gss_canonicalize_name(
                &mut minor as *mut OM_uint32,
                self.to_c(),
                match mech {
                    None => ptr::null_mut::<gss_OID_desc>(),
                    Some(id) => id.to_c()
                },
                &mut out as *mut gss_name_t,
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(Name(out))
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }

    /// Produce a contiguous string representation of a canonicalized
    /// name suitable for direct comparison. You must either use a
    /// canonical name, or call canonicalize before using this method.
    pub fn export(&self) -> Result<Buf, Error> {
        let mut out = Buf::empty();
        let mut minor = GSS_S_COMPLETE;
        let major = unsafe {
            gss_export_name(
                &mut minor as *mut OM_uint32,
                self.0,
                out.to_c()
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(out)
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }

    /// Return the raw textual representation of the internal GSS
    /// name. Usually this will be utf8, or at least ascii, but that
    /// isn't guaranteed.
    pub fn display_name(&self) -> Result<Buf, Error> {
        let mut out = Buf::empty();
        let mut minor = GSS_S_COMPLETE;
        let mut oid = ptr::null_mut::<gss_OID_desc>();
        let major = unsafe {
            gss_display_name(
                &mut minor as *mut OM_uint32,
                self.to_c(),
                out.to_c(),
                &mut oid as *mut gss_OID,
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(out)
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }

    /// Return the raw textual representation of the internal GSS name
    /// as interpreted by the specified mechanism. If no mechanism is
    /// specified then it will be assumed to be NO_OID.
    #[cfg(feature = "localname")]
    pub fn local_name(&self, mechs: Option<&Oid>) -> Result<Buf, Error> {
        let mut out = Buf::empty();
        let mut minor = GSS_S_COMPLETE;
        let major = unsafe {
            gss_localname(
                &mut minor as *mut OM_uint32,
                self.0,
                mechs.map_or(NO_OID, |o| o.to_c()),
                out.to_c()
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(out)
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }

    /// Duplicate the name.
    pub fn duplicate(&self) -> Result<Self, Error> {
        let mut copy = ptr::null_mut::<gss_name_struct>();
        let mut minor = GSS_S_COMPLETE;
        let major = unsafe {
            gss_duplicate_name(
                &mut minor as *mut OM_uint32,
                self.to_c(),
                &mut copy as *mut gss_name_t,
            )
        };
        if major == GSS_S_COMPLETE {
            Ok(Name(copy))
        } else {
            Err(Error {
                major: MajorFlags::from_bits_retain(major),
                minor
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn check_eq() {
        let n1 = Name::new("test@LOCAL.DOMAIN".as_ref(), None).unwrap();
        let n2 = Name::new("test@LOCAL.DOMAIN".as_ref(), None).unwrap();
        assert_eq!(n1, n2);
        let n3 = Name::new("test2@LOCAL.DOMAIN".as_ref(), None).unwrap();
        assert_ne!(n1, n3);
    }
}