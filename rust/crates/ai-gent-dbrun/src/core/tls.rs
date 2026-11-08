//! TLS profile model of the runner (spec 043 FR-015..FR-017, T007).
//!
//! **Proposed field names and keys, for the owner's review** (T007):
//!
//! | profile field (TOML key) | `.env` key | values |
//! |---|---|---|
//! | `tls_mode` | `<PROFILE>_TLS_MODE` | `verify` (default) or `unverified` |
//! | `tls_ca_file` | `<PROFILE>_TLS_CA_FILE` | absolute path (or `~/...`) of a PEM file of CA certificates |
//! | `tls_server_name` | `<PROFILE>_TLS_SERVER_NAME` | the name to check the certificate against, when it differs from `host` |
//!
//! Rules, all decided in the spec (chat decision 3, 2026-11-07):
//! - The server certificate is verified by default: against the system roots, or against `tls_ca_file`
//!   when it is set. A failure is hard and names the field to set.
//! - `unverified` is only ever an explicit value of `tls_mode`. It is accepted for a profile proven
//!   local whose host is loopback, and refused for everything else, in particular for any profile
//!   declared or classed production, with an error naming `tls_mode`. It cannot be combined with
//!   `tls_ca_file`.
//! - The model ([`TlsSettings`], [`TlsContext`], [`Authorized`]) is driver-free and always compiled; the
//!   rustls configuration builder ([`client_config`]) needs the `tls` feature. [`Authorized`] can only be
//!   made by [`TlsSettings::authorize`], and the builder only takes an [`Authorized`]: no engine can
//!   build an unverified configuration without passing the policy check.
//! - Nothing here prints a certificate, a key or a host. The CA file is read bounded and refused when it
//!   holds a private key.
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// Largest CA file read, in bytes.
pub const MAX_CA_FILE_BYTES: u64 = 1024 * 1024;
/// Most certificates taken from a CA file.
pub const MAX_CA_CERTS: usize = 128;

/// A refused TLS setting. Names the profile field to fix; never holds a value of a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsError {
    field: &'static str,
    message: String,
}

impl TlsError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    /// The profile field (TOML key) the error is about.
    #[must_use]
    pub fn field(&self) -> &'static str {
        self.field
    }
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for TlsError {}

/// How the server certificate is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlsMode {
    /// Verify against the system roots, or against the CA file when one is set.
    #[default]
    Verify,
    /// No verification. Explicit, logged on every connection, proven-local loopback only.
    Unverified,
}

impl TlsMode {
    /// The value written in a profile.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Unverified => "unverified",
        }
    }
}

/// The environment class a profile is declared or proven to be (what the policy needs of
/// `profiles::Class`, kept here so `core` does not depend on `profiles`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// Production (also what an unclassified or unknown class means).
    Production,
    /// Staging.
    Staging,
    /// Development.
    Development,
    /// Proven local.
    Local,
}

/// What the unverified policy needs to know about the profile.
#[derive(Debug, Clone, Copy)]
pub struct TlsContext<'a> {
    /// The class the profile file declares (unclassified means production).
    pub declared: ClassKind,
    /// The class after classification: `Local` only when proven.
    pub proven: ClassKind,
    /// The host the driver will connect to (for a published port, the loopback address of the mapping).
    pub host: &'a str,
}

/// The TLS fields of a profile and of a request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlsSettings {
    /// `tls_mode`.
    pub mode: TlsMode,
    /// `tls_ca_file`.
    pub ca_file: Option<PathBuf>,
    /// `tls_server_name`.
    pub server_name: Option<String>,
}

