//! Tipos do protocolo JSON-RPC 2.0 entre o app Treehouse (Windows) e o
//! `treehouse-agent` (WSL). Uma mensagem por linha (JSON delimitado por `\n`).
//!
//! Mantenha `app/src/lib/protocol.ts` em sincronia com este arquivo.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versão do protocolo. O app recusa agentes com versão diferente e os substitui.
pub const PROTOCOL_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Envelope JSON-RPC
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    /// Sem `id` (ou `null`), é uma notificação JSON-RPC: o agente não responde.
    /// O app usa isso nas teclas do terminal, para não esperar ida e volta.
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn err(id: Value, error: RpcError) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None, error: Some(error) }
    }
}

/// Mensagem enviada pelo agente sem pedido (saída de terminal, mudanças no git…).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl Notification {
    pub fn new(method: &str, params: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params: serde_json::to_value(params).unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Códigos de erro do Treehouse (faixa reservada para aplicações: -32000 a -32099).
pub mod codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    /// Um comando git falhou.
    pub const GIT_FAILED: i64 = -32001;
    /// A worktree tem alterações não commitadas e a operação não foi forçada.
    pub const DIRTY_WORKTREE: i64 = -32002;
    /// Terminal inexistente.
    pub const NO_SUCH_TERMINAL: i64 = -32003;
    /// Caminho já existe.
    pub const PATH_EXISTS: i64 = -32004;
}

// ---------------------------------------------------------------------------
// Métodos (nomes)
// ---------------------------------------------------------------------------

pub mod methods {
    pub const VERSION: &str = "agent.version";
    pub const SHUTDOWN: &str = "agent.shutdown";
    pub const WORKSPACE_SCAN: &str = "workspace.scan";
    pub const FEATURE_CREATE: &str = "feature.create";
    pub const FEATURE_ADD_REPO: &str = "feature.addRepo";
    pub const FEATURE_ARCHIVE: &str = "feature.archive";
    pub const WORKTREE_LIST: &str = "worktree.list";
    pub const GIT_STATUS: &str = "git.status";
    pub const GIT_LOG: &str = "git.log";
    pub const GIT_REBASE: &str = "git.rebase";
    pub const GIT_REBASE_ABORT: &str = "git.rebaseAbort";
    pub const FS_WRITE: &str = "fs.writeFile";
    pub const FS_REMOVE: &str = "fs.removeFile";
    /// Expande `~` e devolve o caminho absoluto no WSL.
    pub const FS_EXPAND: &str = "fs.expand";
    pub const TERM_OPEN: &str = "term.open";
    pub const TERM_ATTACH: &str = "term.attach";
    pub const TERM_WRITE: &str = "term.write";
    pub const TERM_RESIZE: &str = "term.resize";
    pub const TERM_CLOSE: &str = "term.close";
    pub const TERM_LIST: &str = "term.list";
    pub const GH_PRS: &str = "gh.prs";

    // Notificações (agente → app)
    pub const EV_TERM_OUTPUT: &str = "term.output";
    pub const EV_TERM_EXIT: &str = "term.exit";
}

