//! Minimal Trello REST client — only the calls the vault sync needs.
//!
//! Auth is key + token as query parameters on every request, per Trello's
//! token-based scheme (<https://developer.atlassian.com/cloud/trello/rest/>).

use serde::Deserialize;

const API: &str = "https://api.trello.com/1";

/// A list (column) on the board.
#[derive(Debug, Clone, Deserialize)]
pub struct List {
    pub id: String,
    pub name: String,
}

/// A card, including archived ones (`closed`).
#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(rename = "idList")]
    pub id_list: String,
    #[serde(default)]
    pub closed: bool,
}

pub struct TrelloClient {
    http: reqwest::Client,
    key: String,
    token: String,
    board: String,
}

impl TrelloClient {
    /// Credentials come from config, falling back to the environment.
    ///
    /// # Errors
    /// Returns an error when key, token or board id is missing.
    pub fn from_config() -> anyhow::Result<Self> {
        let cfg = &pasta_common::config::get().trello;
        let key = or_env(&cfg.api_key, "TRELLO_API_KEY");
        let token = or_env(&cfg.token, "TRELLO_TOKEN");
        let board = or_env(&cfg.board_id, "TRELLO_BOARD_ID");

        for (name, value) in [("api_key", &key), ("token", &token), ("board_id", &board)] {
            if value.is_empty() {
                anyhow::bail!("trello {name} is not configured");
            }
        }

        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            key,
            token,
            board,
        })
    }

    fn auth(&self) -> [(&str, &str); 2] {
        [("key", self.key.as_str()), ("token", self.token.as_str())]
    }

    /// Lists on the configured board, in board order.
    ///
    /// # Errors
    /// Returns an error if the request fails or the board is not accessible.
    pub async fn lists(&self) -> anyhow::Result<Vec<List>> {
        let url = format!("{API}/boards/{}/lists", self.board);
        let resp = self.http.get(url)
            .query(&self.auth())
            .query(&[("fields", "name"), ("filter", "open")])
            .send().await?;
        Ok(check(resp).await?.json().await?)
    }

    /// Create a list at the end of the board.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn create_list(&self, name: &str) -> anyhow::Result<List> {
        let resp = self.http.post(format!("{API}/lists"))
            .query(&self.auth())
            .query(&[("idBoard", self.board.as_str()), ("name", name), ("pos", "bottom")])
            .send().await?;
        Ok(check(resp).await?.json().await?)
    }

    /// Every card on the board, archived ones included.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn cards(&self) -> anyhow::Result<Vec<Card>> {
        let url = format!("{API}/boards/{}/cards", self.board);
        let resp = self.http.get(url)
            .query(&self.auth())
            .query(&[("filter", "all"), ("fields", "name,desc,idList,closed")])
            .send().await?;
        Ok(check(resp).await?.json().await?)
    }

    /// Create a card at the top of `list_id`.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn create_card(&self, list_id: &str, name: &str, desc: &str) -> anyhow::Result<Card> {
        let resp = self.http.post(format!("{API}/cards"))
            .query(&self.auth())
            .query(&[("idList", list_id), ("name", name), ("desc", desc), ("pos", "top")])
            .send().await?;
        Ok(check(resp).await?.json().await?)
    }

    /// Move a card to another list.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn move_card(&self, card_id: &str, list_id: &str) -> anyhow::Result<()> {
        let resp = self.http.put(format!("{API}/cards/{card_id}"))
            .query(&self.auth())
            .query(&[("idList", list_id)])
            .send().await?;
        check(resp).await.map(|_| ())
    }

    /// Archive a card.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub async fn archive_card(&self, card_id: &str) -> anyhow::Result<()> {
        let resp = self.http.put(format!("{API}/cards/{card_id}"))
            .query(&self.auth())
            .query(&[("closed", "true")])
            .send().await?;
        check(resp).await.map(|_| ())
    }
}

/// Config value if set, else the environment variable, else empty.
fn or_env(configured: &str, env_key: &str) -> String {
    if configured.is_empty() {
        std::env::var(env_key).unwrap_or_default()
    } else {
        configured.to_string()
    }
}

/// Turn a non-2xx response into an error carrying Trello's message, which is
/// plain text like `invalid key` or `unauthorized card permission requested`.
async fn check(resp: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    anyhow::bail!("trello api {status}: {}", body.trim());
}
