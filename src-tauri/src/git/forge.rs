//! Where a repository is hosted, read off its remote's URL: which service
//! (for its icon and for which CLI knows its pull requests), and the page to
//! open in a browser.

use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ForgeKind {
    Github,
    Gitlab,
    Bitbucket,
    Azure,
    /// Gitea and Forgejo (Codeberg among them).
    Gitea,
    Generic,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Forge {
    pub kind: ForgeKind,
    pub host: String,
    /// Who it belongs to: a user, an organization, or a group with its
    /// subgroups (`group/sub`).
    pub owner: String,
    pub repo: String,
    /// The repository's page, when the URL says enough to know it.
    pub web_url: Option<String>,
}

fn kind_of(host: &str) -> ForgeKind {
    let host = host.to_ascii_lowercase();
    if host.contains("github") {
        ForgeKind::Github
    } else if host.contains("gitlab") {
        ForgeKind::Gitlab
    } else if host.contains("bitbucket") {
        ForgeKind::Bitbucket
    } else if host.ends_with("dev.azure.com") || host.ends_with("visualstudio.com") {
        ForgeKind::Azure
    } else if host == "codeberg.org" || host.contains("gitea") || host.contains("forgejo") {
        ForgeKind::Gitea
    } else {
        ForgeKind::Generic
    }
}

/// The URL without the password (or token) someone saved in it: what's shown
/// in the panel, never the secret.
pub fn redact(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (authority, tail) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    match authority.rsplit_once('@') {
        Some((user, host)) => {
            let user = user.split(':').next().unwrap_or_default();
            // A lone token in the user's place is a secret too.
            if user.is_empty() || user.len() > 30 {
                format!("{scheme}://{host}{tail}")
            } else {
                format!("{scheme}://{user}@{host}{tail}")
            }
        }
        None => url.to_string(),
    }
}

/// `(host, path, over http)` out of a remote's URL, or `None` for a local
/// path.
fn split(url: &str) -> Option<(String, String, bool)> {
    let url = url.trim();
    if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https" | "ssh" | "git" | "git+ssh") {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
        // The port isn't part of the page's address.
        let host = host.split(':').next().unwrap_or_default();
        return Some((host.to_string(), path.to_string(), scheme.starts_with("http")));
    }
    // scp-like: `git@host:owner/repo.git`. A drive letter (`C:\repo`) is a path.
    let (before, path) = url.split_once(':')?;
    if before.len() < 2 || before.contains('/') || before.contains('\\') || path.starts_with('\\') {
        return None;
    }
    let host = before.rsplit_once('@').map_or(before, |(_, host)| host);
    Some((host.to_string(), path.to_string(), false))
}

pub fn parse(url: &str) -> Option<Forge> {
    let (host, path, http) = split(url)?;
    if host.is_empty() {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let kind = kind_of(&host);

    if kind == ForgeKind::Azure {
        // https://dev.azure.com/org/project/_git/repo, org.visualstudio.com/
        // project/_git/repo, and over ssh v3/org/project/repo.
        parts.retain(|part| *part != "_git");
        if parts.first() == Some(&"v3") {
            parts.remove(0);
        }
        let repo = parts.pop()?.to_string();
        let owner = parts.join("/");
        if owner.is_empty() {
            return None;
        }
        let web_host = if host.ends_with("visualstudio.com") { host.clone() } else { "dev.azure.com".to_string() };
        let (base, project) = match owner.rsplit_once('/') {
            Some((org, project)) if web_host == "dev.azure.com" => (format!("{web_host}/{org}"), project.to_string()),
            _ => (web_host.clone(), owner.rsplit('/').next().unwrap_or(&owner).to_string()),
        };
        let web_url = Some(format!("https://{base}/{project}/_git/{repo}"));
        return Some(Forge { kind, host, owner, repo, web_url });
    }

    let repo = parts.pop()?.to_string();
    let owner = parts.join("/");
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    // A host nothing is known about, reached over ssh: its pages may be
    // anywhere, or nowhere.
    let web_url = (kind != ForgeKind::Generic || http).then(|| format!("https://{host}/{owner}/{repo}"));
    Some(Forge { kind, host, owner, repo, web_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge(url: &str) -> Forge {
        parse(url).unwrap_or_else(|| panic!("{url} wasn't recognized"))
    }

    #[test]
    fn the_big_hosts_are_told_apart_over_https_and_ssh() {
        let github = forge("https://github.com/nicoriff/termo.git");
        assert_eq!((github.kind, github.owner.as_str(), github.repo.as_str()), (ForgeKind::Github, "nicoriff", "termo"));
        assert_eq!(github.web_url.as_deref(), Some("https://github.com/nicoriff/termo"));
        assert_eq!(forge("git@github.com:nicoriff/termo.git"), github);
        assert_eq!(forge("ssh://git@github.com/nicoriff/termo"), github);

        assert_eq!(forge("git@gitlab.com:group/sub/app.git").kind, ForgeKind::Gitlab);
        assert_eq!(forge("git@gitlab.com:group/sub/app.git").owner, "group/sub");
        assert_eq!(forge("https://bitbucket.org/team/app.git").kind, ForgeKind::Bitbucket);
        assert_eq!(forge("https://codeberg.org/someone/app").kind, ForgeKind::Gitea);
    }

    #[test]
    fn a_companys_own_host_is_known_by_its_name() {
        let own = forge("https://gitlab.miempresa.com:8443/equipo/app.git");
        assert_eq!(own.kind, ForgeKind::Gitlab);
        assert_eq!(own.web_url.as_deref(), Some("https://gitlab.miempresa.com/equipo/app"));
        assert_eq!(forge("git@github.acme.io:org/app.git").kind, ForgeKind::Github);
    }

    #[test]
    fn azure_devops_in_its_three_shapes() {
        let https = forge("https://org@dev.azure.com/org/project/_git/app");
        assert_eq!((https.kind, https.owner.as_str(), https.repo.as_str()), (ForgeKind::Azure, "org/project", "app"));
        assert_eq!(https.web_url.as_deref(), Some("https://dev.azure.com/org/project/_git/app"));
        let ssh = forge("git@ssh.dev.azure.com:v3/org/project/app");
        assert_eq!(ssh.web_url, https.web_url);
        let old = forge("https://org.visualstudio.com/project/_git/app");
        assert_eq!(old.web_url.as_deref(), Some("https://org.visualstudio.com/project/_git/app"));
    }

    #[test]
    fn an_unknown_host_is_generic_with_a_page_only_over_http() {
        let http = forge("https://git.example.org/someone/app.git");
        assert_eq!(http.kind, ForgeKind::Generic);
        assert!(http.web_url.is_some());
        assert!(forge("git@git.example.org:someone/app.git").web_url.is_none());
    }

    #[test]
    fn local_paths_are_not_remotes_on_a_host() {
        assert!(parse(r"C:\repos\app.git").is_none());
        assert!(parse("/srv/git/app.git").is_none());
        assert!(parse("../app").is_none());
        assert!(parse("file:///srv/git/app.git").is_none());
    }

    #[test]
    fn a_saved_password_never_shows() {
        assert_eq!(redact("https://nico:ghp_secret@github.com/a/b.git"), "https://nico@github.com/a/b.git");
        assert_eq!(redact("https://github.com/a/b.git"), "https://github.com/a/b.git");
        assert_eq!(redact("git@github.com:a/b.git"), "git@github.com:a/b.git");
        let token = format!("https://{}@github.com/a/b.git", "x".repeat(40));
        assert_eq!(redact(&token), "https://github.com/a/b.git");
    }
}
