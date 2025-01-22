use log::info;
use rorm::fields::types::MaxStr;

use crate::handler::thread::{CreateThreadRequest, MakePostRequest};
use crate::handler::user::{LoginRequest, ProfileResponse, RegisterRequest};

const ORIGIN: &str = "http://localhost:8000";
const USERNAME: &str = "alice";

pub async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::builder().cookie_store(true).build()?;

    let response = client
        .post(format!("{ORIGIN}/api/user/login"))
        .json(&LoginRequest {
            username: USERNAME.to_string(),
            password: "password".to_string(),
        })
        .send()
        .await?;
    if response.status().is_client_error() {
        client
            .post(format!("{ORIGIN}/api/user/register"))
            .json(&RegisterRequest {
                username: MaxStr::new(USERNAME.to_string()).unwrap(),
                password: "password".to_string(),
            })
            .send()
            .await?;

        info!("Created new user account");
    } else {
        info!("Logged into existing user account")
    }

    let ProfileResponse {
        username,
        role,
        posts: _,
    } = client
        .get(format!("{ORIGIN}/api/user/profile/{USERNAME}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(username, USERNAME);
    assert_eq!(role, "user");

    let thread_id = client
        .post(format!("{ORIGIN}/api/thread/create"))
        .json(&CreateThreadRequest {
            name: "Cats".to_string(),
        })
        .send()
        .await?
        .error_for_status()?
        .json::<String>()
        .await?;

    client
        .post(format!("{ORIGIN}/api/thread/posts/{thread_id}"))
        .json(&MakePostRequest {
            message: "Look at this cute cat picture I found".to_string(),
            reply_to: None,
        })
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
