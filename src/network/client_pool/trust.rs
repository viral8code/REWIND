//! Reuse verified TLS configurations independently of the bounded origin pool.
use super::*;
use rustls::pki_types::{pem::PemObject, CertificateDer};

struct Configuration {
    config: Arc<rustls::ClientConfig>,
    bytes: usize,
    slot: usize,
}
#[derive(Default)]
pub(super) struct TrustCache {
    system: Option<Arc<rustls::RootCertStore>>,
    configs: BTreeMap<String, Configuration>,
}
impl TrustCache {
    pub fn configuration(
        &mut self,
        authority: &str,
        ca: &[u8],
    ) -> Result<(Arc<rustls::ClientConfig>, usize), &'static str> {
        if let Some(entry) = self.configs.get(authority) {
            return Ok((entry.config.clone(), entry.slot));
        }
        // Default trust plus eight distinct public CA fingerprints, for the host lifetime.
        if self.configs.len() >= 9 {
            return Err("HttpCaLimit");
        }
        let certificates = if ca.is_empty() {
            vec![]
        } else {
            let certificates = CertificateDer::pem_slice_iter(ca)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "HttpCa")?;
            if certificates.is_empty() {
                return Err("HttpCa");
            }
            certificates
        };
        let system = if let Some(roots) = &self.system {
            roots.clone()
        } else {
            let mut roots = rustls::RootCertStore::empty();
            let loaded = rustls_native_certs::load_native_certs();
            let had_certificates = !loaded.certs.is_empty();
            for certificate in loaded.certs {
                // Native stores can contain obsolete roots. Match reqwest's native-root policy.
                let _ = roots.add(certificate);
            }
            if roots.is_empty() && had_certificates {
                return Err("HttpTls");
            }
            let roots = Arc::new(roots);
            roots
        };
        let roots = if ca.is_empty() {
            system.clone()
        } else {
            let mut roots = system.as_ref().clone();
            for certificate in certificates {
                roots.add(certificate).map_err(|_| "HttpTls")?;
            }
            Arc::new(roots)
        };
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| "HttpTls")?
        .with_root_certificates(roots)
        .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        config.resumption = rustls::client::Resumption::in_memory_sessions(256);
        let config = Arc::new(config);
        let slot = self.configs.len();
        self.system.get_or_insert(system);
        self.configs.insert(
            authority.into(),
            Configuration {
                config: config.clone(),
                bytes: CLIENT_ALLOWANCE + 2 * ca.len() + authority.len() + 1024,
                slot,
            },
        );
        Ok((config, slot))
    }
    pub fn uncovered_bytes(&self, covered: &[bool; 9]) -> usize {
        // A client's existing allowance covers its shared trust configuration. Count
        // cached configurations separately only when no retained client covers them.
        self.configs
            .iter()
            .filter(|(_, entry)| !covered[entry.slot])
            .map(|(_, entry)| entry.bytes)
            .sum()
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.configs.len()
    }
}
