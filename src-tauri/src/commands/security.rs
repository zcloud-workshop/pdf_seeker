//! PDF standard security handler (RC4 128-bit, V2 / R3).
//!
//! lopdf 0.34 can only *decrypt* RC4 (V1/V2, R2/R3) — AESV2 is unsupported —
//! so this module implements the write side itself (Algorithms 3.1–3.5 of the
//! PDF specification) and reuses lopdf's `get_encryption_key` for decryption
//! key derivation and password validation. Unlike `Document::decrypt`, the
//! decryption here recurses into strings nested in arrays and dictionaries.

use lopdf::{encryption, Document, Object, ObjectId};
use md5::{Digest, Md5};
use serde::Deserialize;

type AppResult<T> = Result<T, String>;

const KEY_LEN: usize = 16; // 128-bit

/// Standard 32-byte password padding (PDF 1.7 Table 3.2)
const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

fn pad_password(pw: &[u8]) -> [u8; 32] {
    let n = pw.len().min(32);
    let mut out = [0u8; 32];
    out[..n].copy_from_slice(&pw[..n]);
    // Spec: append the FIRST (32 - n) bytes of the standard padding string
    out[n..].copy_from_slice(&PAD[..32 - n]);
    out
}

struct Rc4 {
    i: u8,
    j: u8,
    state: [u8; 256],
}

impl Rc4 {
    fn new(key: &[u8]) -> Self {
        let mut state = [0u8; 256];
        for (n, s) in state.iter_mut().enumerate() {
            *s = n as u8;
        }
        let (mut i, mut j) = (0usize, 0u8);
        while i < 256 {
            j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
            state.swap(i, j as usize);
            i += 1;
        }
        Rc4 { i: 0, j: 0, state }
    }

    fn xcrypt(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &byte in data {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.state[self.i as usize]);
            self.state.swap(self.i as usize, self.j as usize);
            let k = self.state[(self.state[self.i as usize].wrapping_add(self.state[self.j as usize])) as usize];
            out.push(byte ^ k);
        }
        out
    }
}

fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    Rc4::new(key).xcrypt(data)
}

/// Algorithm 3.2: file encryption key (R3: 50 extra MD5 rounds)
fn file_encryption_key(user_pw: &[u8], o: &[u8], p: i32, id0: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(32 + 32 + 4 + id0.len());
    input.extend_from_slice(&pad_password(user_pw));
    input.extend_from_slice(o);
    input.extend_from_slice(&p.to_le_bytes()[..4]);
    input.extend_from_slice(id0);
    let mut hash = Md5::digest(&input).to_vec();
    for _ in 0..50 {
        hash = Md5::digest(&hash).to_vec();
    }
    hash[..KEY_LEN].to_vec()
}

/// Algorithm 3.3: /O owner entry
fn owner_entry(owner_pw: &[u8], user_pw: &[u8]) -> Vec<u8> {
    let mut digest = Md5::digest(pad_password(owner_pw)).to_vec();
    for _ in 0..50 {
        digest = Md5::digest(&digest).to_vec();
    }
    let key = &digest[..KEY_LEN];
    let mut out = rc4(key, &pad_password(user_pw));
    for round in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|&b| b ^ round).collect();
        out = rc4(&k, &out);
    }
    out
}

/// Algorithm 3.5: /U user entry (R3)
fn user_entry(file_key: &[u8], id0: &[u8]) -> Vec<u8> {
    let mut input = Vec::with_capacity(32 + id0.len());
    input.extend_from_slice(&PAD);
    input.extend_from_slice(id0);
    let mut out = rc4(file_key, &Md5::digest(&input));
    for round in 1..=19u8 {
        let k: Vec<u8> = file_key.iter().map(|&b| b ^ round).collect();
        out = rc4(&k, &out);
    }
    out.extend_from_slice(&[0u8; 16]); // 16 arbitrary bytes
    out
}

/// Algorithm 3.1: per-object RC4 key
fn object_key(file_key: &[u8], id: ObjectId) -> Vec<u8> {
    let mut input = Vec::with_capacity(file_key.len() + 5);
    input.extend_from_slice(file_key);
    input.extend_from_slice(&id.0.to_le_bytes()[..3]);
    input.extend_from_slice(&id.1.to_le_bytes()[..2]);
    let d = Md5::digest(&input);
    d[..(file_key.len() + 5).min(16)].to_vec()
}

