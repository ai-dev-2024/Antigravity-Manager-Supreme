use serde_json::Value;

/// Using Antigravity 的 loadCodeAssist API Get project_id
/// This isGet cloudaicompanionProject the correct way
pub async fn fetch_project_id(access_token: &str) -> Result<String, String> {
    let url = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
    
    let request_body = serde_json::json!({
        "metadata": {
            "ideType": "ANTIGRAVITY"
        }
    });
    
    let client = crate::utils::http::get_client();
    let response = client
        .post(url)
        .bearer_auth(access_token)
        .header("Host", "cloudcode-pa.googleapis.com")
        .header("User-Agent", "antigravity/1.11.9 windows/amd64")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("loadCodeAssist RequestFailed: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("loadCodeAssist ReturnError {}: {}", status, body));
    }
    
    let data: Value = response.json()
        .await
        .map_err(|e| format!("ParseResponseFailed: {}", e))?;
    
    // extract cloudaicompanionProject
    if let Some(project_id) = data.get("cloudaicompanionProject")
        .and_then(|v| v.as_str()) {
        return Ok(project_id.to_string());
    }
    
    // IfNoneReturn project_id，DescriptionAccountNot eligible，Usingbuilt-inRandomGenerate logic as a fallback
    let mock_id = generate_mock_project_id();
    tracing::warn!("AccountNot eligibleGetofficial cloudaicompanionProject，将UsingRandomgenerated Project ID as a cover: {}", mock_id);
    Ok(mock_id)
}

/// generateRandom project_id（WhenUnable to access from API Get时Using）
/// Format：{adjective}-{noun}-{5位Randomcharacter}
pub fn generate_mock_project_id() -> String {
    use rand::Rng;
    
    let adjectives = ["useful", "bright", "swift", "calm", "bold"];
    let nouns = ["fuze", "wave", "spark", "flow", "core"];
    
    let mut rng = rand::thread_rng();
    let adj = adjectives[rng.gen_range(0..adjectives.len())];
    let noun = nouns[rng.gen_range(0..nouns.len())];
    
    // generate5位Randomcharacter（base36）
    let random_num: String = (0..5)
        .map(|_| {
            let chars = "abcdefghijklmnopqrstuvwxyz0123456789";
            let idx = rng.gen_range(0..chars.len());
            chars.chars().nth(idx).unwrap()
        })
        .collect();
    
    format!("{}-{}-{}", adj, noun, random_num)
}
