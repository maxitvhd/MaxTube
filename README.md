# MaxTube

Gerencie os títulos dos vídeos do seu canal do YouTube usando **IA local (Ollama)**.

O fluxo é: **puxar** os vídeos → **gerar** sugestões de título com o modelo local →
**revisar** uma por uma (aprovar/editar/rejeitar) → **aplicar** no YouTube.

Nada de títulos é enviado para serviços de IA na nuvem: tudo roda no seu Ollama.

## Requisitos

- Rust (stable) — https://rustup.rs
- Ollama — https://ollama.com
- Conta do YouTube (canal) e acesso ao Google Cloud Console

## 1. Baixe um modelo no Ollama

```bash
ollama pull qwen2.5:7b
# ou um mais leve para máquinas com pouca RAM:
# ollama pull qwen2.5:3b
```

Garanta que o Ollama está rodando (abra o app ou `ollama serve`).

## 2. Configure o acesso ao YouTube (Google Cloud)

1. Acesse https://console.cloud.google.com e crie um projeto.
2. **APIs e serviços → Biblioteca** → pesquise **YouTube Data API v3** → **Ativar**.
3. **APIs e serviços → Tela de permissão OAuth** → configure como **Externo** e
   adicione a sua conta em **Usuários de teste**.
4. **APIs e serviços → Credenciais → Criar credenciais → ID do cliente OAuth**:
   - Tipo: **App para computador (Desktop app)**
   - Baixe o JSON (ou copie o `client_id` e o `client_secret`).
5. Informe as credenciais e faça login. Duas opções:

**Painel web (recomendado):** abre uma página local onde você cola o JSON (ou os
campos) e clica em "Fazer login com Google".

```bash
cargo run -- panel
```

**Linha de comando:** salve o JSON em `.secrets/client_secret.json` e rode:

```bash
cargo run -- auth
```

O navegador abrirá para você autorizar. O token fica salvo em `.secrets/token.json`
(renovado automaticamente).

## 3. Uso

```bash
# Fluxo completo (puxa, gera, revisa e aplica)
cargo run -- run --limit 20

# Ou passo a passo:
cargo run -- pull --limit 20     # baixa os vídeos para data/videos.json
cargo run -- generate            # cria sugestões com o Ollama (data/proposals.json)
cargo run -- review              # revisão interativa, uma por uma
cargo run -- apply               # envia os títulos aprovados ao YouTube
cargo run -- status              # mostra o estado atual
cargo run -- panel               # painel web para credenciais e login
```

Durante a revisão:

```
[a]provar  [e]ditar  [r]ejeitar  [s]pular  [A]provar todos restantes  [q]sair
```

## Editar o prompt principal

O prompt enviado ao modelo fica em **[`prompt.md`](prompt.md)** — edite o arquivo à
vontade. Placeholders disponíveis: `{{title}}` e `{{description}}`.

## Configuração

`config.toml` (criado automaticamente na primeira execução):

```toml
model = "qwen3.5:4b"
ollama_url = "http://192.168.1.23:11434"
prompt_file = "prompt.md"
client_secret = ".secrets/client_secret.json"
token = ".secrets/token.json"
data_dir = "data"
```

Trocar de modelo é só alterar `model` aqui ou usar `--model nome`.

### Usando um Ollama remoto

O `ollama_url` pode apontar para outra máquina da sua rede (ex.:
`http://192.168.1.23:11434`). O servidor precisa aceitar conexões externas
(`OLLAMA_HOST=0.0.0.0 ollama serve`). Modelos com "thinking" (ex.: `qwen3.5`)
são suportados — o `think` é desativado automaticamente.

## Cota da API

A YouTube Data API tem cota diária de **10.000 unidades**. Cada atualização de
título custa ~50 unidades, então dá para alterar cerca de **200 vídeos por dia**
sem problemas.

## Segurança

`.secrets/` e `data/` são ignorados pelo git. **Nunca** comite o
`client_secret.json` nem o `token.json`.

## Licença

MIT
