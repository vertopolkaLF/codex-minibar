//! TLS setup for public background fetches. Credential clients keep their
//! explicit error handling and native trust configuration.

use std::sync::Arc;

pub(crate) fn public_agent_builder() -> ureq::AgentBuilder {
    with_native_connector(ureq::native_tls::TlsConnector::new())
}

fn with_native_connector(
    connector: Result<ureq::native_tls::TlsConnector, impl std::fmt::Display>,
) -> ureq::AgentBuilder {
    let builder = ureq::AgentBuilder::new();
    match connector {
        Ok(connector) => builder.tls_connector(Arc::new(connector)),
        Err(error) => {
            // Cargo enables ureq's `tls` feature: its default connector is
            // Rustls with certificate verification and bundled public roots.
            crate::logger::info(format!(
                "Native TLS initialization failed; using bundled TLS for public fetches: {error}"
            ));
            builder
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tls_initialization_failure_does_not_panic() {
        let failure = Err(std::io::Error::other("fixture TLS configuration failure"));
        let _agent = with_native_connector(failure).build();
    }
}
