//! The user's agents from an agent CLI typed in a shell (see
//! agents/console_bridge.rs). A console Termo starts as an agent gets Termo's
//! MCP server on its command line; a `claude` typed at a prompt has no
//! command line of Termo's, and Claude Code, Codex and Copilot CLI take a
//! server for one session nowhere else. So a folder of stand-ins goes ahead
//! of the real ones in the shell's PATH: each starts the real CLI with the
//! server's arguments first, when the shell has a token (`shell_extras`) and
//! its files are still there, and exactly as typed otherwise. Gemini CLI
//! has none (see `shell_extras`).
//!
//! The arguments go first so they work with subcommands too (`claude mcp
//! list`, `codex exec`), which don't take them after their own name.

use std::path::Path;

use crate::agents::console_bridge::{self, AUTH_VAR, COPILOT_FILE_VAR, FREE_TOOL, MCP_FILE_VAR, SERVER, TOOL_TIMEOUT_SEC};
use crate::agents::webhook;

/// The stand-ins' folder, in the shell's environment: what session.rs puts
/// first in PATH, and what the stand-ins take back out for the real CLI.
pub const DIR_VAR: &str = "TERMO_CONSOLE_SHIMS";
/// What a shell mustn't inherit from the one Termo itself was started in
/// (another Termo's console): they're that console's, and die with it.
pub const OWN_VARS: &[&str] = &[DIR_VAR, MCP_FILE_VAR, COPILOT_FILE_VAR, AUTH_VAR];

/// Lets Claude Code list the agents without asking, as an agent console's
/// `--allowedTools` does. A settings file rather than that flag: it takes
/// one value, so it also ends `--mcp-config`'s list — which would otherwise
/// swallow whatever the user typed next.
const CLAUDE_SETTINGS: &str = "claude-settings.json";

/// Characters cmd.exe reads as its own syntax even in a path (see
/// agent_launch.rs), plus the one that ends the text `%PATH:…=%` replaces.
#[cfg(windows)]
const CMD_SYNTAX: &[char] = &['&', '|', '<', '>', '^', '%', '"', '!', '='];

/// zsh's startup files usually put their own folders first: back to the
/// front once they're done.
const ZSH_PATH_LINE: &str = r#"[[ -n $TERMO_CONSOLE_SHIMS ]] && path=($TERMO_CONSOLE_SHIMS ${path:#$TERMO_CONSOLE_SHIMS})"#;

/// PowerShell's own stand-in for Claude Code, which is an .exe there: a
/// function runs it directly, where the .cmd one would put cmd.exe in
/// between and lose a prompt's line breaks. Single quotes only, for the
/// same Windows command line reason as cwd_report's prompt wrapper.
const POWERSHELL_CLAUDE: &str = "function global:claude { $shims = $env:TERMO_CONSOLE_SHIMS; $real = Get-Command claude -CommandType Application -All -ErrorAction SilentlyContinue | Where-Object { -not ($shims -and $_.Source.StartsWith($shims, [StringComparison]::OrdinalIgnoreCase)) } | Select-Object -First 1; $exe = if ($real) { $real.Source } else { 'claude.exe' }; $extra = @(); $mcp = $env:TERMO_CONSOLE_MCP; $timed = $false; if ($shims -and $mcp -and (Test-Path -LiteralPath $mcp)) { $extra = @('--mcp-config', $mcp, '--settings', (Join-Path $shims 'claude-settings.json')); if (-not $env:MCP_TOOL_TIMEOUT) { $env:MCP_TOOL_TIMEOUT = '__TIMEOUT_MS__'; $timed = $true } }; try { if ($MyInvocation.ExpectingInput) { $input | & $exe @extra @args } else { & $exe @extra @args } } finally { if ($timed) { Remove-Item Env:MCP_TOOL_TIMEOUT -ErrorAction SilentlyContinue } } }";

const TOOLS: &[&str] = &["claude", "codex", "copilot"];

fn codex_overrides(url: &str) -> [String; 3] {
    [
        format!("mcp_servers.{SERVER}.url={url}"),
        format!("mcp_servers.{SERVER}.env_http_headers.Authorization={AUTH_VAR}"),
        format!("mcp_servers.{SERVER}.tool_timeout_sec={TOOL_TIMEOUT_SEC}"),
    ]
}

