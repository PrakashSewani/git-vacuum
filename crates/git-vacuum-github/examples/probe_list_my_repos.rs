//! Diagnostic tool: prints exactly what GitHub's `/user/repos` endpoint returns
//! for a given token, so we can see whether org-owned repos leak through.
//!
//! Usage:
//!   GITHUB_TOKEN=ghp_xxx cargo run --release -p git-vacuum-github --example probe_list_my_repos
//!
//! Or pass --token. The token is read from env/arg, used only for the
//! HTTP call, and never written to disk or logged. We print *which fields*
//! came back for each repo so we can see what shape Octocrab is mapping.

use std::env;

use git_vacuum_core::GithubApi as _;
use git_vacuum_github::OctocrabGithubApi;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| {
            let mut args = env::args().skip(1);
            while let Some(a) = args.next() {
                if a == "--token" {
                    return args.next();
                }
            }
            None
        })
        .ok_or("set GITHUB_TOKEN or pass --token <PAT>")?;

    let api = OctocrabGithubApi::new("https://api.github.com", "git-vacuum-probe/0.1");
    api.set_token(&token);

    println!("=== /user (validate_token) ===");
    match api.validate_token().await {
        Ok(u) => println!("  login={} id={}", u.login, u.github_user_id),
        Err(e) => {
            println!("  ERROR: {e}");
            return Err(e.into());
        }
    }

    println!("\n=== GET /user/repos?per_page=100&affiliation=owner,collaborator,organization_member ===");
    let repos = api.list_my_repos().await?;
    println!("  count = {}", repos.len());
    let mut org_owned = 0usize;
    let mut user_owned = 0usize;
    let mut unknown = 0usize;
    for r in &repos {
        // We can't see owner_is_org directly here because OctocrabGithubApi
        // already mapped it. But we can detect by owner_login:
        //   - if owner_login == authenticated user, it's personal
        //   - otherwise, it's via org membership (almost certainly)
        // Print first 20 with full detail.
        if r.full_name.contains('/') {
            let (owner, name) = r.full_name.split_once('/').unwrap();
            if owner == "you" { /* placeholder */ }
        }
    }
    // Print full table
    for (i, r) in repos.iter().take(20).enumerate() {
        println!(
            "  [{:>3}] {:50}  owner_login={:30}  ssh={}",
            i,
            r.full_name,
            r.owner_login,
            if r.clone_url_ssh.is_some() { "yes" } else { "no" }
        );
    }
    if repos.len() > 20 {
        println!("  ... ({} more)", repos.len() - 20);
    }

    // Summarize.
    let mut personal = 0usize;
    let mut via_org = 0usize;
    for r in &repos {
        if r.owner_is_org {
            via_org += 1;
        } else {
            personal += 1;
        }
    }
    println!("\n=== Summary ===");
    println!("  owner_is_org=true  (via org membership): {via_org}");
    println!("  owner_is_org=false (personal):           {personal}");
    println!("\nAll 24 (or whatever count) will be cloned by --sync.");

    Ok(())
}
