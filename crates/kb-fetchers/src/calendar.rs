use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use kb_core::{Kind, Record, Source, SyncState};
use serde::Deserialize;
use tracing::info;

pub struct CalendarFetcher {
    lookback_days: i64,
    /// When non-empty, fetch only from these calendar IDs (passed as
    /// `--calendar <id>` flags). When empty, passes `--all`.
    calendar_ids: Vec<String>,
}

impl Default for CalendarFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl CalendarFetcher {
    #[must_use]
    pub const fn new() -> Self {
        Self { lookback_days: 30, calendar_ids: vec![] }
    }

    /// Create a fetcher scoped to specific calendar IDs.
    #[must_use]
    pub fn with_calendar_ids(mut self, ids: Vec<String>) -> Self {
        self.calendar_ids = ids;
        self
    }

    /// Fetch calendar events since last cursor.
    ///
    /// # Errors
    /// Returns error if the gog CLI fails.
    pub async fn fetch(&self, state: &SyncState) -> Result<Vec<Record>> {
        let since = state.cursor("calendar_forward")
            .unwrap_or_else(|| {
                let d = Utc::now() - chrono::Duration::days(self.lookback_days);
                d.to_rfc3339()
            });

        let mut args = vec!["calendar", "events", "--json", "--max", "200", "--from"];
        let since_owned = since.clone();
        args.push(&since_owned);

        // Build owned --calendar args or fall back to --all
        let calendar_args: Vec<String> = if self.calendar_ids.is_empty() {
            // Fetch from all calendars; deduplicate below.
            args.push("--all");
            vec![]
        } else {
            self.calendar_ids.iter()
                .flat_map(|id| vec!["--calendars".to_string(), id.clone()])
                .collect()
        };

        let mut cmd_args: Vec<&str> = args;
        for arg in &calendar_args {
            cmd_args.push(arg.as_str());
        }

        let output = tokio::process::Command::new("gog")
            .args(&cmd_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gog calendar events failed: {stderr}");
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let records = parse_records(&json);

        let now = Utc::now().to_rfc3339();
        state.update_window("calendar_forward", &since, &now).ok();

        info!(count = records.len(), "calendar events fetched");
        Ok(records)
    }
}

/// Parse `gog calendar events --json` output into records, dropping cancelled
/// events and deduplicating by event ID (the same event appears once per
/// subscribed calendar when `--all` is used).
fn parse_records(json: &str) -> Vec<Record> {
    let resp: GogResponse = serde_json::from_str(json).unwrap_or(GogResponse { events: vec![] });
    let mut seen = std::collections::HashSet::new();
    resp.events.iter()
        .filter(|e| e.status.as_deref() != Some("cancelled"))
        .filter(|e| seen.insert(e.id.clone()))
        .map(event_to_record)
        .collect()
}

fn event_to_record(event: &CalEvent) -> Record {
    let id = Record::make_id(Source::Calendar, &event.id);
    let created = parse_event_time(&event.start);
    let title = event.summary.clone().unwrap_or_default();
    let content = event.description.clone().unwrap_or_else(|| title.clone());

    let author = event.organizer.as_ref()
        .and_then(|o| o.email.clone())
        .unwrap_or_default();

    let participants: Vec<String> = event.attendees.as_ref()
        .map(|a| a.iter().filter_map(|att| att.email.clone()).collect())
        .unwrap_or_default();

    let response_status = event.attendees.as_ref()
        .and_then(|a| a.iter().find(|att| att.is_self.unwrap_or(false)))
        .and_then(|att| att.response_status.clone())
        .unwrap_or_default();

    Record {
        id,
        source: Source::Calendar,
        kind: Kind::Event,
        title,
        content,
        author,
        participants,
        created_at: created,
        updated_at: created,
        url: event.html_link.clone().unwrap_or_default(),
        thread_id: String::new(),
        entities: vec![],
        tags: vec![response_status],
    }
}

fn parse_event_time(start: &EventTime) -> DateTime<Utc> {
    start.date_time.as_ref().map_or_else(
        || {
            start.date.as_ref().and_then(|d| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
                    .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                    .map(|ndt| ndt.and_utc())
            }).unwrap_or_else(Utc::now)
        },
        |dt| DateTime::parse_from_rfc3339(dt).map_or_else(|_| Utc::now(), |d| d.with_timezone(&Utc)),
    )
}

// --- gog JSON response types ---

#[derive(Deserialize)]
struct GogResponse {
    #[serde(default)]
    events: Vec<CalEvent>,
}

#[derive(Deserialize)]
struct CalEvent {
    id: String,
    summary: Option<String>,
    description: Option<String>,
    status: Option<String>,
    start: EventTime,
    organizer: Option<Person>,
    attendees: Option<Vec<Attendee>>,
    #[serde(rename = "htmlLink")]
    html_link: Option<String>,
}

#[derive(Deserialize)]
struct EventTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    email: Option<String>,
}

#[derive(Deserialize)]
struct Attendee {
    email: Option<String>,
    #[serde(rename = "responseStatus")]
    response_status: Option<String>,
    #[serde(rename = "self")]
    is_self: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "events": [
            {
                "id": "evt1",
                "summary": "Auth Migration Discussion",
                "description": "Plan the token refresh work",
                "status": "confirmed",
                "start": { "dateTime": "2026-06-20T09:00:00Z" },
                "organizer": { "email": "lead@readpeak.com" },
                "attendees": [
                    { "email": "a@readpeak.com", "responseStatus": "accepted" },
                    { "email": "b@readpeak.com", "responseStatus": "needsAction", "self": true },
                    { "email": "c@readpeak.com" },
                    { "email": "d@readpeak.com" }
                ],
                "htmlLink": "https://cal/evt1"
            },
            {
                "id": "evt2",
                "summary": "Company Holiday",
                "status": "confirmed",
                "start": { "date": "2026-06-21" }
            },
            {
                "id": "evt3",
                "summary": "Cancelled Sync",
                "status": "cancelled",
                "start": { "dateTime": "2026-06-22T10:00:00Z" }
            }
        ]
    }"#;

    #[test]
    fn cancelled_events_excluded() {
        let records = parse_records(SAMPLE);
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|r| !r.title.contains("Cancelled")));
    }

    #[test]
    fn meeting_maps_attendees_and_organizer() {
        let records = parse_records(SAMPLE);
        let meeting = records.iter().find(|r| r.title == "Auth Migration Discussion").unwrap();
        assert_eq!(meeting.id, "calendar-evt1");
        assert_eq!(meeting.source, Source::Calendar);
        assert_eq!(meeting.kind, Kind::Event);
        assert_eq!(meeting.author, "lead@readpeak.com");
        assert_eq!(meeting.participants.len(), 4);
        assert_eq!(meeting.content, "Plan the token refresh work");
        assert_eq!(meeting.url, "https://cal/evt1");
        // responseStatus of the "self" attendee is surfaced as a tag.
        assert!(meeting.tags.contains(&"needsAction".to_string()));
        assert_eq!(meeting.created_at, "2026-06-20T09:00:00Z".parse::<DateTime<Utc>>().unwrap());
    }

    #[test]
    fn all_day_event_parses_to_midnight_utc() {
        let records = parse_records(SAMPLE);
        let holiday = records.iter().find(|r| r.title == "Company Holiday").unwrap();
        assert_eq!(holiday.created_at, "2026-06-21T00:00:00Z".parse::<DateTime<Utc>>().unwrap());
        // No description → content falls back to the title.
        assert_eq!(holiday.content, "Company Holiday");
    }
}

