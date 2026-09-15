use reqwest::Client;
use serde_json::{Value, json};
use std::error::Error;
use std::io::{self, Write};

async fn ask(prompt: &str) -> Result<String, Box<dyn Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;

    let response: Value = Client::new()
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&json!({
            "model": "gpt-5.6-luna",
            "input": prompt
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // 从 Responses API 返回的数据里找到文本
    let text = response["output"]
        .as_array()
        .and_then(|items| {
            items.iter().find_map(|item| {
                item["content"]
                    .as_array()?
                    .iter()
                    .find(|content| content["type"] == "output_text")?["text"]
                    .as_str()
            })
        })
        .ok_or("No output text")?;

    Ok(text.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    print!("请输入问题：");
    io::stdout().flush()?;

    let mut prompt = String::new();
    io::stdin().read_line(&mut prompt)?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("问题不能为空".into());
    }

    let answer = ask(prompt).await?;

    println!("{answer}");

    Ok(())
}
