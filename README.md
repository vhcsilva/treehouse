# Treehouse

Gerencie várias features ao mesmo tempo com **git worktrees**, sem `stash` nem `checkout`.
Cada feature tem a sua pasta, a sua janela do Kiro (com a cor dela na barra de título) e os
seus terminais, que continuam rodando quando você troca de contexto.

- **App de desktop no Windows** (Tauri 2 + React), com o visual de vidro (Mica/Acrylic).
- **Agente no WSL** (`treehouse-agent`, Rust): git, worktrees e terminais rodam no Linux, perto do código.
- Arquitetura completa: [doc de arquitetura](https://claude.ai/artifact/11FuRE5YMz4Yopoaz7x5ji).

> Estado: **fase 1 (MVP)**. Criar, trocar, atualizar da base e arquivar features; terminais
> persistentes; abrir no Kiro; configurações de workspace, Kiro, terminal e aparência.
> GitHub (PRs) e Jira entram nas fases 2 e 3.

## Como funciona

```
Windows                                   WSL
┌──────────────────────────┐   JSON-RPC   ┌──────────────────────────────┐
│ Treehouse (Tauri)        │◄────────────►│ treehouse-agent (daemon)     │
│  interface + SQLite      │  stdio via   │  git · worktrees · PTYs      │
└──────────┬───────────────┘   wsl.exe    └──────────────┬───────────────┘
           │ kiro --file-uri vscode-remote://wsl+…       │ git worktree add
           ▼                                             ▼
        Kiro (uma janela por feature)  ───────►  ~/wt/<workspace>/<feature>/<repo>
                                                 ~/wt/<workspace>/<feature>.code-workspace
```

- O app roda `wsl.exe -d <distro> --exec … treehouse-agent connect`. O `connect` liga o
  daemon (se preciso) e repassa o stdio para o socket dele. Fechar o app **não** mata os terminais.
- Na primeira execução, o app copia o agente para `~/.local/share/treehouse/bin/` na distro
  escolhida e o atualiza sozinho quando a versão muda.
- O `.code-workspace` de cada feature fica **fora** dos repositórios, então não aparece no `git status`.

## Pré-requisitos

**No Windows**

- Windows 11 (Mica precisa dele; no 10 o app usa o modo sólido).
- [Rust](https://rustup.rs) com o toolchain MSVC e as *Build Tools* do Visual Studio (C++).
- Node.js 22 e pnpm 10 (`npm i -g pnpm`).
- Kiro com o comando `kiro` no PATH. Teste no PowerShell: `kiro --version`.
  Se não estiver no PATH, informe o caminho completo em *Configurações → Kiro e terminal*.
- Extensão **Open Remote - WSL** no Kiro (a mesma integração com o WSL que você já usa).

**No WSL**

- `git` (e, para a fase 2, o `gh` autenticado).
- Rust, só para compilar o agente: `curl https://sh.rustup.rs -sSf | sh`.
  Opcional, para um binário estático: `rustup target add x86_64-unknown-linux-musl && sudo apt install musl-tools`.

## Compilar e instalar

1. Clone o repositório **no Windows** (ex.: `C:\dev\treehouse`). O build do Tauri por
   `\\wsl.localhost` é lento e o pnpm não se dá bem com caminhos de rede.
2. Compile o agente **dentro do WSL**, a partir do mesmo checkout:
   ```bash
   cd /mnt/c/dev/treehouse
   ./scripts/build-agent.sh
   ```
   Isso coloca o binário em `app/src-tauri/resources/agent/treehouse-agent` (o cache de
   compilação fica em `~/.cache/treehouse-target`, no Linux, que é bem mais rápido).
3. No PowerShell, gere o instalador:
   ```powershell
   cd C:\dev\treehouse\app
   pnpm install
   pnpm tauri build
   ```
   O instalador sai em `app\src-tauri\target\release\bundle\nsis\Treehouse_0.1.0_x64-setup.exe`.

Para desenvolver com recarga automática: `pnpm tauri dev` (também no PowerShell).

> O workflow em `.github/workflows/ci.yml` faz os três passos no GitHub Actions e publica o
> instalador como artefato, se você preferir não compilar localmente.

## Primeiro uso

1. Escolha a distro, aponte a pasta dos projetos (ex.: `~/code/pagamentos`) e clique em **Escanear**.
2. Ajuste a **branch base** de cada repositório (ex.: `origin/develop` no api, `origin/main` no web)
   e, se quiser, o **comando ao abrir** (ex.: `npm run dev`). Clique em **Começar**.
3. **Nova feature** (`Ctrl+Shift+N`): nome, card do Jira (opcional), repositórios. A tela mostra
   os comandos `git worktree` que vão rodar antes de você confirmar.

## Atalhos

| Ação | Atalho |
| --- | --- |
| Trocar de feature | `Ctrl+1…9` |
| Paleta de comandos | `Ctrl+Shift+K` |
| Nova feature | `Ctrl+Shift+N` |
| Abrir no Kiro | `Ctrl+Shift+O` |
| Atualizar da base (rebase) | `Ctrl+Shift+U` |
| Configurações | `Ctrl+,` |

Os atalhos usam `Shift` de propósito: `Ctrl+K` e `Ctrl+U` apagam a linha no shell, e o
terminal embutido precisa deles.

## Desenvolvimento

```bash
# Agente e protocolo (Linux/WSL)
cargo test                    # testes com repositórios git reais, terminais e daemon
cargo clippy --all-targets -- -D warnings

# Interface no navegador, com um backend simulado (sem Tauri nem WSL)
cd app && pnpm install && pnpm dev     # http://localhost:5173  (?vazio = primeira execução)

# Testes do frontend, incluindo a integração com o agente real
cargo build -p treehouse-agent
TREEHOUSE_AGENT_BIN=$PWD/../target/debug/treehouse-agent pnpm test
```

Estrutura:

```
crates/protocol   tipos do JSON-RPC (espelhados em app/src/lib/protocol.ts)
crates/agent      treehouse-agent: git.rs, term.rs, server.rs, daemon.rs
app/src-tauri     backend do app: ponte com o WSL, SQLite, Kiro, efeitos de janela
app/src           interface React (store.ts, terminals.ts, components/)
scripts           build-agent.sh
```

## Limitações conhecidas

- **WSL ocioso:** com o app fechado, o daemon (e os terminais) continuam vivos enquanto o WSL
  estiver de pé. Se a sua configuração desliga a distro depois de um tempo sem sessões abertas,
  os terminais morrem junto. Com o app aberto isso não acontece.
- **Programas de tela cheia** (vim, htop) podem voltar desenhados pela metade ao reabrir o app, até
  a próxima atualização de tela. Shells e servidores de desenvolvimento voltam certinhos.
- O status do git é atualizado a cada 5 s (e ao focar a janela); o aviso instantâneo via inotify
  está no roadmap.
- Estado “Kiro aberto/fechado” por feature ainda não é detectado.