impl TlsSettings {
    /// Builds the settings from the raw profile values. An empty value means "not set".
    ///
    /// Refused, each naming its field: a `tls_mode` other than `verify` or `unverified`; a
    /// `tls_ca_file` that is not absolute (or `~/...`, taken from `home`), holds a NUL or is
    /// combined with `unverified`; a `tls_server_name` that is not a host name or an IP address.
    ///
    /// # Errors
    /// [`TlsError`] naming the field.
    pub fn from_fields(
        mode: Option<&str>,
        ca_file: Option<&str>,
        server_name: Option<&str>,
        home: Option<&Path>,
    ) -> Result<Self, TlsError> {
        let mode = match mode.map(str::trim).filter(|m| !m.is_empty()) {
            None => TlsMode::Verify,
            Some(m) if m.eq_ignore_ascii_case("verify") => TlsMode::Verify,
            Some(m) if m.eq_ignore_ascii_case("unverified") => TlsMode::Unverified,
            Some(_) => {
                return Err(TlsError::new(
                    "tls_mode",
                    "must be `verify` (the default) or `unverified`",
                ));
            }
        };
        let ca_file = match ca_file.map(str::trim).filter(|c| !c.is_empty()) {
            None => None,
            Some(c) => Some(expand_ca_path(c, home)?),
        };
        if mode == TlsMode::Unverified && ca_file.is_some() {
            return Err(TlsError::new(
                "tls_ca_file",
                "cannot be set with tls_mode = unverified (it would be ignored); remove one of them",
            ));
        }
        let server_name = match server_name.map(str::trim).filter(|n| !n.is_empty()) {
            None => None,
            Some(n) if valid_server_name(n) => Some(n.to_string()),
            Some(_) => {
                return Err(TlsError::new(
                    "tls_server_name",
                    "must be a host name or an IP address (letters, digits, `-`, `_`, `.`, at most 253 characters)",
                ));
            }
        };
        Ok(Self {
            mode,
            ca_file,
            server_name,
        })
    }

    /// Applies the policy and returns the token the builder needs. Verified settings always pass;
    /// `unverified` passes only for a profile whose declared class is not production, that is proven
    /// local, and whose host is loopback. Everything else is refused, naming `tls_mode`.
    ///
    /// # Errors
    /// [`TlsError`] naming `tls_mode`.
    pub fn authorize(&self, ctx: &TlsContext<'_>) -> Result<Authorized, TlsError> {
        if self.mode == TlsMode::Unverified {
            if ctx.declared == ClassKind::Production || ctx.proven == ClassKind::Production {
                return Err(TlsError::new(
                    "tls_mode",
                    "unverified is refused on a production profile; use verify and set tls_ca_file to the CA that signed the server certificate",
                ));
            }
            if ctx.proven != ClassKind::Local {
                return Err(TlsError::new(
                    "tls_mode",
                    "unverified is accepted only for a profile proven local; this one is not (see `classify`); use verify and set tls_ca_file",
                ));
            }
            if !is_loopback_host(ctx.host) {
                return Err(TlsError::new(
                    "tls_mode",
                    "unverified is accepted only when the host is loopback (127.0.0.1, ::1, localhost); use verify and set tls_ca_file",
                ));
            }
        }
        Ok(Authorized(self.clone()))
    }
}

/// Settings that passed [`TlsSettings::authorize`].
#[derive(Debug, Clone)]
pub struct Authorized(TlsSettings);

impl Authorized {
    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &TlsSettings {
        &self.0
    }
}

