use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use openssl::base64;
use openssl::md::Md;
use openssl::pkey::{PKey, Private};
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::{Padding, Rsa};
use openssl::sha::sha512;
use openssl::sign::RsaPssSaltlen;

use crate::error::Result;
use crate::protocol::{
    AUTH_VERIFY_RSA_3072_SHA512, HDC_HOST_DAEMON_BUF_SEPARATOR, TAG_AUTH_TYPE, append_tlv,
};

const RSA_KEY_BITS: u32 = 3072;
const PRIVATE_KEY_NAME: &str = "hdckey";
const PUBLIC_KEY_NAME: &str = "hdckey.pub";

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostPlatform {
    LinuxLike,
    Ohos,
    Windows,
}

#[cfg(target_family = "windows")]
const HOST_PLATFORM: HostPlatform = HostPlatform::Windows;

#[cfg(all(target_family = "unix", target_env = "ohos"))]
const HOST_PLATFORM: HostPlatform = HostPlatform::Ohos;

#[cfg(all(target_family = "unix", not(target_env = "ohos")))]
const HOST_PLATFORM: HostPlatform = HostPlatform::LinuxLike;

#[cfg(not(any(target_family = "windows", target_family = "unix")))]
compile_error!("hmdriver_rs host currently supports Unix-family and Windows targets only");

pub struct HostKeys {
    private_key: PKey<Private>,
    public_key_pem: String,
}

impl HostKeys {
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;

        let private_key_path = dir.join(PRIVATE_KEY_NAME);
        let public_key_path = dir.join(PUBLIC_KEY_NAME);

        if private_key_path.exists() {
            let private_key_pem = fs::read(&private_key_path)?;
            let private_key = PKey::private_key_from_pem(&private_key_pem)?;
            let public_key_pem = if public_key_path.exists() {
                String::from_utf8(fs::read(&public_key_path)?)?
            } else {
                let pem = String::from_utf8(private_key.public_key_to_pem()?)?;
                fs::write(&public_key_path, pem.as_bytes())?;
                pem
            };
            return Ok(Self {
                private_key,
                public_key_pem,
            });
        }

        let rsa = Rsa::generate(RSA_KEY_BITS)?;
        let private_key = PKey::from_rsa(rsa)?;
        let private_key_pem = private_key.private_key_to_pem_pkcs8()?;
        let public_key_pem = String::from_utf8(private_key.public_key_to_pem()?)?;

        fs::write(&private_key_path, &private_key_pem)?;
        fs::write(&public_key_path, public_key_pem.as_bytes())?;
        set_secure_permissions(&private_key_path)?;

        Ok(Self {
            private_key,
            public_key_pem,
        })
    }

    pub fn public_key_payload(&self, hostname: &str) -> Result<String> {
        Ok(format!(
            "{hostname}{separator}{pem}",
            separator = HDC_HOST_DAEMON_BUF_SEPARATOR,
            pem = self.public_key_pem
        ))
    }

    pub fn sign_token_pss_sha512_base64(&self, token: &[u8]) -> Result<String> {
        let digest = sha512(token);
        let mut ctx = PkeyCtx::new(&self.private_key)?;
        ctx.sign_init()?;
        ctx.set_rsa_padding(Padding::PKCS1_PSS)?;
        ctx.set_signature_md(Md::sha512())?;
        ctx.set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)?;

        let mut signature = Vec::new();
        let _ = ctx.sign_to_vec(&digest, &mut signature)?;
        Ok(base64::encode_block(&signature))
    }
}

pub fn modern_auth_tlv() -> String {
    let mut tlv = String::new();
    append_tlv(&mut tlv, TAG_AUTH_TYPE, AUTH_VERIFY_RSA_3072_SHA512);
    tlv
}

pub fn default_key_dir() -> PathBuf {
    default_key_dir_for_platform(HOST_PLATFORM, |key| std::env::var_os(key))
}

pub fn current_hostname() -> String {
    current_hostname_for_platform(
        HOST_PLATFORM,
        |key| std::env::var_os(key),
        || {
            Command::new("hostname")
                .output()
                .ok()
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        },
    )
}

fn set_secure_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn default_key_dir_for_platform<F>(platform: HostPlatform, mut get_env: F) -> PathBuf
where
    F: FnMut(&str) -> Option<OsString>,
{
    match platform {
        HostPlatform::Windows => {
            if let Some(base) = first_env_path(&mut get_env, &["APPDATA", "LOCALAPPDATA"]) {
                return base.join("hmdriver_rs").join("keys");
            }
            if let Some(base) = first_env_path(&mut get_env, &["USERPROFILE", "HOME"]) {
                return base.join(".hmdriver_rs").join("keys");
            }
        }
        HostPlatform::LinuxLike | HostPlatform::Ohos => {
            if let Some(base) = first_env_path(&mut get_env, &["HOME"]) {
                return base.join(".hmdriver_rs").join("keys");
            }
        }
    }

    PathBuf::from(".").join(".hmdriver_rs").join("keys")
}

