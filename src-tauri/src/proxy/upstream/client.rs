// upstreamClientaccomplish
// Based on highPerformancecommunicationInterfaceEncapsulation

use reqwest::{header, Client, Response, StatusCode};
use serde_json::Value;
use tokio::time::Duration;

// Cloud Code v1internal endpoints (fallback order: prod → daily)
// priorityUsingStable的 prod endpoint，avoid impactCachehit rate
const V1_INTERNAL_BASE_URL_PROD: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const V1_INTERNAL_BASE_URL_DAILY: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal";
const V1_INTERNAL_BASE_URL_FALLBACKS: [&str; 2] = [
    V1_INTERNAL_BASE_URL_PROD,   // priorityUsingProductionEnvironment（Stable）
    V1_INTERNAL_BASE_URL_DAILY,  // spareTestEnvironment（新Function）
];

pub struct UpstreamClient {
    http_client: Client,
}

impl UpstreamClient {
    pub fn new(proxy_config: Option<crate::proxy::config::UpstreamProxyConfig>) -> Self {
        let mut builder = Client::builder()
            // Connection settings (OptimizationConnectReuse，Reduce setup overhead)
            .connect_timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(16)                  // 每Hostmost 16 个Empty闲Connect
            .pool_idle_timeout(Duration::from_secs(90))  // Empty闲ConnectKeep 90 秒
            .tcp_keepalive(Duration::from_secs(60))      // TCP Keep-alive detection 60 秒
            .timeout(Duration::from_secs(600))
            .user_agent("antigravity/1.11.9 windows/amd64");

        if let Some(config) = proxy_config {
            if config.enabled && !config.url.is_empty() {
                if let Ok(proxy) = reqwest::Proxy::all(&config.url) {
                    builder = builder.proxy(proxy);
                    tracing::info!("UpstreamClient enabled proxy: {}", config.url);
                }
            }
        }

        let http_client = builder.build().expect("Failed to create HTTP client");

        Self { http_client }
    }

    /// build v1internal URL
    /// 
    /// build API RequestAddress
    fn build_url(base_url: &str, method: &str, query_string: Option<&str>) -> String {
        if let Some(qs) = query_string {
            format!("{}:{}?{}", base_url, method, qs)
        } else {
            format!("{}:{}", base_url, method)
        }
    }

    /// judgeYesNo应Tryingnext endpoint
    /// 
    /// Whenencountered the followingError时，TryingSwitch to alternate endpoint：
    /// - 429 Too Many Requests（Rate Limit）
    /// - 408 Request Timeout（Timeout）
    /// - 404 Not Found（endpointDoes not exist）
    /// - 5xx Server Error（ServerError）
    fn should_try_next_endpoint(status: StatusCode) -> bool {
        status == StatusCode::TOO_MANY_REQUESTS
            || status == StatusCode::REQUEST_TIMEOUT
            || status == StatusCode::NOT_FOUND
            || status.is_server_error()
    }

    /// call v1internal API（BaseMethod）
    /// 
    /// Launch basic networkRequest，SupportMultiple endpoints automatically Fallback
    pub async fn call_v1_internal(
        &self,
        method: &str,
        access_token: &str,
        body: Value,
        query_string: Option<&str>,
    ) -> Result<Response, String> {
        self.call_v1_internal_with_headers(method, access_token, body, query_string, std::collections::HashMap::new()).await
    }

    /// [FIX #765] call v1internal API，SupportPassthroughextra Headers
    pub async fn call_v1_internal_with_headers(
        &self,
        method: &str,
        access_token: &str,
        body: Value,
        query_string: Option<&str>,
        extra_headers: std::collections::HashMap<String, String>,
    ) -> Result<Response, String> {
        // build Headers (AllEndpoint reuse)
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", access_token))
                .map_err(|e| e.to_string())?,
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("antigravity/1.11.9 windows/amd64"),
        );

        // inject additional Headers (如 anthropic-beta)
        for (k, v) in extra_headers {
            if let Ok(hk) = header::HeaderName::from_bytes(k.as_bytes()) {
                if let Ok(hv) = header::HeaderValue::from_str(&v) {
                    headers.insert(hk, hv);
                }
            }
        }

        let mut last_err: Option<String> = None;

        // TraverseAllendpoint，FailedAutomatically switch when
        for (idx, base_url) in V1_INTERNAL_BASE_URL_FALLBACKS.iter().enumerate() {
            let url = Self::build_url(base_url, method, query_string);
            let has_next = idx + 1 < V1_INTERNAL_BASE_URL_FALLBACKS.len();

            let response = self
                .http_client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        if idx > 0 {
                            tracing::info!(
                                "✓ Upstream fallback succeeded | Endpoint: {} | Status: {} | Attempt: {}/{}",
                                base_url,
                                status,
                                idx + 1,
                                V1_INTERNAL_BASE_URL_FALLBACKS.len()
                            );
                        } else {
                            tracing::debug!("✓ Upstream request succeeded | Endpoint: {} | Status: {}", base_url, status);
                        }
                        return Ok(resp);
                    }

