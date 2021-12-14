/// A device X509 certificate
#[derive(Clone, Debug)]
pub struct DeviceCertificate {
    /// Certificate private key content
    pub bytes: Vec<u8>,

    /// Private key decryption password
    pub password: String,
}