/// The stand-in for `tool` as a POSIX script: macOS, Linux, and Git Bash on
/// Windows.
fn sh_script(tool: &str, url: &str) -> String {
    let with_server = match tool {
        "claude" => format!(
            "if [ -n \"${{{MCP_FILE_VAR}:-}}\" ] && [ -f \"${MCP_FILE_VAR}\" ]; then\n  \
             MCP_TOOL_TIMEOUT=${{MCP_TOOL_TIMEOUT:-{timeout}}}\n  export MCP_TOOL_TIMEOUT\n  \
             exec claude --mcp-config \"${MCP_FILE_VAR}\" --settings \"$self/{CLAUDE_SETTINGS}\" \"$@\"\nfi\n",
            timeout = TOOL_TIMEOUT_SEC * 1000
        ),
        "codex" => {
            let overrides: String = codex_overrides(url).iter().map(|value| format!(" -c '{value}'")).collect();
            format!(
                "if [ -n \"${{{AUTH_VAR}:-}}\" ] && [ -f \"${{{MCP_FILE_VAR}:-}}\" ]; then\n  exec codex{overrides} \"$@\"\nfi\n"
            )
        }
        _ => format!(
            "if [ -n \"${{{COPILOT_FILE_VAR}:-}}\" ] && [ -f \"${COPILOT_FILE_VAR}\" ]; then\n  \
             exec {tool} --additional-mcp-config \"@${COPILOT_FILE_VAR}\" \"$@\"\nfi\n"
        ),
    };
    format!(
        "#!/bin/sh\n\
         # Written by Termo, and rewritten when it starts a console: starts the real\n\
         # `{tool}` with the user's Termo agents within reach (pty/console_shims.rs).\n\
         self=${{0%/*}}\n\
         rest=\n\
         set -f\n\
         old_ifs=$IFS\n\
         IFS=:\n\
         for dir in $PATH; do\n  [ \"$dir\" = \"$self\" ] || rest=${{rest:+$rest:}}$dir\ndone\n\
         IFS=$old_ifs\n\
         set +f\n\
         PATH=$rest\n\
         export PATH\n\
         real=$(command -v {tool} 2>/dev/null)\n\
         # Still this file: there's no real one to start.\n\
         if [ -n \"$real\" ] && [ \"$real\" -ef \"$0\" ]; then\n  exit 127\nfi\n\
         {with_server}\
         exec {tool} \"$@\"\n"
    )
}

/// The stand-in for `tool` as a batch file, for cmd.exe and for PowerShell
/// (which runs Codex and Copilot CLI through npm's own .cmd anyway). No
/// parenthesised blocks: a folder named with one would close them.
#[cfg(windows)]
fn cmd_script(tool: &str, dir: &str, url: &str) -> String {
    let with_server = match tool {
        "claude" => format!(
            "if not defined {MCP_FILE_VAR} goto plain\r\n\
             if not exist \"%{MCP_FILE_VAR}%\" goto plain\r\n\
             if not defined MCP_TOOL_TIMEOUT set \"MCP_TOOL_TIMEOUT={timeout}\"\r\n\
             \"%REAL%\" --mcp-config \"%{MCP_FILE_VAR}%\" --settings \"{dir}\\{CLAUDE_SETTINGS}\" %*\r\n",
            timeout = TOOL_TIMEOUT_SEC * 1000
        ),
        "codex" => {
            let overrides: String = codex_overrides(url).iter().map(|value| format!(" -c {value}")).collect();
            format!(
                "if not defined {AUTH_VAR} goto plain\r\n\
                 if not defined {MCP_FILE_VAR} goto plain\r\n\
                 if not exist \"%{MCP_FILE_VAR}%\" goto plain\r\n\
                 \"%REAL%\"{overrides} %*\r\n"
            )
        }
        _ => format!(
            "if not defined {COPILOT_FILE_VAR} goto plain\r\n\
             if not exist \"%{COPILOT_FILE_VAR}%\" goto plain\r\n\
             \"%REAL%\" --additional-mcp-config \"@%{COPILOT_FILE_VAR}%\" %*\r\n"
        ),
    };
    format!(
        "@echo off\r\n\
         rem Written by Termo, and rewritten when it starts a console: starts the real\r\n\
         rem {tool} with the user's Termo agents within reach (pty/console_shims.rs).\r\n\
         setlocal\r\n\
         set \"PATH=%PATH:{dir};=%\"\r\n\
         set \"PATH=%PATH:;{dir}=%\"\r\n\
         set \"REAL=\"\r\n\
         for %%I in ({tool}.com {tool}.exe {tool}.bat {tool}.cmd) do if not defined REAL if /i not \"%%~$PATH:I\"==\"%~f0\" set \"REAL=%%~$PATH:I\"\r\n\
         if not defined REAL goto missing\r\n\
         {with_server}\
         exit /b\r\n\
         :plain\r\n\
         \"%REAL%\" %*\r\n\
         exit /b\r\n\
         :missing\r\n\
         rem cmd.exe's own words for a command that isn't there, once.\r\n\
         if defined TERMO_CONSOLE_SHIM_MISSING exit /b 9009\r\n\
         set \"TERMO_CONSOLE_SHIM_MISSING=1\"\r\n\
         {tool} %*\r\n\
         exit /b\r\n"
    )
}

