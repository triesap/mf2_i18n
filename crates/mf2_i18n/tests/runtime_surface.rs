#[cfg(feature = "runtime")]
mod tests {
    use mf2_i18n::{
        Manifest, ManifestSigning, PackEntry, Runtime, RuntimeError, RuntimeParts, RuntimeResult,
        StdFormatBackend, parse_sha256_literal,
    };

    #[test]
    fn root_exports_runtime_surface() {
        let _runtime: Option<Runtime> = None;
        let _parts: Option<RuntimeParts> = None;
        let _manifest: Option<Manifest> = None;
        let _signing: Option<ManifestSigning> = None;
        let _entry: Option<PackEntry> = None;
        let _parse: fn(&str) -> RuntimeResult<[u8; 32]> = parse_sha256_literal;

        let err = parse_sha256_literal("sha256:00").expect_err("invalid digest should fail");
        assert!(matches!(err, RuntimeError::InvalidHash));

        let backend = StdFormatBackend::new("en-US").expect("std backend");
        assert_eq!(backend.resolution().requested_locale(), "en-US");
    }
}