/// Recursively encrypt every string and stream content of the object.
/// Encrypted strings switch to hexadecimal serialization because the
/// ciphertext is binary.
fn encrypt_object(obj: &mut Object, file_key: &[u8], id: ObjectId) {
    match obj {
        Object::String(bytes, fmt) => {
            *bytes = rc4(&object_key(file_key, id), bytes);
            *fmt = lopdf::StringFormat::Hexadecimal;
        }
        Object::Array(items) => {
            for item in items.iter_mut() {
                encrypt_object(item, file_key, id);
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter_mut() {
                encrypt_object(value, file_key, id);
            }
        }
        Object::Stream(stream) => {
            // Encrypt the raw (still filter-encoded) bytes; keep /Filter as-is
            let cipher = rc4(&object_key(file_key, id), &stream.content);
            stream.set_content(cipher);
        }
        _ => {}
    }
}

/// Recursively decrypt every string and stream content of the object.
fn decrypt_object(obj: &mut Object, file_key: &[u8], id: ObjectId) {
    match obj {
        Object::String(bytes, _) => {
            *bytes = rc4(&object_key(file_key, id), bytes);
        }
        Object::Array(items) => {
            for item in items.iter_mut() {
                decrypt_object(item, file_key, id);
            }
        }
        Object::Dictionary(dict) => {
            for (_, value) in dict.iter_mut() {
                decrypt_object(value, file_key, id);
            }
        }
        Object::Stream(stream) => {
            let plain = rc4(&object_key(file_key, id), &stream.content);
            stream.set_content(plain);
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptPdfRequest {
    pub input_path: String,
    pub output_path: String,
    #[serde(default)]
    pub user_password: Option<String>,
    pub owner_password: String,
    pub allow_printing: bool,
    pub allow_modifying: bool,
    pub allow_copying: bool,
    pub allow_annotating: bool,
}

#[tauri::command]
pub fn encrypt_pdf(req: EncryptPdfRequest) -> AppResult<()> {
    if req.owner_password.is_empty() {
        return Err("Owner password is required".into());
    }

    let mut doc = Document::load(&req.input_path)
        .map_err(|e| format!("Load '{}': {}", req.input_path, e))?;
    if doc.is_encrypted() {
        return Err("This PDF is already encrypted".into());
    }

    // Normalize through a temp save/load so object streams and incremental
    // updates are flattened before encryption
    let temp_path = std::env::temp_dir().join(format!(
        "pdf_seeker_encrypt_{}.pdf",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    doc.save(&temp_path)
        .map_err(|e| format!("Normalize pass failed: {}", e))?;
    let mut doc = Document::load(&temp_path)
        .map_err(|e| format!("Reload normalized file failed: {}", e))?;
    let _ = std::fs::remove_file(&temp_path);

    // Permissions: bits 1-2 reserved zero, everything else allowed by default
    let mut p: i32 = -4; // 0xFFFFFFFC
    if !req.allow_printing { p &= !0x4; }
    if !req.allow_modifying { p &= !0x8; }
    if !req.allow_copying { p &= !0x10; }
    if !req.allow_annotating { p &= !0x20; }

    // Random file ID
    let mut id0 = [0u8; 16];
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    id0[..16].copy_from_slice(&nanos.to_le_bytes());
    let file_id = Object::String(id0.to_vec(), lopdf::StringFormat::Hexadecimal);

    let user_pw = req.user_password.clone().unwrap_or_default();
    let o = owner_entry(req.owner_password.as_bytes(), user_pw.as_bytes());
    let file_key = file_encryption_key(user_pw.as_bytes(), &o, p, &id0);
    let u = user_entry(&file_key, &id0);

    // Encrypt every object; the /Encrypt dictionary is added afterwards, so
    // its own /O and /U strings are never touched
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        if let Some(obj) = doc.objects.get_mut(&id) {
            encrypt_object(obj, &file_key, id);
        }
    }

    let encrypt_dict = Object::Dictionary(lopdf::Dictionary::from_iter(vec![
        (b"Filter".to_vec(), Object::Name(b"Standard".to_vec())),
        (b"V".to_vec(), Object::Integer(2)),
        (b"R".to_vec(), Object::Integer(3)),
        (b"Length".to_vec(), Object::Integer(128)),
        (b"P".to_vec(), Object::Integer(p as i64)),
        (b"O".to_vec(), Object::String(o, lopdf::StringFormat::Hexadecimal)),
        (b"U".to_vec(), Object::String(u, lopdf::StringFormat::Hexadecimal)),
    ]));
    let encrypt_id = doc.add_object(encrypt_dict);

    doc.trailer.set(b"Encrypt", Object::Reference(encrypt_id));
    doc.trailer.set(b"ID", Object::Array(vec![file_id.clone(), file_id]));

    doc.save(&req.output_path)
        .map_err(|e| format!("Save '{}': {}", req.output_path, e))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptPdfRequest {
    pub input_path: String,
    pub output_path: String,
    #[serde(default)]
    pub password: Option<String>,
}

/// Derive the file key from either the user or the owner password.
/// lopdf's `get_encryption_key` only handles the user-password path; for the
/// owner password, Algorithm 3.7 recovers the padded user password from /O
/// and re-runs the user path with it.
fn try_derive_key(doc: &Document, pw: &[u8]) -> Option<Vec<u8>> {
    if let Ok(key) = encryption::get_encryption_key(doc, pw, true) {
        return Some(key);
    }
    let enc_ref = doc.trailer.get(b"Encrypt").ok()?.as_reference().ok()?;
    let enc = doc.get_object(enc_ref).ok()?.as_dict().ok()?;
    let o = match enc.get(b"O").ok()? {
        Object::String(bytes, _) => bytes.clone(),
        _ => return None,
    };

    let mut digest = Md5::digest(pad_password(pw)).to_vec();
    for _ in 0..50 {
        digest = Md5::digest(&digest).to_vec();
    }
    let key = &digest[..KEY_LEN];
    let mut recovered = rc4(key, &o);
    for round in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|&b| b ^ round).collect();
        recovered = rc4(&k, &recovered);
    }
    // `recovered` is the 32-byte padded user password; feeding it back runs
    // Algorithm 3.2 with zero additional padding
    encryption::get_encryption_key(doc, &recovered, true).ok()
}

#[tauri::command]
pub fn decrypt_pdf(req: DecryptPdfRequest) -> AppResult<()> {
    let mut doc = Document::load(&req.input_path)
        .map_err(|e| format!("Load '{}': {}", req.input_path, e))?;
    if !doc.is_encrypted() {
        return Err("This PDF is not encrypted".into());
    }

    let password = req.password.clone().unwrap_or_default();
    let file_key = try_derive_key(&doc, password.as_bytes())
        .ok_or("Wrong password or unsupported encryption (only RC4 V2/R3 is supported)")?;

    let encrypt_id = doc.trailer.get(b"Encrypt")
        .ok()
        .and_then(|o| o.as_reference().ok());

    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        if Some(id) == encrypt_id {
            continue; // the encryption dictionary itself is never encrypted
        }
        if let Some(obj) = doc.objects.get_mut(&id) {
            decrypt_object(obj, &file_key, id);
        }
    }

    doc.trailer.remove(b"Encrypt");
    if let Some(eid) = encrypt_id {
        doc.objects.remove(&eid);
    }

    doc.save(&req.output_path)
        .map_err(|e| format!("Save '{}': {}", req.output_path, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_plain_pdf(dir: &std::path::Path, name: &str) -> String {
        let path = dir.join(name);
        let mut doc = Document::with_version("1.4");
        let catalog_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::new()));
        let pages_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Pages".to_vec())),
            (b"Count".to_vec(), Object::Integer(1)),
            (b"Kids".to_vec(), Object::Array(vec![])),
        ])));
        if let Some(cat) = doc.objects.get_mut(&catalog_id) {
            if let Ok(d) = cat.as_dict_mut() {
                d.set("Type", Object::Name(b"Catalog".to_vec()));
                d.set("Pages", Object::Reference(pages_id));
            }
        }
        let content = b"BT /F1 12 Tf 72 700 Td (Secret Hello) Tj ET".to_vec();
        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            content,
        )));
        let font_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Font".to_vec())),
            (b"Subtype".to_vec(), Object::Name(b"Type1".to_vec())),
            (b"BaseFont".to_vec(), Object::Name(b"Helvetica".to_vec())),
        ])));
        let page_id = doc.add_object(Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            (b"Type".to_vec(), Object::Name(b"Page".to_vec())),
            (b"Parent".to_vec(), Object::Reference(pages_id)),
            (b"MediaBox".to_vec(), Object::Array(vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(612), Object::Integer(792),
            ])),
            (b"Contents".to_vec(), Object::Reference(content_id)),
            (b"Resources".to_vec(), Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                (b"Font".to_vec(), Object::Dictionary(lopdf::Dictionary::from_iter(vec![
                    (b"F1".to_vec(), Object::Reference(font_id)),
                ]))),
            ]))),
        ])));
        if let Some(pages_obj) = doc.objects.get_mut(&pages_id) {
            if let Ok(d) = pages_obj.as_dict_mut() {
                d.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
            }
        }
        doc.trailer.set(b"Root", Object::Reference(catalog_id));
        doc.save(&path).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_rc4_known_vector() {
        // RFC 6229 / classic test vector: RC4(key="Key", data="Plaintext")
        let out = rc4(b"Key", b"Plaintext");
        let expected = [0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3];
        assert_eq!(&out[..expected.len()], &expected);
    }

    #[test]
    fn test_encrypt_then_lopdf_validates_password() {
        let dir = TempDir::new().unwrap();
        let src = create_plain_pdf(dir.path(), "plain.pdf");
        let out = dir.path().join("enc.pdf");
        let out_str = out.to_string_lossy().to_string();

        encrypt_pdf(EncryptPdfRequest {
            input_path: src,
            output_path: out_str.clone(),
            user_password: Some("user456".into()),
            owner_password: "owner123".into(),
            allow_printing: true,
            allow_modifying: true,
            allow_copying: true,
            allow_annotating: true,
        })
        .unwrap();

        let doc = Document::load(&out_str).unwrap();
        assert!(doc.is_encrypted());

        // Wrong password must fail validation against our /U entry
        assert!(encryption::get_encryption_key(&doc, b"wrong", true).is_err());
        // Correct user password validates our Algorithms 3.2/3.5 against lopdf
        assert!(encryption::get_encryption_key(&doc, b"user456", true).is_ok());
        // Owner password derives the same key via Algorithm 3.7
        assert!(try_derive_key(&doc, b"owner123").is_some());
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let dir = TempDir::new().unwrap();
        let src = create_plain_pdf(dir.path(), "plain2.pdf");
        let enc = dir.path().join("enc2.pdf");
        let enc_str = enc.to_string_lossy().to_string();
        let dec = dir.path().join("dec2.pdf");
        let dec_str = dec.to_string_lossy().to_string();

        encrypt_pdf(EncryptPdfRequest {
            input_path: src,
            output_path: enc_str.clone(),
            user_password: Some("user456".into()),
            owner_password: "owner123".into(),
            allow_printing: true,
            allow_modifying: false,
            allow_copying: false,
            allow_annotating: true,
        })
        .unwrap();

        // Wrong password is rejected
        assert!(decrypt_pdf(DecryptPdfRequest {
            input_path: enc_str.clone(),
            output_path: dec_str.clone(),
            password: Some("nope".into()),
        })
        .is_err());

        decrypt_pdf(DecryptPdfRequest {
            input_path: enc_str,
            output_path: dec_str.clone(),
            password: Some("user456".into()),
        })
        .unwrap();

        let doc = Document::load(&dec_str).unwrap();
        assert!(!doc.is_encrypted());
        assert_eq!(doc.get_pages().len(), 1);

        // Content stream survived the roundtrip
        let page_id = *doc.get_pages().get(&1).unwrap();
        let contents_ref = doc.get_object(page_id).unwrap().as_dict().unwrap()
            .get(b"Contents").unwrap().as_reference().unwrap();
        let stream = doc.get_object(contents_ref).unwrap().as_stream().unwrap();
        let raw = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("(Secret Hello) Tj"), "content corrupted: {}", text);
    }

    #[test]
    fn test_empty_user_password_roundtrip() {
        let dir = TempDir::new().unwrap();
        let src = create_plain_pdf(dir.path(), "plain3.pdf");
        let enc = dir.path().join("enc3.pdf");
        let enc_str = enc.to_string_lossy().to_string();
        let dec = dir.path().join("dec3.pdf");
        let dec_str = dec.to_string_lossy().to_string();

        encrypt_pdf(EncryptPdfRequest {
            input_path: src,
            output_path: enc_str.clone(),
            user_password: None,
            owner_password: "owner-only".into(),
            allow_printing: true,
            allow_modifying: true,
            allow_copying: true,
            allow_annotating: true,
        })
        .unwrap();

        // No password needed to open (empty user password)
        decrypt_pdf(DecryptPdfRequest {
            input_path: enc_str,
            output_path: dec_str.clone(),
            password: None,
        })
        .unwrap();

        let doc = Document::load(&dec_str).unwrap();
        assert!(!doc.is_encrypted());
        assert_eq!(doc.get_pages().len(), 1);
    }

    #[test]
    fn test_decrypt_unencrypted_fails() {
        let dir = TempDir::new().unwrap();
        let src = create_plain_pdf(dir.path(), "plain4.pdf");
        let result = decrypt_pdf(DecryptPdfRequest {
            input_path: src,
            output_path: dir.path().join("x.pdf").to_string_lossy().to_string(),
            password: Some("pw".into()),
        });
        assert!(result.is_err());
    }
}

