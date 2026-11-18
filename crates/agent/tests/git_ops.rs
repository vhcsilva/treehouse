//! Testes com repositórios git reais em pastas temporárias.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use treehouse_agent::git;
use treehouse_protocol::*;

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn identity() {
    for (k, v) in [
        ("GIT_AUTHOR_NAME", "Teste"),
        ("GIT_AUTHOR_EMAIL", "teste@example.com"),
        ("GIT_COMMITTER_NAME", "Teste"),
        ("GIT_COMMITTER_EMAIL", "teste@example.com"),
    ] {
        std::env::set_var(k, v);
    }
}

fn commit(dir: &Path, file: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(file), content).unwrap();
    sh(dir, &["add", file]);
    sh(dir, &["commit", "-q", "-m", msg]);
}

/// Cria um "remoto" (bare) com `main` e `develop` e um clone dele em `code/<name>`.
fn setup_repo(tmp: &Path, name: &str) -> PathBuf {
    identity();
    let seed = tmp.join(format!("seed-{name}"));
    std::fs::create_dir_all(&seed).unwrap();
    sh(&seed, &["init", "-q", "-b", "main"]);
    commit(&seed, "README.md", "olá\n", "inicial");
    sh(&seed, &["branch", "develop"]);
    let remote = tmp.join(format!("{name}.git"));
    sh(tmp, &["clone", "-q", "--bare", seed.to_str().unwrap(), remote.to_str().unwrap()]);
    let code = tmp.join("code");
    std::fs::create_dir_all(&code).unwrap();
    let clone = code.join(name);
    sh(tmp, &["clone", "-q", remote.to_str().unwrap(), clone.to_str().unwrap()]);
    clone
}

fn spec(clone: &Path, name: &str, base: &str) -> RepoSpec {
    RepoSpec { name: name.into(), clone_path: clone.to_string_lossy().into(), base: base.into(), branch: None }
}

/// Empurra um commit novo para `develop` no remoto (simula um colega).
fn advance_remote(tmp: &Path, name: &str, file: &str, content: &str) {
    let other = tmp.join(format!("other-{name}-{file}"));
    sh(tmp, &["clone", "-q", "-b", "develop", tmp.join(format!("{name}.git")).to_str().unwrap(), other.to_str().unwrap()]);
    commit(&other, file, content, &format!("muda {file}"));
    sh(&other, &["push", "-q", "origin", "develop"]);
}

#[test]
fn scan_finds_repos_and_default_base() {
    let tmp = TempDir::new().unwrap();
    setup_repo(tmp.path(), "api");
    setup_repo(tmp.path(), "web");
    std::fs::create_dir_all(tmp.path().join("code/nao-e-repo")).unwrap();
    let repos = git::scan(&ScanParams { dir: tmp.path().join("code").to_string_lossy().into() }).unwrap();
    let names: Vec<_> = repos.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["api", "web"]);
    assert_eq!(repos[0].default_base.as_deref(), Some("origin/main"));
    assert!(repos[0].remote.as_deref().unwrap().ends_with("api.git"));
}

#[test]
fn create_single_repo_feature() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/pay-412");
    let created = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/PAY-412-pix".into(),
        repos: vec![spec(&api, "api", "origin/develop")],
        fetch: true,
    })
    .unwrap();
    assert_eq!(created.len(), 1);
    assert!(!created[0].reused_branch);
    let wt = root.join("api");
    assert!(wt.join("README.md").exists());
    let st = git::status(&StatusParams { path: wt.to_string_lossy().into(), base: Some("origin/develop".into()) })
        .unwrap();
    assert_eq!(st.branch.as_deref(), Some("feature/PAY-412-pix"));
    assert_eq!((st.ahead, st.behind), (0, 0));
    assert!(st.changes.is_empty());
    // Sem upstream configurado (--no-track): um push por engano não vai para develop.
    let upstream = Command::new("git").arg("-C").arg(&wt).args(["rev-parse", "--abbrev-ref", "@{u}"]).output().unwrap();
    assert!(!upstream.status.success());
    // Aparece na lista de worktrees do clone.
    let list = git::worktree_list(&WorktreeListParams { clone_path: api.to_string_lossy().into() }).unwrap();
    assert!(list.iter().any(|e| e.branch.as_deref() == Some("feature/PAY-412-pix")));
}

#[test]
fn multi_repo_failure_rolls_back_everything() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let web = setup_repo(tmp.path(), "web");
    let root = tmp.path().join("wt/pay-500");
    let err = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/PAY-500".into(),
        repos: vec![spec(&api, "api", "origin/develop"), spec(&web, "web", "origin/nao-existe")],
        fetch: false,
    })
    .unwrap_err();
    assert_eq!(err.code, codes::GIT_FAILED);
    assert!(err.message.contains("web"), "{}", err.message);
    assert!(!root.exists(), "a pasta da feature deveria ter sido removida");
    let branches = sh(&api, &["branch", "--list", "feature/PAY-500"]);
    assert!(branches.trim().is_empty(), "a branch criada no api deveria ter sido apagada");
}

