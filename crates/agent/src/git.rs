//! Operações de git. Tudo passa pelo `git` da linha de comando, para respeitar a
//! configuração, os hooks e as credenciais que o usuário já tem no WSL.

use std::path::{Path, PathBuf};
use std::process::Command;

use treehouse_protocol::*;

use crate::error::{AgentError, Result};
use crate::paths::expand;

/// Executa `git -C dir args…` e devolve o stdout. Erro se o código de saída ≠ 0.
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| AgentError::internal(format!("não foi possível executar o git: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(AgentError::git(format!("git {} falhou", args.first().unwrap_or(&"")), stderr))
    }
}

/// Como `git`, mas só informa se deu certo.
fn git_ok(dir: &Path, args: &[&str]) -> bool {
    git(dir, args).is_ok()
}

fn ensure_repo(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Err(AgentError::invalid(format!("pasta não encontrada: {}", dir.display())));
    }
    git(dir, &["rev-parse", "--git-dir"])
        .map(|_| ())
        .map_err(|_| AgentError::invalid(format!("não é um repositório git: {}", dir.display())))
}

fn has_origin(dir: &Path) -> bool {
    git_ok(dir, &["remote", "get-url", "origin"])
}

fn fetch(dir: &Path) -> Result<()> {
    if has_origin(dir) {
        git(dir, &["fetch", "origin", "--prune"])
            .map_err(|e| AgentError::git("git fetch falhou", e.detail.unwrap_or_default()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Escanear a pasta de projetos
// ---------------------------------------------------------------------------

pub fn scan(params: &ScanParams) -> Result<Vec<ScannedRepo>> {
    let dir = expand(&params.dir);
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| AgentError::invalid(format!("não foi possível ler {}: {e}", dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join(".git").exists())
        .collect();
    entries.sort();
    Ok(entries
        .into_iter()
        .map(|path| {
            let remote = git(&path, &["remote", "get-url", "origin"]).ok().map(|s| s.trim().to_string());
            ScannedRepo {
                name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                default_base: default_base(&path),
                remote,
                path: path.to_string_lossy().into_owned(),
            }
        })
        .collect())
}

fn default_base(repo: &Path) -> Option<String> {
    if let Ok(s) = git(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
        let s = s.trim();
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    for candidate in ["origin/main", "origin/master", "main", "master"] {
        if git_ok(repo, &["rev-parse", "--verify", "--quiet", candidate]) {
            return Some(candidate.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Criar / arquivar features
// ---------------------------------------------------------------------------

/// Cria uma worktree por repositório em `root/<repo>`. Se algum repositório falhar,
/// desfaz as worktrees já criadas: a feature nunca fica pela metade.
pub fn create_feature(p: &CreateFeatureParams) -> Result<Vec<CreatedWorktree>> {
    if p.repos.is_empty() {
        return Err(AgentError::invalid("escolha ao menos um repositório"));
    }
    validate_branch(&p.branch)?;
    let root = expand(&p.root);
    let root_existed = root.exists();
    std::fs::create_dir_all(&root)?;

    let mut created: Vec<(PathBuf, CreatedWorktree)> = Vec::new();
    for repo in &p.repos {
        match add_one(&root, &p.branch, repo, p.fetch) {
            Ok(c) => created.push((expand(&repo.clone_path), c)),
            Err(e) => {
                rollback(&created);
                if !root_existed {
                    let _ = std::fs::remove_dir(&root);
                }
                return Err(AgentError {
                    message: format!("{} (repositório {}); nada foi criado", e.message, repo.name),
                    ..e
                });
            }
        }
    }
    Ok(created.into_iter().map(|(_, c)| c).collect())
}

pub fn add_repo(p: &AddRepoParams) -> Result<CreatedWorktree> {
    validate_branch(&p.branch)?;
    let root = expand(&p.root);
    std::fs::create_dir_all(&root)?;
    add_one(&root, &p.branch, &p.repo, p.fetch)
}

fn validate_branch(branch: &str) -> Result<()> {
    let ok = !branch.is_empty()
        && Command::new("git")
            .args(["check-ref-format", "--branch", branch])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(AgentError::invalid(format!("nome de branch inválido: {branch}")))
    }
}

fn add_one(root: &Path, branch: &str, repo: &RepoSpec, do_fetch: bool) -> Result<CreatedWorktree> {
    let clone = expand(&repo.clone_path);
    ensure_repo(&clone)?;
    let target = root.join(&repo.name);
    if target.exists() {
        return Err(AgentError::new(
            codes::PATH_EXISTS,
            format!("a pasta já existe: {}", target.display()),
        ));
    }
    if do_fetch {
        fetch(&clone)?;
    }
    let target_s = target.to_string_lossy().into_owned();
    let exists = git_ok(&clone, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]);
    if exists {
        git(&clone, &["worktree", "add", &target_s, branch])?;
    } else {
        git(&clone, &["worktree", "add", "--no-track", "-b", branch, &target_s, &repo.base])?;
    }
    Ok(CreatedWorktree {
        repo: repo.name.clone(),
        path: target_s,
        branch: branch.to_string(),
        base: repo.base.clone(),
        reused_branch: exists,
    })
}

fn rollback(created: &[(PathBuf, CreatedWorktree)]) {
    for (clone, c) in created.iter().rev() {
        let _ = git(clone, &["worktree", "remove", "--force", &c.path]);
        if !c.reused_branch {
            let _ = git(clone, &["branch", "-D", &c.branch]);
        }
    }
}

pub fn archive(p: &ArchiveParams) -> Result<ArchiveResult> {
    // Primeiro confere tudo; só depois remove (nada de arquivar pela metade).
    if !p.force {
        for r in &p.repos {
            let path = expand(&r.path);
            if path.exists() {
                let st = status(&StatusParams { path: r.path.clone(), base: None })?;
                if !st.changes.is_empty() {
                    return Err(AgentError::new(
                        codes::DIRTY_WORKTREE,
                        format!("{} tem {} alterações não commitadas", r.path, st.changes.len()),
                    ));
                }
            }
        }
    }
    let mut res = ArchiveResult::default();
    for r in &p.repos {
        let clone = expand(&r.clone_path);
        let path = expand(&r.path);
        let path_s = path.to_string_lossy().into_owned();
        if path.exists() {
            let mut args = vec!["worktree", "remove"];
            if p.force {
                args.push("--force");
            }
            args.push(&path_s);
            git(&clone, &args)?;
            res.removed.push(path_s.clone());
        } else {
            let _ = git(&clone, &["worktree", "prune"]);
            res.warnings.push(format!("{path_s} já não existia; registro limpo"));
        }
        if p.delete_branch {
            let flag = if p.force { "-D" } else { "-d" };
            match git(&clone, &["branch", flag, &r.branch]) {
                Ok(_) => res.deleted_branches.push(r.branch.clone()),
                Err(e) => res.warnings.push(format!(
                    "branch {} mantida: {}",
                    r.branch,
                    e.detail.unwrap_or(e.message)
                )),
            }
        }
    }
    if let Some(ws) = &p.workspace_file {
        let _ = std::fs::remove_file(expand(ws));
    }
    let root = expand(&p.root);
    if root.is_dir() && std::fs::read_dir(&root).map(|mut d| d.next().is_none()).unwrap_or(false) {
        let _ = std::fs::remove_dir(&root);
    }
    Ok(res)
}

// ---------------------------------------------------------------------------
// Consultas
// ---------------------------------------------------------------------------

pub fn worktree_list(p: &WorktreeListParams) -> Result<Vec<WorktreeEntry>> {
    let clone = expand(&p.clone_path);
    let out = git(&clone, &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_list(&out))
}

pub fn parse_worktree_list(out: &str) -> Vec<WorktreeEntry> {
    let mut list = Vec::new();
    let mut cur: Option<WorktreeEntry> = None;
    for line in out.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(e) = cur.take() {
                list.push(e);
            }
            continue;
        }
        if let Some(p) = line.strip_prefix("worktree ") {
            cur = Some(WorktreeEntry { path: p.into(), head: None, branch: None, bare: false, detached: false });
        } else if let Some(e) = cur.as_mut() {
            if let Some(h) = line.strip_prefix("HEAD ") {
                e.head = Some(h.into());
            } else if let Some(b) = line.strip_prefix("branch ") {
                e.branch = Some(b.trim_start_matches("refs/heads/").into());
            } else if line == "bare" {
                e.bare = true;
            } else if line == "detached" {
                e.detached = true;
            }
        }
    }
    list
}

pub fn status(p: &StatusParams) -> Result<RepoStatus> {
    let path = expand(&p.path);
    let out = git(&path, &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"])?;
    let mut st = parse_status(&out);
    st.path = path.to_string_lossy().into_owned();
    if let Some(base) = &p.base {
        if let Ok(counts) = git(&path, &["rev-list", "--left-right", "--count", &format!("{base}...HEAD")]) {
            let mut it = counts.split_whitespace();
            st.behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            st.ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }
    st.rebasing = in_progress(&path).is_some();
    Ok(st)
}

/// Interpreta `git status --porcelain=v2 --branch -z`.
pub fn parse_status(out: &str) -> RepoStatus {
    let mut st = RepoStatus::default();
    let mut fields = out.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = fields.next() {
        let kind = entry.as_bytes()[0];
        match kind {
            b'#' => {
                if let Some(h) = entry.strip_prefix("# branch.head ") {
                    if h != "(detached)" {
                        st.branch = Some(h.to_string());
                    }
                }
            }
            b'1' => {
                let parts: Vec<&str> = entry.splitn(9, ' ').collect();
                if parts.len() == 9 {
                    push_change(&mut st, parts[1], parts[8]);
                }
            }
            b'2' => {
                let parts: Vec<&str> = entry.splitn(10, ' ').collect();
                let _orig = fields.next(); // caminho original vem no campo seguinte
                if parts.len() == 10 {
                    push_change(&mut st, parts[1], parts[9]);
                }
            }
            b'u' => {
                let parts: Vec<&str> = entry.splitn(11, ' ').collect();
                if parts.len() == 11 {
                    st.changes.push(FileChange { code: "U".into(), path: parts[10].into(), staged: false });
                }
            }
            b'?' => {
                st.changes.push(FileChange { code: "?".into(), path: entry[2..].into(), staged: false });
            }
            _ => {}
        }
    }
    st
}

fn push_change(st: &mut RepoStatus, xy: &str, path: &str) {
    let mut c = xy.chars();
    let x = c.next().unwrap_or('.');
    let y = c.next().unwrap_or('.');
    let raw = if y != '.' { y } else { x };
    let code = match raw {
        'A' => "A",
        'D' => "D",
        'R' | 'C' => "R",
        _ => "M",
    };
    st.changes.push(FileChange { code: code.into(), path: path.into(), staged: x != '.' && y == '.' });
}

enum InProgress {
    Rebase,
    Merge,
}

fn in_progress(path: &Path) -> Option<InProgress> {
    let gp = |name: &str| -> Option<PathBuf> {
        git(path, &["rev-parse", "--git-path", name]).ok().map(|s| {
            let p = PathBuf::from(s.trim());
            if p.is_absolute() { p } else { path.join(p) }
        })
    };
    if gp("rebase-merge").map(|p| p.exists()).unwrap_or(false)
        || gp("rebase-apply").map(|p| p.exists()).unwrap_or(false)
    {
        return Some(InProgress::Rebase);
    }
    if gp("MERGE_HEAD").map(|p| p.exists()).unwrap_or(false) {
        return Some(InProgress::Merge);
    }
    None
}

pub fn log(p: &LogParams) -> Result<Vec<Commit>> {
    let path = expand(&p.path);
    let fmt = "--format=%h%x1f%s%x1f%cr";
    let limit = format!("-n{}", p.limit.max(1));
    let mut commits = parse_log(&git(&path, &["log", fmt, &limit, &format!("{}..HEAD", p.base)])?, true);
    if let Ok(mb) = git(&path, &["merge-base", "HEAD", &p.base]) {
        let base_commits = git(&path, &["log", fmt, "-n3", mb.trim()]).unwrap_or_default();
        commits.extend(parse_log(&base_commits, false));
    }
    Ok(commits)
}

fn parse_log(out: &str, in_feature: bool) -> Vec<Commit> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split('\x1f');
            Some(Commit {
                hash: it.next()?.into(),
                subject: it.next()?.into(),
                when: it.next().unwrap_or_default().into(),
                in_feature,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Atualizar da base
// ---------------------------------------------------------------------------

pub fn rebase(p: &RebaseParams) -> Result<RebaseResult> {
    let path = expand(&p.path);
    if p.fetch {
        fetch(&path)?;
    }
    if !p.autostash {
        let st = status(&StatusParams { path: p.path.clone(), base: None })?;
        if st.changes.iter().any(|c| c.code != "?") {
            return Err(AgentError::new(
                codes::DIRTY_WORKTREE,
                "há alterações não commitadas; faça commit ou use o stash automático",
            ));
        }
    }
    let mut args: Vec<&str> = match p.strategy {
        UpdateStrategy::Rebase => vec!["rebase"],
        UpdateStrategy::Merge => vec!["merge", "--no-edit"],
    };
    if p.autostash {
        args.push("--autostash");
    }
    args.push(&p.base);
    match git(&path, &args) {
        Ok(out) => Ok(RebaseResult { ok: true, conflicts: vec![], message: out.trim().to_string() }),
        Err(e) => {
            let conflicts: Vec<String> = git(&path, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect();
            if conflicts.is_empty() && in_progress(&path).is_none() {
                return Err(e);
            }
            Ok(RebaseResult {
                ok: false,
                conflicts,
                message: e.detail.unwrap_or(e.message),
            })
        }
    }
}

pub fn rebase_abort(p: &PathParams) -> Result<()> {
    let path = expand(&p.path);
    match in_progress(&path) {
        Some(InProgress::Rebase) => git(&path, &["rebase", "--abort"]).map(|_| ()),
        Some(InProgress::Merge) => git(&path, &["merge", "--abort"]).map(|_| ()),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// GitHub (via gh)
// ---------------------------------------------------------------------------

const PR_FIELDS: &str =
    "number,title,headRefName,baseRefName,state,isDraft,reviewDecision,statusCheckRollup,url,author";

pub fn gh_prs(p: &GhPrsParams) -> Result<serde_json::Value> {
    let path = expand(&p.path);
    let run = |extra: &[&str]| -> Result<serde_json::Value> {
        let out = Command::new("gh")
            .current_dir(&path)
            .args(["pr", "list", "--json", PR_FIELDS, "--limit", "50"])
            .args(extra)
            .output()
            .map_err(|e| AgentError::internal(format!("gh não encontrado no WSL: {e}")))?;
        if !out.status.success() {
            return Err(AgentError::git("gh pr list falhou", String::from_utf8_lossy(&out.stderr)));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| AgentError::internal(e.to_string()))
    };
    Ok(serde_json::json!({
        "mine": run(&["--author", "@me"])?,
        "reviewRequested": run(&["--search", "review-requested:@me"])?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_v2() {
        let out = "# branch.oid abc\0# branch.head feature/x\0\
1 .M N... 100644 100644 100644 aaa bbb src/a b.ts\0\
1 A. N... 000000 100644 100644 000 ccc src/new.ts\0\
2 R. N... 100644 100644 100644 ddd eee R100 src/renamed.ts\0src/old.ts\0\
u UU N... 100644 100644 100644 100644 f1 f2 f3 src/conflict.ts\0\
? notes.txt\0";
        let st = parse_status(out);
        assert_eq!(st.branch.as_deref(), Some("feature/x"));
        let got: Vec<(&str, &str, bool)> =
            st.changes.iter().map(|c| (c.code.as_str(), c.path.as_str(), c.staged)).collect();
        assert_eq!(
            got,
            vec![
                ("M", "src/a b.ts", false),
                ("A", "src/new.ts", true),
                ("R", "src/renamed.ts", true),
                ("U", "src/conflict.ts", false),
                ("?", "notes.txt", false),
            ]
        );
    }

    #[test]
    fn parses_worktree_list() {
        let out = "worktree /code/api\nHEAD 111\nbranch refs/heads/main\n\n\
worktree /wt/pay-412/api\nHEAD 222\nbranch refs/heads/feature/PAY-412\n\n\
worktree /wt/review\nHEAD 333\ndetached\n";
        let l = parse_worktree_list(out);
        assert_eq!(l.len(), 3);
        assert_eq!(l[1].branch.as_deref(), Some("feature/PAY-412"));
        assert!(l[2].detached);
    }
}