fn current_hostname_for_platform<F, G>(
    platform: HostPlatform,
    mut get_env: F,
    mut run_hostname: G,
) -> String
where
    F: FnMut(&str) -> Option<OsString>,
    G: FnMut() -> Option<String>,
{
    let env_keys = match platform {
        HostPlatform::Windows => &["COMPUTERNAME", "HOSTNAME"][..],
        HostPlatform::LinuxLike | HostPlatform::Ohos => &["HOSTNAME", "COMPUTERNAME"][..],
    };

    if let Some(hostname) = first_env_string(&mut get_env, env_keys) {
        return hostname;
    }

    if let Some(hostname) = run_hostname().and_then(|value| normalize_string(&value)) {
        return hostname;
    }

    "unknown".to_string()
}

fn first_env_path<F>(get_env: &mut F, keys: &[&str]) -> Option<PathBuf>
where
    F: FnMut(&str) -> Option<OsString>,
{
    keys.iter().find_map(|key| {
        let value = get_env(key)?;
        if value.is_empty() {
            return None;
        }
        let path = PathBuf::from(value);
        if path.as_os_str().is_empty() {
            None
        } else {
            Some(path)
        }
    })
}

fn first_env_string<F>(get_env: &mut F, keys: &[&str]) -> Option<String>
where
    F: FnMut(&str) -> Option<OsString>,
{
    keys.iter()
        .find_map(|key| get_env(key).and_then(|value| normalize_os_string(&value)))
}

fn normalize_os_string(value: &OsString) -> Option<String> {
    normalize_string(&value.to_string_lossy())
}

fn normalize_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use crate::auth::{
        HostKeys, HostPlatform, current_hostname_for_platform, default_key_dir_for_platform,
    };

    fn temp_test_dir(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "hmdriver-rs-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn ensure_keypair_creates_pem_files() {
        let dir = temp_test_dir("keys");
        let _ = HostKeys::load_or_create(&dir).unwrap();

        assert!(dir.join("hdckey").exists());
        assert!(dir.join("hdckey.pub").exists());
    }

    #[test]
    fn public_key_payload_contains_hostname_separator_and_pem() {
        let dir = temp_test_dir("pubkey");
        let keys = HostKeys::load_or_create(&dir).unwrap();

        let payload = keys.public_key_payload("test-host").unwrap();

        assert!(payload.starts_with("test-host\u{000C}"));
        assert!(payload.contains("BEGIN PUBLIC KEY"));
    }

    #[test]
    fn windows_key_dir_prefers_appdata() {
        let path = default_key_dir_for_platform(HostPlatform::Windows, |key| match key {
            "APPDATA" => Some(OsString::from(r"C:\Users\tester\AppData\Roaming")),
            "USERPROFILE" => Some(OsString::from(r"C:\Users\tester")),
            _ => None,
        });

        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\tester\AppData\Roaming")
                .join("hmdriver_rs")
                .join("keys")
        );
    }

    #[test]
    fn windows_key_dir_falls_back_to_userprofile() {
        let path = default_key_dir_for_platform(HostPlatform::Windows, |key| match key {
            "USERPROFILE" => Some(OsString::from(r"C:\Users\tester")),
            _ => None,
        });

        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\tester")
                .join(".hmdriver_rs")
                .join("keys")
        );
    }

    #[test]
    fn unix_key_dir_uses_home() {
        let path = default_key_dir_for_platform(HostPlatform::LinuxLike, |key| match key {
            "HOME" => Some(OsString::from("/home/tester")),
            _ => None,
        });

        assert_eq!(path, PathBuf::from("/home/tester/.hmdriver_rs/keys"));
    }

    #[test]
    fn ohos_key_dir_matches_unix_layout() {
        let path = default_key_dir_for_platform(HostPlatform::Ohos, |key| match key {
            "HOME" => Some(OsString::from("/data/home/tester")),
            _ => None,
        });

        assert_eq!(path, PathBuf::from("/data/home/tester/.hmdriver_rs/keys"));
    }

    #[test]
    fn key_dir_without_env_falls_back_to_local_hidden_directory() {
        let path = default_key_dir_for_platform(HostPlatform::LinuxLike, |_| None);
        assert_eq!(path, PathBuf::from("./.hmdriver_rs/keys"));
    }

    #[test]
    fn windows_hostname_prefers_computername() {
        let hostname = current_hostname_for_platform(
            HostPlatform::Windows,
            |key| match key {
                "COMPUTERNAME" => Some(OsString::from("WIN-BOX")),
                "HOSTNAME" => Some(OsString::from("unix-name")),
                _ => None,
            },
            || Some("fallback".to_string()),
        );

        assert_eq!(hostname, "WIN-BOX");
    }

    #[test]
    fn unix_hostname_prefers_hostname_env() {
        let hostname = current_hostname_for_platform(
            HostPlatform::LinuxLike,
            |key| match key {
                "HOSTNAME" => Some(OsString::from("linux-host")),
                "COMPUTERNAME" => Some(OsString::from("WIN-BOX")),
                _ => None,
            },
            || Some("fallback".to_string()),
        );

        assert_eq!(hostname, "linux-host");
    }

    #[test]
    fn hostname_falls_back_to_command_output() {
        let hostname = current_hostname_for_platform(
            HostPlatform::Ohos,
            |_| None,
            || Some(" shell-host \n".to_string()),
        );

        assert_eq!(hostname, "shell-host");
    }
}