fn expand_ca_path(raw: &str, home: Option<&Path>) -> Result<PathBuf, TlsError> {
    if raw.contains('\0') {
        return Err(TlsError::new("tls_ca_file", "holds a NUL character"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let Some(home) = home else {
            return Err(TlsError::new(
                "tls_ca_file",
                "starts with ~/ but the home directory is unknown; use an absolute path",
            ));
        };
        return Ok(home.join(rest));
    }
    let p = PathBuf::from(raw);
    if !p.is_absolute() {
        return Err(TlsError::new(
            "tls_ca_file",
            "must be an absolute path (or start with ~/), so it does not depend on the directory the runner starts in",
        ));
    }
    Ok(p)
}

fn valid_server_name(n: &str) -> bool {
    if n.parse::<IpAddr>().is_ok() {
        return true;
    }
    n.len() <= 253
        && n.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// Whether `host` is a loopback address or `localhost` (IPv4 `127.0.0.0/8`, IPv6 `::1`, and the
/// IPv4-mapped form of either). Anything else, an empty host included, is not.
#[must_use]
pub fn is_loopback_host(host: &str) -> bool {
    let h = host.trim();
    let h = h
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .unwrap_or(h);
    if h.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => a.is_loopback(),
        Ok(IpAddr::V6(a)) => a.is_loopback() || a.to_ipv4_mapped().is_some_and(|v| v.is_loopback()),
        Err(_) => false,
    }
}

/// The warning every unverified connection logs. Holds no host, user or password.
pub const UNVERIFIED_WARNING: &str = "warning: TLS certificate verification is OFF for this connection (tls_mode = unverified, proven-local loopback profile)";

#[cfg(feature = "tls")]
mod build {
    use super::{
        Authorized, MAX_CA_CERTS, MAX_CA_FILE_BYTES, TlsError, TlsMode, TlsSettings,
        UNVERIFIED_WARNING,
    };
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
    use std::sync::Arc;

    /// Where the unverified warning goes: called on every handshake.
    pub type WarnFn = Arc<dyn Fn(&str) + Send + Sync>;

    fn provider() -> Arc<rustls::crypto::CryptoProvider> {
        Arc::new(rustls::crypto::ring::default_provider())
    }

    /// The rustls client configuration for authorized settings: system roots or the CA file in
    /// verify mode; the warning verifier in unverified mode (warning to standard error).
    ///
    /// # Errors
    /// [`TlsError`]: the CA file is missing, too large, not a regular file, holds a private key or no
    /// certificate; or no system roots could be loaded (naming `tls_ca_file` as the way out).
    pub fn client_config(auth: &Authorized) -> Result<ClientConfig, TlsError> {
        client_config_with(auth, Arc::new(|m: &str| eprintln!("{m}")))
    }

    /// [`client_config`] with the warning sink given (tests count the warnings).
    ///
    /// # Errors
    /// As [`client_config`].
    pub fn client_config_with(auth: &Authorized, warn: WarnFn) -> Result<ClientConfig, TlsError> {
        let s = auth.settings();
        let p = provider();
        let builder = ClientConfig::builder_with_provider(Arc::clone(&p))
            .with_safe_default_protocol_versions()
            .map_err(|_| TlsError::new("tls_mode", "no usable TLS protocol version"))?;
        if s.mode == TlsMode::Unverified {
            let v = Unverified { p, warn };
            let mut cfg = builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(v))
                .with_no_client_auth();
            // A resumed session skips the verifier, so it would skip the warning: every connection
            // does a full handshake.
            cfg.resumption = rustls::client::Resumption::disabled();
            return Ok(cfg);
        }
        Ok(builder
            .with_root_certificates(roots(s)?)
            .with_no_client_auth())
    }

    fn roots(s: &TlsSettings) -> Result<RootCertStore, TlsError> {
        let mut store = RootCertStore::empty();
        if let Some(path) = &s.ca_file {
            let bytes = crate::fsutil::read_bytes_bounded(path, MAX_CA_FILE_BYTES)
                .map_err(|why| TlsError::new("tls_ca_file", why))?;
            if bytes.windows(11).any(|w| w == b"PRIVATE KEY") {
                return Err(TlsError::new(
                    "tls_ca_file",
                    "holds a private key; point it at the CA certificate only",
                ));
            }
            let mut n = 0;
            for cert in CertificateDer::pem_slice_iter(&bytes) {
                let cert = cert.map_err(|_| {
                    TlsError::new("tls_ca_file", "is not a valid PEM file of certificates")
                })?;
                n += 1;
                if n > MAX_CA_CERTS {
                    return Err(TlsError::new(
                        "tls_ca_file",
                        format!("holds more than {MAX_CA_CERTS} certificates"),
                    ));
                }
                store.add(cert).map_err(|_| {
                    TlsError::new(
                        "tls_ca_file",
                        "holds a certificate that cannot be used as a trust anchor",
                    )
                })?;
            }
            if n == 0 {
                return Err(TlsError::new("tls_ca_file", "holds no certificate"));
            }
            return Ok(store);
        }
        let loaded = rustls_native_certs::load_native_certs();
        let (added, _ignored) = store.add_parsable_certificates(loaded.certs);
        if added == 0 {
            return Err(TlsError::new(
                "tls_ca_file",
                "no system root certificates could be loaded; set tls_ca_file to the CA that signed the server certificate",
            ));
        }
        Ok(store)
    }

    /// The name to check the certificate against: `tls_server_name` when set, else `host`.
    ///
    /// # Errors
    /// [`TlsError`] naming `tls_server_name` when the name is not usable.
    pub fn server_name(s: &TlsSettings, host: &str) -> Result<ServerName<'static>, TlsError> {
        let name = match &s.server_name {
            Some(n) => n.as_str(),
            None => host.trim().trim_start_matches('[').trim_end_matches(']'),
        };
        ServerName::try_from(name.to_string()).map_err(|_| {
            TlsError::new(
                "tls_server_name",
                "the name to verify the certificate against is not a valid host name; set tls_server_name",
            )
        })
    }

    struct Unverified {
        p: Arc<rustls::crypto::CryptoProvider>,
        warn: WarnFn,
    }

    impl std::fmt::Debug for Unverified {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Unverified")
        }
    }

    impl ServerCertVerifier for Unverified {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            (self.warn)(UNVERIFIED_WARNING);
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.p.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.p.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.p.signature_verification_algorithms.supported_schemes()
        }
    }
}

