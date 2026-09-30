# dejavu-echo (aplicativo para desktop)

> Twitch Links → Discord, com uma janela em vez do terminal

Este é o **port para aplicativo de desktop** do `dejavu-echo`. O programa é o
mesmo: lê o chat de uma live da Twitch e envia para um canal do Discord os
**links** que passarem por um filtro. A diferença é que ele tem uma interface
gráfica — nada de terminal, `docker compose` ou instalação do Node.js por quem
vai usar.

O programa foi reescrito em **Rust** (núcleo) com **Tauri 2** e uma tela em
**React + TypeScript**.

- **Não precisa de conta, token ou app na Twitch**: o chat é lido de forma anônima.
- **Não precisa de bot no Discord**: usa um webhook do canal.
- A URL do webhook fica guardada num **arquivo oculto** fora da pasta do projeto,
  então não há como enviá-la ao Git por acidente.

---

## Como funciona

1. Você abre o aplicativo, cola a URL do webhook do Discord e informa o canal da Twitch.
2. Cada mensagem do chat que contém link passa pelo filtro (ver [Regras do filtro](#regras-do-filtro)).
3. Se aprovada, a mensagem é enviada ao Discord neste formato:

> 🔗 **NickDoUsuario**: olha isso https://exemplo.com/pagina
> ↪️ *em resposta a **Fulano**: "texto da mensagem original"*

A segunda linha só aparece quando a mensagem é uma **resposta** a outra pessoa no
chat. O link fica em texto normal, então o Discord gera a pré-visualização.

---

## Telas

O aplicativo tem três abas.

### Conexão

- **Webhook do Discord**: cole a URL, clique em *Salvar*. O programa valida o
  formato, confere com o Discord (sem postar nada) e guarda no arquivo oculto.
  Os botões *Verificar salvo*, *Apagar*, *Abrir Discord* e *Mostrar pasta do
  segredo* ficam aqui.
- **Canal da Twitch**: digite o nome (`nomedocanal`) ou cole o link
  (`https://www.twitch.tv/nomedocanal`) e clique em *Iniciar*. O indicador muda
  para **Lendo o chat** quando a conexão está ativa. *Encerrar* para parar.

### Filtro

Todas as regras da seção [Regras do filtro](#regras-do-filtro) podem ser
editadas na tela. As alterações só vão para o disco ao clicar em
*Salvar configurações*.

### Registro

Mostra os contadores (enviadas, ignoradas, apagadas, banidos) e o histórico do
que aconteceu, com filtro por nível e botão para exportar como `.log`. Os
registros ficam **só na memória** e somem ao fechar o programa.

---

## Requisitos

- **Windows 10/11** (o app é empacotado como instalador; o WebView2 já vem no
  Windows 10 mais recente e no 11).
- Um servidor do Discord em que você possa **gerenciar webhooks** no canal de destino.
- Conexão com a internet.

Para **compilar** o projeto você também precisa de:

- [Rust](https://rustup.rs/) (toolchain `stable`, arquitetura MSVC)
- [Node.js](https://nodejs.org/) 20+ e [pnpm](https://pnpm.io/)
- *Desktop development with C++* do Visual Studio Build Tools (para o linker MSVC)

---

## Como usar

### 1. Crie o webhook no Discord

1. No Discord, abra o canal onde os links devem aparecer.
2. Clique na engrenagem do canal (**Editar canal**) → **Integrações** → **Webhooks**.
3. Clique em **Novo Webhook**, dê um nome (por exemplo, "Links da live") e, se quiser, uma imagem.
4. Clique em **Copiar URL do Webhook** e mantenha a URL copiada.

> É necessária a permissão **Gerenciar Webhooks** no canal. Se o botão não
> aparecer, peça a um administrador.

> ⚠️ A URL do webhook funciona como uma senha: quem a tiver pode postar no canal.
> Não a mostre em transmissões, prints ou mensagens.

### 2. Configure o aplicativo

1. Exclua o webhook e crie outro. Depois, na aba **Conexão**, cole a nova URL e
   clique em *Salvar*.
2. Informe o canal da Twitch e clique em *Iniciar*.
3. Quando aparecer **Lendo o chat de #nomedocanal**, está funcionando. Deixe a
   janela aberta durante a live.

---

## Desenvolvimento

```bash
pnpm install          # dependências da tela (React/TS)
pnpm tauri dev        # roda o aplicativo em modo de desenvolvimento
pnpm tauri build      # gera o instalador em src-tauri/target/release/bundle
```

Testes, formatação e lint do núcleo em Rust (a partir de `src-tauri/`):

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

---

## Regras do filtro

| Opção | Padrão | Descrição |
|---|---|---|
| `allowedRoles` | nenhum (só o dono do canal) | Restringe quem pode ter links repassados. Marque dono do canal, moderador, VIP e/ou assinante |
| `blockedUsers` | Nightbot, StreamElements, Streamlabs, Moobot, Fossabot, WizeBot | Usuários ignorados (bots de chat) |
| `blockedDomains` | `bit.ly`, `tinyurl.com`, `t.co`, `grabify.link`, `iplogger.org`, `discord.gg` | Domínios sempre bloqueados (inclui subdomínios) |
| `allowedDomains` | nenhum (todos, exceto os bloqueados) | Se preenchido, **só** estes domínios passam. Ex.: `youtube.com`, `youtu.be` |
| `dedupeMinutes` | `30` | Ignora o mesmo link repetido dentro desse prazo |
| `userCooldownSeconds` | `20` | Intervalo mínimo entre mensagens aprovadas do mesmo usuário (`0` desliga) |
| `delaySeconds` | `8` | Espera antes de postar. Se um mod apagar a mensagem (ou banir o autor) nesse tempo, nada é enviado |

### Comportamentos importantes

- **Link reprovado junto com um aprovado:** se a mesma mensagem tiver um link
  bloqueado e outro aprovado, o bloqueado é trocado por *[link removido]*.
- **Sem menções:** ninguém é marcado no Discord. O envio sempre leva
  `allowed_mentions` vazio, então `@everyone`, cargos e usuários não disparam.
- **Markdown seguro:** nicks e textos têm o markdown escapado, o que impede links
  "mascarados" como `[youtube.com](https://golpe.example)`.
- **Limite do Discord:** as mensagens entram numa fila. Se o Discord pedir para
  esperar (erro 429), o programa aguarda e tenta de novo.
- **Respostas:** o `@fulano` que a Twitch adiciona no começo das respostas é
  removido, e o trecho da mensagem original (até 120 caracteres) aparece em
  itálico, com os links dela trocados por `[link]`.

---

## Segurança e privacidade

- A URL do webhook é salva **apenas** num arquivo oculto chamado `.secret_env`, na
  pasta de configuração do aplicativo (`%APPDATA%\com.dejavuecho.desktop` no Windows),
  **fora da pasta do projeto**, com permissão restrita ao usuário do programa.
- **Se o webhook vazar:** no Discord, vá em *Integrações → Webhooks*, exclua o
  webhook e crie outro. Depois, no aplicativo, clique em *Apagar* e cole a nova URL.
- Nada é gravado além do webhook e das regras do filtro. Links já vistos e
  cooldowns ficam só na memória e somem ao fechar o programa.
- O que é enviado ao Discord: o nick de quem postou, o texto da mensagem (com os
  links aprovados) e, quando for resposta, o nick e o trecho da mensagem original.

---

## Solução de problemas

| Problema | Provável causa e solução |
|---|---|
| `Isso não parece uma URL de webhook do Discord` | A URL foi copiada incompleta. Ela deve começar com `https://discord.com/api/webhooks/` |
| `O Discord não reconheceu esse webhook` | O webhook foi excluído ou a URL está incorreta. Copie de novo |
| `O webhook salvo não existe mais` | Ele foi excluído durante o uso. Clique em *Apagar*, cole uma URL nova e inicie de novo |
| `Nome de canal inválido` | Confira o nome. Aceita `nomedocanal`, `#nomedocanal` ou o link completo |
| `Não foi possível conectar ao chat da Twitch` | Verifique a internet e o nome do canal. Tente de novo em alguns instantes |
| Nada aparece no Discord | Confira: o link começa com `http://`, `https://` ou `www.`? O domínio está bloqueado? O usuário está em cooldown ou é um bot ignorado? Lembre do atraso de 8 segundos |
| O aplicativo abre uma janela em branco | Atualize o WebView2 (Windows) em *Configurações → Aplicativos → Microsoft Edge WebView2 Runtime* |

---

## Limitações

- **Só uma pessoa deve rodar o programa por canal ao mesmo tempo.** A
  deduplicação é local, então duas cópias rodando no mesmo canal enviariam links
  repetidos.
- **Um canal por vez.** Para acompanhar dois canais, abra dois aplicativos.
- **Leitura anônima do chat:** é um método amplamente usado, mas a Twitch não o
  garante. Se ele deixar de funcionar, será preciso passar a usar uma conta com token.
- **Detecção de links:** só reconhece endereços iniciados por `http://`,
  `https://` ou `www.`. Textos como `site.com/pagina` sem prefixo não são detectados.
- **Sem dados da live:** título e tempo de live não são incluídos, pois exigiriam
  credenciais da API da Twitch.
- As regras do Discord e da Twitch mudam com o tempo. Ao usar o programa em
  comunidades maiores, vale conferir os termos de cada plataforma.

---

## Estrutura do projeto

```
├── package.json                 # dependências da tela (React/TS) e scripts
├── index.html                   # página raiz da tela
├── src/                         # tela (React + TypeScript)
│   ├── App.tsx                  # abas Conexão, Filtro e Registro
│   ├── App.css                  # estilos
│   ├── types.ts                 # tipos espelhando o núcleo em Rust
│   ├── useEngine.ts             # liga a tela aos comandos e eventos do Tauri
│   └── components/              # ConnectionTab, FilterTab, LogTab, TagInput
└── src-tauri/                   # núcleo em Rust
    ├── src/
    │   ├── lib.rs               # montagem do Tauri, plugins e comandos
    │   ├── config.rs            # configuração e normalização de domínios
    │   ├── filter.rs            # regras do filtro e formatação das mensagens
    │   ├── engine.rs            # cooldown, antiduplicidade, atraso e cancelamento
    │   ├── twitch.rs            # leitura anônima do chat (twitch-irc)
    │   ├── discord.rs           # validação do webhook e fila de envio
    │   ├── secrets.rs           # leitura e escrita do arquivo oculto .secret_env
    │   ├── model.rs             # tipos compartilhados
    │   └── state.rs             # estado global, comandos e laço do motor
    ├── tauri.conf.json          # janela, identificador e empacotamento
    └── Cargo.toml               # dependências do núcleo
```

Este projeto foi baseado na versão original (Node.js e Docker), mantida em
`../dejavu-echo` como referência.