fn claude_settings() -> String {
    serde_json::json!({ "permissions": { "allow": [format!("mcp__{SERVER}__{FREE_TOOL}")] } }).to_string()
}

/// Writes `text` unless it's there already: a batch file is read as it
/// runs, so one in use is left alone when nothing changed.
fn write(path: &Path, text: &str, executable: bool) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    console_bridge::replace_file(path, text)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|err| err.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// The stand-ins, in `dir`.
fn install(dir: &Path, url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let folder = {
        let folder = dir.to_string_lossy().into_owned();
        if folder.contains(CMD_SYNTAX) {
            return Err("the folder's name can't go in a batch file".into());
        }
        folder
    };
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    write(&dir.join(CLAUDE_SETTINGS), &claude_settings(), false)?;
    for tool in TOOLS {
        write(&dir.join(tool), &sh_script(tool, url), true)?;
        #[cfg(windows)]
        write(&dir.join(format!("{tool}.cmd")), &cmd_script(tool, &folder, url), true)?;
    }
    Ok(())
}

/// Gives the shell about to start the user's agents for the CLIs typed in
/// it: its token and files in `env`, and the stand-ins ahead in its PATH.
/// Returns the token, to bind to the console once it runs — none when
/// there's nothing to give (see `shell_extras`), and the shell starts as
/// always.
pub fn apply(
    root: &Path,
    global: bool,
    shell_path: &str,
    args: &mut Vec<String>,
    env: &mut Vec<(&'static str, String)>,
    zsh_lines: &mut Vec<String>,
) -> Option<String> {
    let extras = console_bridge::shell_extras(root, global);
    let token = extras.token?;
    let dir = root.join("console-shims");
    if install(&dir, &webhook::mcp_url()).is_err() {
        console_bridge::discard(&token);
        return None;
    }
    env.extend(extras.env);
    env.push((DIR_VAR, dir.to_string_lossy().into_owned()));
    if cfg!(windows) && super::powershell_keys::is_powershell(shell_path) {
        let function = POWERSHELL_CLAUDE.replace("__TIMEOUT_MS__", &(TOOL_TIMEOUT_SEC * 1000).to_string());
        super::oh_my_posh::append_powershell_command(args, &function);
    }
    zsh_lines.push(ZSH_PATH_LINE.to_string());
    Some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "http://127.0.0.1:47821/mcp";

    /// The server's arguments go before what the user typed, so a
    /// subcommand still comes after them.
    #[test]
    fn the_servers_arguments_go_first() {
        let claude = sh_script("claude", URL);
        assert!(claude.contains(r#"exec claude --mcp-config "$TERMO_CONSOLE_MCP" --settings "$self/claude-settings.json" "$@""#));
        assert!(claude.contains("MCP_TOOL_TIMEOUT=${MCP_TOOL_TIMEOUT:-3600000}"));
        let codex = sh_script("codex", URL);
        assert!(codex.contains("exec codex -c 'mcp_servers.termo.url=http://127.0.0.1:47821/mcp'"));
        assert!(codex.contains("-c 'mcp_servers.termo.env_http_headers.Authorization=TERMO_CONSOLE_AUTH'"));
        assert!(codex.contains(r#"-c 'mcp_servers.termo.tool_timeout_sec=3600' "$@""#));
        let copilot = sh_script("copilot", URL);
        assert!(copilot.contains(r#"exec copilot --additional-mcp-config "@$TERMO_CONSOLE_COPILOT" "$@""#));
    }

    /// Without a token, or with its files gone (the console closed), the
    /// real CLI starts exactly as typed.
    #[test]
    fn without_a_token_the_cli_starts_as_typed() {
        for tool in TOOLS {
            let script = sh_script(tool, URL);
            assert!(script.starts_with("#!/bin/sh\n"));
            assert!(script.ends_with(&format!("exec {tool} \"$@\"\n")), "{tool}");
            assert!(script.contains("[ -f "), "{tool}");
            assert!(!script.contains('\r'));
        }
    }

    #[test]
    fn claude_lists_the_agents_without_asking() {
        let settings: serde_json::Value = serde_json::from_str(&claude_settings()).unwrap();
        assert_eq!(settings["permissions"]["allow"][0], "mcp__termo__list_agents");
    }

    #[cfg(windows)]
    #[test]
    fn the_batch_files_find_the_real_cli_and_never_themselves() {
        let dir = r"C:\Users\me\AppData\Roaming\com.ln.termo\agents\console-shims";
        for tool in TOOLS {
            let script = cmd_script(tool, dir, URL);
            assert!(script.contains(&format!("set \"PATH=%PATH:{dir};=%\"")));
            assert!(script.contains("if /i not \"%%~$PATH:I\"==\"%~f0\""));
            assert!(script.contains("\"%REAL%\" %*"));
            // cmd.exe wants its own line endings to find a label.
            assert_eq!(script.matches('\n').count(), script.matches("\r\n").count());
            // A folder with a parenthesis in its name would close a block.
            assert!(!script.lines().any(|line| line.trim_end().ends_with('(')));
        }
        let claude = cmd_script("claude", dir, URL);
        assert!(claude.contains(&format!("\"%REAL%\" --mcp-config \"%TERMO_CONSOLE_MCP%\" --settings \"{dir}\\claude-settings.json\" %*")));
        assert!(cmd_script("codex", dir, URL).contains("\"%REAL%\" -c mcp_servers.termo.url=http://127.0.0.1:47821/mcp -c "));
        assert!(cmd_script("copilot", dir, URL).contains("\"%REAL%\" --additional-mcp-config \"@%TERMO_CONSOLE_COPILOT%\" %*"));
    }

    #[test]
    fn the_folder_gets_every_stand_in_and_is_left_alone_when_unchanged() {
        let dir = std::env::temp_dir().join(format!("termo-shims-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install(&dir, URL).unwrap();
        for tool in TOOLS {
            assert!(dir.join(tool).is_file());
            #[cfg(windows)]
            assert!(dir.join(format!("{tool}.cmd")).is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(std::fs::metadata(dir.join(tool)).unwrap().permissions().mode() & 0o111, 0o111);
            }
        }
        let written = std::fs::metadata(dir.join("claude")).unwrap().modified().unwrap();
        install(&dir, URL).unwrap();
        assert_eq!(std::fs::metadata(dir.join("claude")).unwrap().modified().unwrap(), written);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_cmd_would_misread_gets_no_stand_ins() {
        let dir = std::env::temp_dir().join("termo-shims-a&b");
        assert!(install(&dir, URL).is_err());
        assert!(!dir.exists());
    }

    /// A double quote in here comes out mangled on the far side of a Windows
    /// command line, which is how this snippet reaches the shell.
    #[test]
    fn escapes_the_windows_command_line() {
        assert!(!POWERSHELL_CLAUDE.contains('"'));
    }
}