// ---------------------------------------------------------------------------
// Parâmetros e resultados
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub agent_version: String,
    pub protocol: u32,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanParams {
    /// Pasta com os clones, ex.: `~/code/pagamentos`.
    pub dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScannedRepo {
    pub name: String,
    pub path: String,
    pub remote: Option<String>,
    /// Ex.: `origin/main`, lido de `refs/remotes/origin/HEAD`.
    pub default_base: Option<String>,
}

/// Um repositório que participa de uma feature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoSpec {
    /// Nome curto, vira a subpasta dentro da pasta da feature.
    pub name: String,
    /// Caminho do clone principal.
    pub clone_path: String,
    /// Ponto de partida: `origin/develop`, uma tag ou um commit.
    pub base: String,
    /// Branch deste repositório. Sem ela, usa a `branch` geral do pedido.
    /// Cada repo pode ter a sua (ex.: escopos diferentes de conventional commits).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeatureParams {
    /// Pasta da feature, ex.: `~/wt/pagamentos/pay-412`.
    pub root: String,
    /// Branch padrão; cada repo pode sobrescrever com `RepoSpec::branch`.
    #[serde(default)]
    pub branch: String,
    pub repos: Vec<RepoSpec>,
    /// Rodar `git fetch origin` antes de criar (padrão: sim).
    #[serde(default = "default_true")]
    pub fetch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRepoParams {
    pub root: String,
    #[serde(default)]
    pub branch: String,
    pub repo: RepoSpec,
    #[serde(default = "default_true")]
    pub fetch: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWorktree {
    pub repo: String,
    pub path: String,
    pub branch: String,
    pub base: String,
    /// `true` quando a branch já existia e foi só reaproveitada.
    pub reused_branch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveParams {
    pub root: String,
    pub repos: Vec<ArchiveRepo>,
    /// Apagar a branch local depois de remover a worktree.
    #[serde(default)]
    pub delete_branch: bool,
    /// Remover mesmo com alterações não commitadas.
    #[serde(default)]
    pub force: bool,
    /// Arquivo `.code-workspace` a apagar junto (opcional).
    #[serde(default)]
    pub workspace_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRepo {
    pub clone_path: String,
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveResult {
    pub removed: Vec<String>,
    pub deleted_branches: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListParams {
    pub clone_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusParams {
    pub path: String,
    /// Base para calcular à frente/atrás, ex.: `origin/develop`.
    #[serde(default)]
    pub base: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub path: String,
    pub branch: Option<String>,
    /// Commits na feature que não estão na base.
    pub ahead: u32,
    /// Commits na base que não estão na feature.
    pub behind: u32,
    pub changes: Vec<FileChange>,
    /// Há um rebase em andamento (normalmente por conflito).
    pub rebasing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// `M` modificado, `A` adicionado, `D` removido, `R` renomeado, `U` em conflito, `?` não rastreado.
    pub code: String,
    pub path: String,
    pub staged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogParams {
    pub path: String,
    pub base: String,
    #[serde(default = "default_log_limit")]
    pub limit: u32,
}

fn default_log_limit() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub when: String,
    /// `true` para commits da feature; `false` para os da base.
    pub in_feature: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseParams {
    pub path: String,
    pub base: String,
    #[serde(default = "default_true")]
    pub fetch: bool,
    /// Guardar e devolver alterações não commitadas automaticamente.
    #[serde(default)]
    pub autostash: bool,
    /// `rebase` (padrão) ou `merge`.
    #[serde(default)]
    pub strategy: UpdateStrategy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateStrategy {
    #[default]
    Rebase,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RebaseResult {
    pub ok: bool,
    pub conflicts: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathParams {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermOpenParams {
    /// Id estável escolhido pelo app, ex.: `pay-412/api/0`.
    pub id: String,
    pub cwd: String,
    #[serde(default)]
    pub shell: Option<String>,
    /// Comando digitado no shell logo após abrir (o shell continua aberto depois).
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    120
}
fn default_rows() -> u16 {
    32
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermOpenResult {
    /// `false` quando o terminal já existia e foi só reaproveitado.
    pub created: bool,
    /// Saída já guardada (base64), para redesenhar a tela.
    pub scrollback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermIdParams {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermWriteParams {
    pub id: String,
    /// Bytes em base64.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermResizeParams {
    pub id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermInfo {
    pub id: String,
    pub cwd: String,
    pub alive: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermOutputEvent {
    pub id: String,
    /// Bytes em base64.
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermExitEvent {
    pub id: String,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GhPrsParams {
    /// Qualquer clone do repositório (o `gh` descobre o remoto).
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"git.status","params":{"path":"/tmp/x","base":"origin/main"}}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, methods::GIT_STATUS);
        let p: StatusParams = serde_json::from_value(req.params).unwrap();
        assert_eq!(p.base.as_deref(), Some("origin/main"));
    }

    #[test]
    fn defaults_apply() {
        let p: CreateFeatureParams = serde_json::from_str(
            r#"{"root":"/r","branch":"b","repos":[]}"#,
        )
        .unwrap();
        assert!(p.fetch);
        let r: RebaseParams = serde_json::from_str(r#"{"path":"/p","base":"origin/main"}"#).unwrap();
        assert_eq!(r.strategy, UpdateStrategy::Rebase);
        assert!(!r.autostash);
    }
}
