/// SAS-based authentication
#[cfg(feature = "sas")]
pub mod sas;

/// Certificates-based authentication
#[cfg(feature = "certificates")]
pub mod certificate;

/// Represents the credentials of a single device or module
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "certificates"), derive(Copy))]
pub enum DeviceCredentials {
    /// Shared Access Signature
    #[cfg(feature = "sas")]
    Sas(String),

    /// X509 certificate
    #[cfg(feature = "certificates")]
    Certificate(certificate::DeviceCertificate),
}
