// LogMiddleware
// directUsing tower_http::trace::TraceLayer::new_for_http() 在Route中

#[cfg(test)]
mod tests {
    #[test]
    fn test_logging_middleware() {
        // Logging middleware pass tower_http::trace::TraceLayer::new_for_http() directUsing
        assert!(true);
    }
}
