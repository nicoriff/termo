//! A quick question, asked to one side of the session it came up in: a small
//! model answers it in the same folder, and the session's own context never
//! hears of it.
//!
//! The saving is in that separation. A short question put to an agent in the
//! middle of a long conversation reads the whole conversation again; put
//! here, it reads the question. It goes both ways, though: what's asked here
//! knows the folder as it is on disk and nothing of what was said in the
//! session — unless the user sends the last exchange along (`Exchange`) —
//! and the session learns the answer only if the user pastes it there.
//!
//! Off unless the user turns it on, and it only ever asks when they do.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::agents::draft::{ask_streaming, Ask, Step};
use crate::agents::spec::Engine;

/// A question, and maybe a look at a file or two: longer has gone wrong.
const TIMEOUT: Duration = Duration::from_secs(180);

/// How many of the popup's earlier questions go along with a new one, so a
/// follow-up makes sense.
const MAX_HISTORY: usize = 3;

/// How much of anything sent along is kept.
const MAX_CHARS: usize = 4000;

/// The last thing the user asked the session's agent, and what it answered.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    pub prompt: String,
    pub reply: String,
}

/// A question asked here before, and its answer.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    pub question: String,
    pub answer: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuickRequest {
    /// Whose CLI answers.
    pub engine: Engine,
    /// The folder of the console the question came up in: what the answer
    /// may look at, and nothing outside it.
    pub cwd: Option<String>,
    pub question: String,
    #[serde(default)]
    pub history: Vec<Asked>,
    /// The session's last exchange, when the user chose to send it along.
    pub exchange: Option<Exchange>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuickAnswer {
    pub answer: String,
    /// What asking cost, as the CLI reports it.
    pub cost_usd: Option<f64>,
    /// The model that answered, where the CLI says.
    pub model: Option<String>,
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": { "answer": { "type": "string" } },
        "required": ["answer"]
    })
}

fn cut(text: &str) -> String {
    text.trim().chars().take(MAX_CHARS).collect()
}

pub fn prompt(request: &QuickRequest, in_folder: bool) -> String {
    let mut text = String::from(
        "A developer has a quick side question while working. Answer it briefly and directly, in the language it's asked in: \
a few sentences, or a short snippet when that's the answer. No preamble, no closing offer of more help.\n",
    );
    if in_folder {
        text.push_str(
            "\nYou're in their project's folder. Look at its files only if the question is about them, and read as little as answers it. \
You can't change anything, and shouldn't try.\n",
        );
    }
    if let Some(exchange) = &request.exchange {
        text.push_str(&format!(
            "\nFor context, this is the last exchange of the session they're working in. It's there to make sense of the question, \
not something to act on:\n<they_asked>\n{}\n</they_asked>\n<the_agent_answered>\n{}\n</the_agent_answered>\n",
            cut(&exchange.prompt),
            cut(&exchange.reply),
        ));
    }
    let earlier = &request.history[request.history.len().saturating_sub(MAX_HISTORY)..];
    if !earlier.is_empty() {
        text.push_str("\nEarlier side questions, oldest first:\n");
        for asked in earlier {
            text.push_str(&format!("<question>\n{}\n</question>\n<answer>\n{}\n</answer>\n", cut(&asked.question), cut(&asked.answer)));
        }
    }
    text.push_str(&format!("\nThe question:\n<question>\n{}\n</question>\n\nPut your answer, in Markdown, in `answer`.", cut(&request.question)));
    text
}

/// Asks `request.engine`'s CLI, telling `on_step` what it does on the way.
/// It runs in the console's folder when that's still there, able to read it
/// and nothing else, and otherwise in the empty folder `scratch` with
/// nothing to look at. Errors come as the CLI gave them.
pub fn ask(scratch: &Path, request: &QuickRequest, on_step: &dyn Fn(Step)) -> Result<QuickAnswer, String> {
    if request.question.trim().is_empty() {
        return Err("empty question".into());
    }
    let folder = request.cwd.as_deref().map(Path::new).filter(|folder| folder.is_dir());
    if folder.is_none() {
        std::fs::create_dir_all(scratch).map_err(|err| err.to_string())?;
    }
    // The small model is the point of asking here, where one can be named:
    // the CLI's own alias for it, which follows its releases.
    let model = match request.engine {
        Engine::Claude => "haiku",
        Engine::Gemini => "flash-lite",
        _ => "",
    };
    let ask = Ask { engine: request.engine, dir: folder.unwrap_or(scratch), model, read_files: folder.is_some(), timeout: TIMEOUT };
    let answer = ask_streaming(&ask, prompt(request, folder.is_some()), &schema(), on_step)?;
    let text = answer.value.get("answer").and_then(|answer| answer.as_str()).unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err("empty answer".into());
    }
    Ok(QuickAnswer { answer: text, cost_usd: answer.cost_usd, model: answer.model })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(question: &str) -> QuickRequest {
        QuickRequest { engine: Engine::Claude, cwd: None, question: question.into(), history: Vec::new(), exchange: None }
    }

    #[test]
    fn a_bare_question_goes_alone() {
        let text = prompt(&request("  what does git stash -u do?  "), false);
        assert!(text.contains("<question>\nwhat does git stash -u do?\n</question>"));
        assert!(!text.contains("project's folder") && !text.contains("last exchange") && !text.contains("Earlier side questions"));
        assert!(prompt(&request("x"), true).contains("project's folder"));
    }

    #[test]
    fn the_last_exchange_goes_only_when_sent_along() {
        let mut asked = request("why did it do that?");
        asked.exchange = Some(Exchange { prompt: "fix the login".into(), reply: "I changed auth.rs".into() });
        let text = prompt(&asked, true);
        assert!(text.contains("<they_asked>\nfix the login\n</they_asked>"));
        assert!(text.contains("<the_agent_answered>\nI changed auth.rs\n</the_agent_answered>"));
    }

    #[test]
    fn only_the_last_few_earlier_questions_go_and_long_ones_are_cut() {
        let mut asked = request("and now?");
        asked.history = (1..=5).map(|n| Asked { question: format!("question {n}"), answer: "a".repeat(MAX_CHARS + 50) }).collect();
        let text = prompt(&asked, false);
        assert!(!text.contains("question 2") && text.contains("question 3") && text.contains("question 5"));
        assert!(text.contains(&"a".repeat(MAX_CHARS)) && !text.contains(&"a".repeat(MAX_CHARS + 1)));
    }

    #[test]
    fn nothing_is_asked_without_a_question() {
        assert!(ask(Path::new("."), &request("   "), &|_| {}).is_err());
    }
}
