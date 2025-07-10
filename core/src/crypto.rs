pub fn init_crypto() {
    #[allow(clippy::expect_used)]
    // Expect is used here because we are initializing the crypto provider at startup
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
}
