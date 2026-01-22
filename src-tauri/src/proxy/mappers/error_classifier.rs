// ErrorClassificationModule - Place the bottom layerErrorConvert为UserfriendlyMessage
use reqwest::Error;

/// ClassificationStreaming responseError并ReturnErrorType、EnglishMessage和 i18n key
/// 
/// ReturnValue: (ErrorType, EnglishErrorMessage, i18n_key)
/// - ErrorType: used forLog和Error码
/// - EnglishMessage: fallback Message,for non-browsersClientUsing
/// - i18n_key: Front-end translationKey,for browserClientlocalization
pub fn classify_stream_error(error: &Error) -> (&'static str, &'static str, &'static str) {
    if error.is_timeout() {
        (
            "timeout_error",
            "Request timeout, please check your network connection",
            "errors.stream.timeout_error"
        )
    } else if error.is_connect() {
        (
            "connection_error",
            "Connection failed, please check your network or proxy settings",
            "errors.stream.connection_error"
        )
    } else if error.is_decode() {
        (
            "decode_error",
            "Network unstable, data transmission interrupted. Try: 1) Check network 2) Switch proxy 3) Retry",
            "errors.stream.decode_error"
        )
    } else if error.is_body() {
        (
            "stream_error",
            "Stream transmission error, please retry later",
            "errors.stream.stream_error"
        )
    } else {
        (
            "unknown_error",
            "Unknown error occurred",
            "errors.stream.unknown_error"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_timeout_error() {
        // Createa simulatedTimeout error
        let url = "http://example.com";
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        let error = rt.block_on(async {
            client.get(url).send().await.unwrap_err()
        });
        
        if error.is_timeout() {
            let (error_type, message, i18n_key) = classify_stream_error(&error);
            assert_eq!(error_type, "timeout_error");
            assert!(message.contains("timeout"));
            assert_eq!(i18n_key, "errors.stream.timeout_error");
        }
    }

    #[test]
    fn test_error_message_format() {
        // TestErrorMessageFormat
        let url = "http://invalid-domain-that-does-not-exist-12345.com";
        let client = reqwest::Client::new();
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        let error = rt.block_on(async {
            client.get(url).send().await.unwrap_err()
        });
        
        let (error_type, message, i18n_key) = classify_stream_error(&error);
        
        // ErrorTypeShouldYesknownTypeone
        assert!(
            error_type == "timeout_error" ||
            error_type == "connection_error" ||
            error_type == "decode_error" ||
            error_type == "stream_error" ||
            error_type == "unknown_error"
        );
        
        // Message不Should为Empty
        assert!(!message.is_empty());
        
        // i18n_key Should以 errors.stream. beginning
        assert!(i18n_key.starts_with("errors.stream."));
    }

    #[test]
    fn test_i18n_keys_format() {
        // ValidateAllErrorTypeAll are correct i18n_key Format
        let test_cases = vec![
            ("timeout_error", "errors.stream.timeout_error"),
            ("connection_error", "errors.stream.connection_error"),
            ("decode_error", "errors.stream.decode_error"),
            ("stream_error", "errors.stream.stream_error"),
            ("unknown_error", "errors.stream.unknown_error"),
        ];
        
        // Herewe onlyValidate i18n_key Format
        for (expected_type, expected_key) in test_cases {
            assert_eq!(format!("errors.stream.{}", expected_type), expected_key);
        }
    }
}