#[test]
fn reuses_existing_branch() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    sh(&api, &["branch", "feature/existente", "origin/main"]);
    let root = tmp.path().join("wt/existente");
    let created = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/existente".into(),
        repos: vec![spec(&api, "api", "origin/develop")],
        fetch: false,
    })
    .unwrap();
    assert!(created[0].reused_branch);
}

#[test]
fn rejects_invalid_branch_and_existing_path() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/x");
    let bad = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/com espaço".into(),
        repos: vec![spec(&api, "api", "origin/main")],
        fetch: false,
    })
    .unwrap_err();
    assert_eq!(bad.code, codes::INVALID_PARAMS);

    std::fs::create_dir_all(root.join("api")).unwrap();
    let exists = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/x".into(),
        repos: vec![spec(&api, "api", "origin/main")],
        fetch: false,
    })
    .unwrap_err();
    assert_eq!(exists.code, codes::PATH_EXISTS);
}

#[test]
fn status_counts_changes_and_behind() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/pay-1");
    git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/pay-1".into(),
        repos: vec![spec(&api, "api", "origin/develop")],
        fetch: true,
    })
    .unwrap();
    let wt = root.join("api");
    commit(&wt, "feature.txt", "x\n", "feat: algo");
    std::fs::write(wt.join("README.md"), "mudou\n").unwrap();
    std::fs::write(wt.join("novo.txt"), "?\n").unwrap();
    advance_remote(tmp.path(), "api", "outro.txt", "do colega\n");
    sh(&api, &["fetch", "-q", "origin"]);

    let st = git::status(&StatusParams { path: wt.to_string_lossy().into(), base: Some("origin/develop".into()) })
        .unwrap();
    assert_eq!((st.ahead, st.behind), (1, 1));
    let codes: Vec<_> = st.changes.iter().map(|c| (c.code.as_str(), c.path.as_str())).collect();
    assert!(codes.contains(&("M", "README.md")));
    assert!(codes.contains(&("?", "novo.txt")));

    let log = git::log(&LogParams { path: wt.to_string_lossy().into(), base: "origin/develop".into(), limit: 10 })
        .unwrap();
    assert_eq!(log[0].subject, "feat: algo");
    assert!(log[0].in_feature);
    assert!(log.iter().any(|c| !c.in_feature));
}

#[test]
fn rebase_clean_and_with_conflict() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/pay-2");
    git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/pay-2".into(),
        repos: vec![spec(&api, "api", "origin/develop")],
        fetch: true,
    })
    .unwrap();
    let wt = root.join("api");
    let wt_s: String = wt.to_string_lossy().into();
    commit(&wt, "minha.txt", "1\n", "feat: minha");

    // Sujo e sem autostash: recusa.
    std::fs::write(wt.join("minha.txt"), "sujo\n").unwrap();
    let dirty = git::rebase(&RebaseParams {
        path: wt_s.clone(),
        base: "origin/develop".into(),
        fetch: false,
        autostash: false,
        strategy: UpdateStrategy::Rebase,
    })
    .unwrap_err();
    assert_eq!(dirty.code, codes::DIRTY_WORKTREE);
    sh(&wt, &["checkout", "--", "minha.txt"]);

    // Sem conflito.
    advance_remote(tmp.path(), "api", "deles.txt", "a\n");
    let ok = git::rebase(&RebaseParams {
        path: wt_s.clone(),
        base: "origin/develop".into(),
        fetch: true,
        autostash: false,
        strategy: UpdateStrategy::Rebase,
    })
    .unwrap();
    assert!(ok.ok, "{}", ok.message);
    let st = git::status(&StatusParams { path: wt_s.clone(), base: Some("origin/develop".into()) }).unwrap();
    assert_eq!((st.ahead, st.behind), (1, 0));

    // Com conflito: para, lista os arquivos e permite abortar.
    commit(&wt, "README.md", "versão da feature\n", "feat: readme");
    advance_remote(tmp.path(), "api", "README.md", "versão do colega\n");
    let conflict = git::rebase(&RebaseParams {
        path: wt_s.clone(),
        base: "origin/develop".into(),
        fetch: true,
        autostash: false,
        strategy: UpdateStrategy::Rebase,
    })
    .unwrap();
    assert!(!conflict.ok);
    assert_eq!(conflict.conflicts, vec!["README.md".to_string()]);
    assert!(git::status(&StatusParams { path: wt_s.clone(), base: None }).unwrap().rebasing);
    git::rebase_abort(&PathParams { path: wt_s.clone() }).unwrap();
    let after = git::status(&StatusParams { path: wt_s.clone(), base: None }).unwrap();
    assert!(!after.rebasing);
    assert_eq!(std::fs::read_to_string(wt.join("README.md")).unwrap(), "versão da feature\n");
}

