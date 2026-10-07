//! Explicit registered Linux identity; never infer it from OCI USER or ambient HOME.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionIdentity {
    pub uid: u32,
    pub gid: u32,
    pub home: String,
}
impl ExecutionIdentity {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.uid == 1000 && self.gid == 1000 && self.home == "/home/ion",
            "Registered worker identity must match the reviewed Ion Linux runtime"
        );
        Ok(())
    }
    pub(crate) fn verify_process(&self) -> Result<()> {
        self.validate()?;
        #[cfg(unix)]
        {
            // These getters have no side effects and cannot fail. No credential
            // or native catalog lookup happens until this identity is proven.
            ensure!(
                unsafe { libc::geteuid() } == self.uid
                    && unsafe { libc::getegid() } == self.gid
                    && std::env::var_os("HOME").as_deref()
                        == Some(std::ffi::OsStr::new(&self.home))
                    && std::env::current_dir()? == std::path::Path::new("/ion/work"),
                "Worker process identity/home/cwd differs from its original registered request"
            );
            Ok(())
        }
        #[cfg(not(unix))]
        anyhow::bail!("Explicit registered worker identity requires Linux/Unix")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_remote_execution_identity_rejects_root_aliases_and_ambient_homes() {
        for (uid, gid, home) in [
            (0, 1000, "/home/ion"),
            (1000, 0, "/home/ion"),
            (1001, 1000, "/home/ion"),
            (1000, 1000, "/root"),
            (1000, 1000, "/home/ion/.."),
            (1000, 1000, ""),
        ] {
            assert!(
                ExecutionIdentity {
                    uid,
                    gid,
                    home: home.into()
                }
                .validate()
                .is_err()
            );
        }
        let identity = ExecutionIdentity {
            uid: 1000,
            gid: 1000,
            home: "/home/ion".into(),
        };
        identity.validate().unwrap();
    }
}