#[cfg(feature = "tls")]
pub use build::{WarnFn, client_config, client_config_with, server_name};

/// A throwaway certificate authority for the engine groups' TLS tests (FR-017): never in a release
/// build (feature `test-ca`).
#[cfg(feature = "test-ca")]
pub mod testca {
    use rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
        KeyUsagePurpose,
    };
    use std::path::{Path, PathBuf};

    /// A CA made for one test.
    pub struct TestCa {
        issuer: Issuer<'static, KeyPair>,
        cert_pem: String,
    }

    /// A server certificate and key, PEM, signed by a [`TestCa`].
    pub struct ServerCert {
        /// The certificate chain (the leaf only).
        pub cert_pem: String,
        /// The private key, PKCS#8 PEM.
        pub key_pem: String,
    }

    impl TestCa {
        /// A new self-signed CA (ECDSA P-256) named `ai-gent test CA <name>`.
        ///
        /// # Panics
        /// If key generation fails (a test environment without a random source).
        #[must_use]
        pub fn new(name: &str) -> Self {
            let mut p = CertificateParams::new(Vec::<String>::new()).expect("params");
            p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, format!("ai-gent test CA {name}"));
            p.distinguished_name = dn;
            let key = KeyPair::generate().expect("key");
            let cert = p.self_signed(&key).expect("self-signed CA");
            Self {
                cert_pem: cert.pem(),
                issuer: Issuer::new(p, key),
            }
        }

        /// The CA certificate, PEM: what `tls_ca_file` points at.
        #[must_use]
        pub fn ca_pem(&self) -> &str {
            &self.cert_pem
        }

        /// Writes the CA certificate to `dir/ca.pem` and returns the path.
        ///
        /// # Panics
        /// If the file cannot be written.
        #[must_use]
        pub fn write_ca(&self, dir: &Path) -> PathBuf {
            let p = dir.join("ca.pem");
            std::fs::write(&p, &self.cert_pem).expect("write ca.pem");
            p
        }

        /// A server certificate for `names` (host names and IP addresses), signed by this CA.
        ///
        /// # Panics
        /// If a name is not valid or signing fails.
        #[must_use]
        pub fn issue_server(&self, names: &[&str]) -> ServerCert {
            let mut p =
                CertificateParams::new(names.iter().map(|n| (*n).to_string()).collect::<Vec<_>>())
                    .expect("names");
            let mut dn = DistinguishedName::new();
            dn.push(DnType::CommonName, "ai-gent test server");
            p.distinguished_name = dn;
            let key = KeyPair::generate().expect("key");
            let cert = p.signed_by(&key, &self.issuer).expect("signed server cert");
            ServerCert {
                cert_pem: cert.pem(),
                key_pem: key.serialize_pem(),
            }
        }

        /// A server certificate signed by a different, unrelated CA (for the rejection tests): same
        /// names, issuer the caller does not trust.
        #[must_use]
        pub fn rogue_server(names: &[&str]) -> ServerCert {
            Self::new("rogue").issue_server(names)
        }
    }
}