#[test]
fn merge_strategy_works() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/m");
    git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/m".into(),
        repos: vec![spec(&api, "api", "origin/develop")],
        fetch: true,
    })
    .unwrap();
    let wt = root.join("api");
    commit(&wt, "a.txt", "a\n", "feat: a");
    advance_remote(tmp.path(), "api", "b.txt", "b\n");
    let r = git::rebase(&RebaseParams {
        path: wt.to_string_lossy().into(),
        base: "origin/develop".into(),
        fetch: true,
        autostash: false,
        strategy: UpdateStrategy::Merge,
    })
    .unwrap();
    assert!(r.ok, "{}", r.message);
    assert!(wt.join("b.txt").exists());
}

#[test]
fn archive_refuses_dirty_then_forces() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let web = setup_repo(tmp.path(), "web");
    let root = tmp.path().join("wt/pay-3");
    let ws_file = tmp.path().join("wt/pay-3.code-workspace");
    std::fs::create_dir_all(tmp.path().join("wt")).unwrap();
    std::fs::write(&ws_file, "{}").unwrap();
    git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/pay-3".into(),
        repos: vec![spec(&api, "api", "origin/develop"), spec(&web, "web", "origin/main")],
        fetch: false,
    })
    .unwrap();
    std::fs::write(root.join("web/sujo.txt"), "x").unwrap();
    let repos = vec![
        ArchiveRepo {
            clone_path: api.to_string_lossy().into(),
            path: root.join("api").to_string_lossy().into(),
            branch: "feature/pay-3".into(),
        },
        ArchiveRepo {
            clone_path: web.to_string_lossy().into(),
            path: root.join("web").to_string_lossy().into(),
            branch: "feature/pay-3".into(),
        },
    ];
    let mut p = ArchiveParams {
        root: root.to_string_lossy().into(),
        repos,
        delete_branch: true,
        force: false,
        workspace_file: Some(ws_file.to_string_lossy().into()),
    };
    let err = git::archive(&p).unwrap_err();
    assert_eq!(err.code, codes::DIRTY_WORKTREE);
    // Nada foi removido.
    assert!(root.join("api").exists() && root.join("web").exists());

    p.force = true;
    let res = git::archive(&p).unwrap();
    assert_eq!(res.removed.len(), 2);
    assert_eq!(res.deleted_branches.len(), 2);
    assert!(!root.exists());
    assert!(!ws_file.exists());
    assert!(sh(&api, &["branch", "--list", "feature/pay-3"]).trim().is_empty());
}

#[test]
fn archive_keeps_unmerged_branch_without_force() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let root = tmp.path().join("wt/pay-4");
    git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: "feature/pay-4".into(),
        repos: vec![spec(&api, "api", "origin/main")],
        fetch: false,
    })
    .unwrap();
    commit(&root.join("api"), "x.txt", "x", "feat: x");
    let res = git::archive(&ArchiveParams {
        root: root.to_string_lossy().into(),
        repos: vec![ArchiveRepo {
            clone_path: api.to_string_lossy().into(),
            path: root.join("api").to_string_lossy().into(),
            branch: "feature/pay-4".into(),
        }],
        delete_branch: true,
        force: false,
        workspace_file: None,
    })
    .unwrap();
    assert_eq!(res.removed.len(), 1);
    assert!(res.deleted_branches.is_empty());
    assert_eq!(res.warnings.len(), 1, "{:?}", res.warnings);
    assert!(!sh(&api, &["branch", "--list", "feature/pay-4"]).trim().is_empty());
}

#[test]
fn each_repo_can_have_its_own_branch() {
    let tmp = TempDir::new().unwrap();
    let api = setup_repo(tmp.path(), "api");
    let web = setup_repo(tmp.path(), "web");
    let root = tmp.path().join("wt/pay-7");
    let mut api_spec = spec(&api, "api", "origin/develop");
    api_spec.branch = Some("feat/api-PAY-7-pix".into());
    let mut web_spec = spec(&web, "web", "origin/main");
    web_spec.branch = Some("feature/web/PAY-7-pix".into());
    let created = git::create_feature(&CreateFeatureParams {
        root: root.to_string_lossy().into(),
        branch: String::new(),
        repos: vec![api_spec, web_spec.clone()],
        fetch: false,
    })
    .unwrap();
    assert_eq!(created[0].branch, "feat/api-PAY-7-pix");
    assert_eq!(created[1].branch, "feature/web/PAY-7-pix");
    let st = git::status(&StatusParams { path: root.join("web").to_string_lossy().into(), base: None }).unwrap();
    assert_eq!(st.branch.as_deref(), Some("feature/web/PAY-7-pix"));

    // Uma branch inválida em qualquer repo: nada é criado.
    let mut bad = spec(&api, "api", "origin/main");
    bad.branch = Some("feat/com espaço".into());
    let err = git::create_feature(&CreateFeatureParams {
        root: tmp.path().join("wt/pay-8").to_string_lossy().into(),
        branch: String::new(),
        repos: vec![spec(&web, "web", "origin/main"), bad],
        fetch: false,
    })
    .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
    assert!(!tmp.path().join("wt/pay-8").exists());
}
