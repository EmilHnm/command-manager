//! Keys for the Keychain: import from pasted text (OpenSSH, PEM, PuTTY .ppk)
//! or generate a new ed25519 key.

use super::known_hosts::fingerprint;
use crate::error::{Error, Result};
use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::ssh_key::{self, LineEnding};
use russh::keys::{self, PublicKey};

/// What gets stored: the key text (kept in the vault) and the public facts
/// shown in the Keychain list.
#[derive(Debug, Clone)]
pub struct ParsedKey {
    pub text: String,
    pub algorithm: String,
    /// `authorized_keys` line, with the key name as comment.
    pub public_key: String,
    pub fingerprint: String,
    /// The stored text still needs `passphrase` to be used.
    pub encrypted: bool,
}

fn describe(public: &PublicKey, name: &str, text: String, encrypted: bool) -> Result<ParsedKey> {
    let labelled = PublicKey::new(public.key_data().clone(), name.trim());
    Ok(ParsedKey {
        text,
        algorithm: public.algorithm().as_str().to_string(),
        public_key: labelled
            .to_openssh()
            .map_err(|error| Error::msg(format!("Không xuất được public key: {error}")))?,
        fingerprint: fingerprint(public),
        encrypted,
    })
}

fn passphrase_needed() -> Error {
    Error::msg("Khóa này có passphrase, hãy nhập passphrase để nhập vào Keychain")
}

/// Reads a pasted private key. A passphrase, when given, is checked here so
/// a typo is caught on import rather than on the first login.
pub fn import(text: &str, passphrase: Option<&str>, name: &str) -> Result<ParsedKey> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::msg("Chưa dán nội dung khóa riêng"));
    }
    let passphrase = passphrase.filter(|value| !value.is_empty());

    if text.starts_with("PuTTY-User-Key-File") {
        // Stored as OpenSSH, decrypted; the vault encrypts it at rest.
        let key = ssh_key::PrivateKey::from_ppk(text, passphrase.map(str::to_string)).map_err(
            |error| match passphrase {
                None if text.contains("Encryption: aes") => passphrase_needed(),
                _ => Error::msg(format!("Không đọc được khóa PuTTY: {error}")),
            },
        )?;
        let openssh = key
            .to_openssh(LineEnding::LF)
            .map_err(|error| Error::msg(format!("Không chuyển được khóa PuTTY: {error}")))?;
        return describe(key.public_key(), name, openssh.to_string(), false);
    }

    if let Ok(key) = ssh_key::PrivateKey::from_openssh(text) {
        // The public half of an OpenSSH key is readable without the
        // passphrase.
        if key.is_encrypted() {
            if let Some(passphrase) = passphrase {
                key.decrypt(passphrase)
                    .map_err(|_| Error::msg("Sai passphrase của khóa"))?;
            }
        }
        return describe(key.public_key(), name, format!("{text}\n"), key.is_encrypted());
    }

    // PEM (PKCS#1 / PKCS#8 / SEC1): the public key needs the passphrase.
    match keys::decode_secret_key(text, None) {
        Ok(key) => describe(key.public_key(), name, format!("{text}\n"), false),
        Err(keys::Error::KeyIsEncrypted) => {
            let passphrase = passphrase.ok_or_else(passphrase_needed)?;
            let key = keys::decode_secret_key(text, Some(passphrase))
                .map_err(|_| Error::msg("Sai passphrase của khóa"))?;
            describe(key.public_key(), name, format!("{text}\n"), true)
        }
        Err(error) => Err(Error::msg(format!("Nội dung khóa riêng không hợp lệ: {error}"))),
    }
}

/// A new ed25519 key, like Termius' "Generate key". It has no passphrase of
/// its own: the vault already encrypts it at rest.
pub fn generate_ed25519(name: &str) -> Result<ParsedKey> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| Error::msg(format!("Không lấy được số ngẫu nhiên: {error}")))?;
    let mut key = ssh_key::PrivateKey::from(Ed25519Keypair::from_seed(&seed));
    key.set_comment(name.trim());
    let text = key
        .to_openssh(LineEnding::LF)
        .map_err(|error| Error::msg(format!("Không tạo được khóa: {error}")))?;
    describe(key.public_key(), name, text.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENCRYPTED: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABAKHWN57b
kSgyUrpWHIbnS8AAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIA5u5MSGFrz++vaz
hNAeiyH3UT073FU18G8+8ExbWbTAAAAAkB3s4vnxtYVmCgnDlaVBmJG6bjEdW3+ydi9i4F
FeNiAIlzONG1sJNbf1wfhmwiubwsXKb16nXcMZmzhf1F5vAbcHkPs6UbXFJDAUgfaU4htK
kh/9baFx0qItH9qOelPvbyegdj2g5koBQBZmiXhAjeVmTxfMPX8ivDlPGlnZe6xPRmEml1
jP34K1hkg+nukZ8g==
-----END OPENSSH PRIVATE KEY-----";

    #[test]
    fn generated_keys_import_back_to_the_same_public_key() {
        let generated = generate_ed25519("laptop").expect("generate");
        assert_eq!(generated.algorithm, "ssh-ed25519");
        assert!(generated.public_key.starts_with("ssh-ed25519 AAAA"));
        assert!(generated.public_key.ends_with(" laptop"));
        assert!(!generated.encrypted);

        let imported = import(&generated.text, None, "again").expect("import");
        assert_eq!(imported.fingerprint, generated.fingerprint);
        assert!(keys::decode_secret_key(&imported.text, None).is_ok());

        assert_ne!(
            generate_ed25519("x").expect("second").fingerprint,
            generated.fingerprint
        );
    }

    #[test]
    fn encrypted_openssh_keys_keep_their_passphrase() {
        let without = import(ENCRYPTED, None, "enc").expect("public part is readable");
        assert!(without.encrypted);
        assert_eq!(
            without.public_key,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA5u5MSGFrz++vazhNAeiyH3UT073FU18G8+8ExbWbTA enc"
        );
        assert!(import(ENCRYPTED, Some("open sesame"), "enc").is_ok());
        assert!(import(ENCRYPTED, Some("wrong"), "enc").is_err());
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(import("   ", None, "x").is_err());
        assert!(import("hello", None, "x").is_err());
    }
}
