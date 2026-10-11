//! The one part of the Companion that asks a model: a prompt the user sent,
//! written again so the agent needs fewer turns and fewer tokens for the
//! same result.
//!
//! Only ever on asking, one prompt at a time, through the CLI of an agent
//! the user already has (see `agents::draft`). What goes out is the prompt's
//! own text and a few figures about how the turn went — never what the tools
//! returned or anything else of the session — and what the answer cost comes
//! back with it, since a tool for spending less shouldn't hide what it
//! spends.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::agents::draft::{ask_with, Ask};
use crate::agents::spec::Engine;
use crate::i18n::{self, Lang};

/// One short answer, no tools: anything longer has gone wrong.
const TIMEOUT: Duration = Duration::from_secs(120);

/// A prompt longer than this is cut before it's sent: a rewrite of the start
/// of a very long one says as much as a rewrite of all of it would.
const MAX_PROMPT_CHARS: usize = 6000;

/// The prompt to look at, and how its turn went.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdviceRequest {
    /// Whose CLI answers.
    pub engine: Engine,
    /// The agent the prompt was sent to, by its name.
    pub agent: String,
    pub prompt: String,
    pub model: Option<String>,
    pub request_count: u32,
    pub tool_calls: u32,
    pub context_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Advice {
    pub rewritten: String,
    /// What was changed and why, in a few short lines.
    pub changes: Vec<String>,
    /// The prompt was fine as it was: `rewritten` is the same prompt.
    pub already_good: bool,
    /// What asking cost, as the CLI reports it.
    pub cost_usd: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answered {
    rewritten: String,
    #[serde(default)]
    changes: Vec<String>,
    #[serde(default)]
    already_good: bool,
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "rewritten": { "type": "string" },
            "changes": { "type": "array", "items": { "type": "string" } },
            "alreadyGood": { "type": "boolean" }
        },
        "required": ["rewritten", "changes", "alreadyGood"]
    })
}

pub fn prompt(request: &AdviceRequest, lang: Lang) -> String {
    let language = match lang {
        Lang::Es => "Spanish (Rioplatense, using \"vos\")",
        Lang::En => "English",
    };
    let sent: String = request.prompt.chars().take(MAX_PROMPT_CHARS).collect();
    let model = request.model.as_deref().unwrap_or("an unknown model");
    format!(
        "You review prompts people write to AI coding agents, so that the same result takes fewer turns and fewer tokens.\n\
\n\
This prompt was sent to {agent}:\n\
<prompt>\n{sent}\n</prompt>\n\
\n\
How answering it went: {calls} calls to {model}, {tools} tool calls, {context} tokens of context read in all, {output} tokens written.\n\
\n\
Write the prompt again so the agent can get it right in one go: say what the result should be and what done looks like, \
name the scope when the original implies one, put several small asks together, and leave out whatever adds nothing. \
Keep the person's own intent and language. Never invent a fact the original doesn't have: where something the agent would need is missing, \
leave a short placeholder in [square brackets] for the person to fill in.\n\
\n\
If the prompt is already as good as it needs to be, say so: return it unchanged with `alreadyGood` true and no changes. \
A short prompt is not a bad one when the task is that simple.\n\
\n\
Answer with:\n\
- `rewritten`: the prompt, ready to send, in the language it was written in.\n\
- `changes`: at most four short lines, in {language}, each naming one thing you changed and what it saves.\n\
- `alreadyGood`: whether it needed no change.",
        agent = request.agent,
        calls = request.request_count,
        tools = request.tool_calls,
        context = request.context_tokens,
        output = request.output_tokens,
    )
}

/// Asks `request.engine`'s CLI, in the empty folder `scratch`. Errors come
/// as the CLI gave them, for the panel to word.
pub fn advise(scratch: &Path, request: &AdviceRequest) -> Result<Advice, String> {
    if request.prompt.trim().is_empty() {
        return Err("empty prompt".into());
    }
    std::fs::create_dir_all(scratch).map_err(|err| err.to_string())?;
    // The small model is plenty for this, where the CLI lets one be named.
    let model = if request.engine == Engine::Claude { "haiku" } else { "" };
    let ask = Ask { engine: request.engine, dir: scratch, model, read_files: false, timeout: TIMEOUT };
    let answer = ask_with(&ask, prompt(request, i18n::current()), &schema())?;
    advice_from(answer.value, answer.cost_usd)
}

fn advice_from(value: serde_json::Value, cost_usd: Option<f64>) -> Result<Advice, String> {
    let answered: Answered = serde_json::from_value(value).map_err(|err| err.to_string())?;
    Ok(Advice {
        rewritten: answered.rewritten.trim().to_string(),
        changes: answered.changes.into_iter().map(|change| change.trim().to_string()).filter(|change| !change.is_empty()).take(4).collect(),
        already_good: answered.already_good,
        cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(prompt: &str) -> AdviceRequest {
        AdviceRequest {
            engine: Engine::Claude,
            agent: "Claude Code".into(),
            prompt: prompt.into(),
            model: Some("claude-opus-5-5".into()),
            request_count: 14,
            tool_calls: 15,
            context_tokens: 900_000,
            output_tokens: 4_200,
        }
    }

    #[test]
    fn the_prompt_carries_what_was_sent_and_how_it_went() {
        let text = prompt(&request("arreglá el login"), Lang::Es);
        assert!(text.contains("<prompt>\narreglá el login\n</prompt>"));
        assert!(text.contains("sent to Claude Code"));
        assert!(text.contains("14 calls to claude-opus-5-5, 15 tool calls, 900000 tokens of context"));
        assert!(text.contains("Spanish"));
        assert!(prompt(&request("fix the login"), Lang::En).contains("in English"));
    }

    #[test]
    fn a_very_long_prompt_is_cut_before_its_sent() {
        let long = "x".repeat(MAX_PROMPT_CHARS + 500);
        let text = prompt(&request(&long), Lang::En);
        assert!(text.contains(&"x".repeat(MAX_PROMPT_CHARS)));
        assert!(!text.contains(&"x".repeat(MAX_PROMPT_CHARS + 1)));
    }

    #[test]
    fn an_answer_reads_into_advice() {
        let value = serde_json::json!({
            "rewritten": "  Fix the login bug in [file]: …  ",
            "changes": ["Names the file", "  ", "Says what done looks like", "c", "d", "e"],
            "alreadyGood": false
        });
        let advice = advice_from(value, Some(0.002)).unwrap();
        assert_eq!(advice.rewritten, "Fix the login bug in [file]: …");
        assert_eq!(advice.changes, ["Names the file", "Says what done looks like", "c", "d"]);
        assert_eq!((advice.already_good, advice.cost_usd), (false, Some(0.002)));
        assert!(advice_from(serde_json::json!({ "changes": [] }), None).is_err());
        assert!(advise(Path::new("."), &request("   ")).is_err());
    }
}
