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
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".hmdriver_rs").join("keys")
}

pub fn current_hostname() -> String {
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Ok(output) = Command::new("hostname").output() {
        let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !hostname.is_empty() {
            return hostname;
        }
    }

    "unknown".to_string()
}

fn set_secure_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::auth::HostKeys;

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
}
