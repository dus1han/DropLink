//! Device identity: a long-lived ECDSA P-256 key pair and self-signed
//! certificate. The device ID is the SHA-256 fingerprint of the certificate,
//! so an ID can only be claimed by whoever holds the matching private key
//! (enforced by the TLS handshake signature).

use std::fs;
use std::io::BufReader;
use std::path::Path;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

const CERT_FILE: &str = "identity.crt";
const KEY_FILE: &str = "identity.key";

pub struct Identity {
    pub device_id: String,
    pub cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
}

impl Identity {
    pub fn key(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    /// Loads the identity from `dir`, generating a new one on first launch.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        if cert_path.exists() && key_path.exists() {
            match Self::load(&cert_path, &key_path) {
                Ok(id) => return Ok(id),
                Err(e) => tracing::warn!("identity unreadable ({e}); generating a new one"),
            }
        }
        let id = Self::generate()?;
        write_private(&key_path, id.key_pem.as_bytes())?;
        fs::write(&cert_path, id.cert_pem.as_bytes())?;
        tracing::info!(device_id = %id.identity.device_id, "generated new device identity");
        Ok(id.identity)
    }

    fn load(cert_path: &Path, key_path: &Path) -> Result<Self> {
        let mut cert_reader = BufReader::new(fs::File::open(cert_path)?);
        let cert = rustls_pemfile::certs(&mut cert_reader)
            .next()
            .ok_or_else(|| Error::Other("identity certificate missing".into()))??;
        let mut key_reader = BufReader::new(fs::File::open(key_path)?);
        let key = rustls_pemfile::private_key(&mut key_reader)?
            .ok_or_else(|| Error::Other("identity key missing".into()))?;
        Ok(Self { device_id: fingerprint(&cert), cert, key })
    }

    fn generate() -> Result<Generated> {
        let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
        let mut params = rcgen::CertificateParams::new(vec!["droplink.local".to_string()])?;
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "DropLink Device");
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(2124, 1, 1);
        let cert = params.self_signed(&key_pair)?;
        let cert_der = cert.der().clone();
        let key = PrivateKeyDer::try_from(key_pair.serialize_der())
            .map_err(|e| Error::Other(e.to_string()))?;
        Ok(Generated {
            cert_pem: cert.pem(),
            key_pem: key_pair.serialize_pem(),
            identity: Identity { device_id: fingerprint(&cert_der), cert: cert_der, key },
        })
    }

    /// In-memory identity for tests.
    pub fn ephemeral() -> Result<Self> {
        Ok(Self::generate()?.identity)
    }
}

struct Generated {
    cert_pem: String,
    key_pem: String,
    identity: Identity,
}

pub fn fingerprint(cert: &CertificateDer<'_>) -> String {
    hex::encode(Sha256::digest(cert.as_ref()))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(bytes)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        // The per-user application data directory is only readable by the
        // owning account (and administrators) under default Windows ACLs.
        fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(dir.path()).unwrap();
        let b = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(a.device_id, b.device_id);
        assert_eq!(a.device_id.len(), 64);
        let c = Identity::ephemeral().unwrap();
        assert_ne!(a.device_id, c.device_id);
    }
}