                    // IfThere is a next endpoint andCurrentError可Retry，then switch
                    if has_next && Self::should_try_next_endpoint(status) {
                        tracing::warn!(
                            "Upstream endpoint returned {} at {} (method={}), trying next endpoint",
                            status,
                            base_url,
                            method
                        );
                        last_err = Some(format!("Upstream {} returned {}", base_url, status));
                        continue;
                    }

                    // NoRetry的Erroror haveYesFinallyan endpoint，directReturn
                    return Ok(resp);
                }
                Err(e) => {
                    let msg = format!("HTTP request failed at {}: {}", base_url, e);
                    tracing::debug!("{}", msg);
                    last_err = Some(msg);

                    // IfYesFinallyan endpoint，exit loop
                    if !has_next {
                        break;
                    }
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "All endpoints failed".to_string()))
    }

    /// call v1internal API（带 429 Retry,Support闭Packet）
    /// 
    /// with fault tolerance andRetry的CoreRequestlogic
    /// 
    /// # Arguments
    /// * `method` - API method (e.g., "generateContent")
    /// * `query_string` - Optional query string (e.g., "?alt=sse")
    /// * `get_credentials` - 闭Packet，GetCredential（SupportAccountrotation）
    /// * `build_body` - 闭Packet，Receive project_id buildRequest体
    /// * `max_attempts` - Max RetryCount
    /// 
    /// # Returns
    /// HTTP Response
    // 已RemoveDeprecatedRetryMethod (call_v1_internal_with_retry)

    // 已RemoveDeprecatedAuxiliaryMethod (parse_retry_delay)

    // 已RemoveDeprecatedAuxiliaryMethod (parse_duration_ms)

    /// GetAvailableModelList
    /// 
    /// GetremoteModelList，SupportMultiple endpoints automatically Fallback
    #[allow(dead_code)] // API ready for future model discovery feature
    pub async fn fetch_available_models(&self, access_token: &str) -> Result<Value, String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", access_token))
                .map_err(|e| e.to_string())?,
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("antigravity/1.11.9 windows/amd64"),
        );

        let mut last_err: Option<String> = None;

        // TraverseAllendpoint，FailedAutomatically switch when
        for (idx, base_url) in V1_INTERNAL_BASE_URL_FALLBACKS.iter().enumerate() {
            let url = Self::build_url(base_url, "fetchAvailableModels", None);

            let response = self
                .http_client
                .post(&url)
                .headers(headers.clone())
                .json(&serde_json::json!({}))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        if idx > 0 {
                            tracing::info!(
                                "✓ Upstream fallback succeeded for fetchAvailableModels | Endpoint: {} | Status: {}",
                                base_url,
                                status
                            );
                        } else {
                            tracing::debug!("✓ fetchAvailableModels succeeded | Endpoint: {}", base_url);
                        }
                        let json: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("Parse json failed: {}", e))?;
                        return Ok(json);
                    }

                    // IfThere is a next endpoint andCurrentError可Retry，then switch
                    let has_next = idx + 1 < V1_INTERNAL_BASE_URL_FALLBACKS.len();
                    if has_next && Self::should_try_next_endpoint(status) {
                        tracing::warn!(
                            "fetchAvailableModels returned {} at {}, trying next endpoint",
                            status,
                            base_url
                        );
                        last_err = Some(format!("Upstream error: {}", status));
                        continue;
                    }

                    // NoRetry的Erroror haveYesFinallyan endpoint
                    return Err(format!("Upstream error: {}", status));
                }
                Err(e) => {
                    let msg = format!("Request failed at {}: {}", base_url, e);
                    tracing::debug!("{}", msg);
                    last_err = Some(msg);

                    // IfYesFinallyan endpoint，exit loop
                    if idx + 1 >= V1_INTERNAL_BASE_URL_FALLBACKS.len() {
                        break;
                    }
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "All endpoints failed".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url() {
        let base_url = "https://cloudcode-pa.googleapis.com/v1internal";
        
        let url1 = UpstreamClient::build_url(base_url, "generateContent", None);
        assert_eq!(
            url1,
            "https://cloudcode-pa.googleapis.com/v1internal:generateContent"
        );

        let url2 = UpstreamClient::build_url(base_url, "streamGenerateContent", Some("alt=sse"));
        assert_eq!(
            url2,
            "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse"
        );
    }

}
