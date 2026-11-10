# Tempo — gestão de tempo por disponibilidade

App Android, web e Windows, escrito em Rust, que divide o **tempo disponível** do dia
(24h − tempo reservado) em **porcentagens por tag** e transforma cada porcentagem em um **timer**.

> Reservando 8h de sono (um bloco criado por você), sobram 16h. Estudo = 10% → 1h36min de timer por dia.

A especificação completa (requisitos funcionais, não funcionais e regras de negócio) está em
[`documentação.md`](documentação.md). Os comentários no código citam os IDs de lá (`RN15`, `RF21`…).

## Estrutura

```
crates/
  tempo-core/    Regras puras: sono, porcentagens, dia lógico, timer, estatísticas (sem Android)
  tempo-store/   SQLite + serviço de aplicação (Tempo): transações, virada do dia, histórico
  tempo-app/     Interface Slint (ui/*.slint), ligação com o serviço e integração Android (JNI)
  tempo-web/     Versão web: interface Leptos (HTML) compilada para WebAssembly
  tempo-pc/      Programa para Windows (Tauri) que roda a interface web numa janela nativa
android/         Projeto Gradle: manifesto, recursos e o shim Java mínimo (com.projetotempo)
scripts/         build-android.ps1 (APK), build-web.ps1/dev-web.ps1 (web) e check.ps1 (fmt + clippy + testes)
```

Fluxo em execução:

```
NativeActivity ──► android_main (Rust) ──► Slint UI ──► Tempo (SQLite)
                                              │
                                              └─ JNI ─► Bridge.java ─► notificação, alarme, permissões
Receptores Java (botões da notificação, alarme, boot) ─ JNI ─► Native.onCommand (Rust) ─► Tempo
```

O timer é baseado em marcação de tempo (RN18): nada fica "contando" em segundo plano. A notificação
usa o cronômetro do próprio sistema e o fim é avisado por `AlarmManager`. Por isso não há serviço
em primeiro plano nem wake lock (RNF13).

## Ambiente

| Ferramenta | Versão usada | Observação |
|---|---|---|
| Rust | 1.99 (`x86_64-pc-windows-gnu`) | alvos `aarch64-linux-android`, `x86_64-linux-android` |
| MinGW (winlibs) | 16.2 | linker/`dlltool` do toolchain GNU e compilação do SQLite no desktop |
| JDK | Temurin 17 | `javac`/`d8` para o shim Java e o helper do Slint |
| Android SDK | platform 36, build-tools 36.0.0 | |
| Android NDK | 28.2.13676358 | |
| cargo-ndk | 4.1 | `cargo install cargo-ndk` |
| Gradle | 9.9 (wrapper) + AGP 9.5.1 | |

Nesta máquina tudo foi instalado em nível de usuário: Rust em `~/.cargo`, JDK/MinGW via Scoop e o
SDK em `E:\toolchains\android-sdk`. O script usa esses caminhos por padrão; para outros, defina
`ANDROID_HOME`, `ANDROID_NDK_ROOT` e `JAVA_HOME`.

## Compilar e instalar

```powershell
./scripts/build-android.ps1            # APKs release em android/app/build/outputs/apk/release
./scripts/build-android.ps1 -Install   # instala o APK universal no aparelho conectado (adb)
./scripts/build-android.ps1 -Debug     # build debug
```

Saída: `app-arm64-v8a-release.apk` (celulares), `app-x86_64-release.apk` (emulador) e
`app-universal-release.apk`.

O release é assinado com a **chave de debug** para facilitar a instalação local. Para publicar na
Play Store, crie uma keystore e configure um `signingConfig` próprio em `android/app/build.gradle.kts`.

## Desenvolver no desktop

A mesma interface roda no Windows, útil para iterar sem emulador:

```powershell
cargo run -p tempo-app --bin tempo-desktop          # usa ./tempo-desktop.db
$env:TEMPO_SNAPSHOTS="shots"; cargo run -p tempo-app --bin tempo-desktop   # salva PNG de cada tela
```

No desktop, notificações e alarmes são apenas registrados em log, e "Exportar CSV" grava um arquivo.

### Atualização automática enquanto edita

```powershell
./scripts/dev.ps1
```

- Salvou um `.slint`: a tela muda na hora, sem fechar o app (Slint live-preview).
- Salvou um `.rs`: o app é recompilado e reaberto sozinho (cargo-watch).

Usa `target/dev` como pasta de compilação, separada do `target/debug`.

## Testes e qualidade

```powershell
./scripts/check.ps1     # cargo fmt --check, clippy -D warnings, cargo test
```

- `tempo-core`: testes unitários dos cálculos (RN01–RN08), dia lógico, timer e estatísticas.
- `tempo-store/tests/service.rs`: critérios de aceite da documentação com relógio manual
  (virada do dia, timer que sobrevive ao fechamento do app, relógio voltando, lançamento manual…).

## Versão web

`crates/tempo-web` usa as mesmas regras (`tempo-core`) e o mesmo serviço/SQLite (`tempo-store`) do
Android, com uma interface HTML própria em [Leptos](https://leptos.dev), compilada para WebAssembly.
Não há servidor: o banco roda em memória no navegador e, a cada alteração, uma cópia completa
(`Tempo::snapshot`, feature `snapshot` do `tempo-store`) é gravada no IndexedDB. Os dados da web são
separados dos do celular.

```powershell
./scripts/dev-web.ps1     # http://127.0.0.1:8080, recarrega ao salvar
./scripts/build-web.ps1   # site estático em crates/tempo-web/dist (GitHub Pages, Netlify, nginx…)
```

Requisitos extras: alvo `wasm32-unknown-unknown`, [trunk](https://trunkrs.dev) e `wasm-opt`
(`scoop install binaryen`; a versão em `Trunk.toml` deve bater com a instalada). O SQLite em
WebAssembly precisa de um clang com alvo wasm32: o script usa o do Android NDK se não houver LLVM
no PATH.

Diferenças em relação ao Android:

- Aviso de fim via Notification API + som (Web Audio). Com a aba em segundo plano o navegador pode
  atrasar o aviso em até ~1 min; com o navegador fechado não há aviso (o timer continua correto,
  pois é baseado em marcação de tempo).
- Layout com navegação lateral e duas colunas em telas largas; atalhos: espaço pausa/retoma, Esc
  fecha diálogos. O título da aba mostra o tempo restante.
- Várias abas abertas se sincronizam via `BroadcastChannel` (a última alteração salva vence).

## Programa para Windows

`crates/tempo-pc` embrulha a mesma interface web (`tempo-web`) numa janela nativa com
[Tauri](https://tauri.app). A regra de negócio e a interface são exatamente as mesmas da versão
web; o processo nativo cuida do que o navegador não faz bem: guarda o banco num arquivo SQLite no
PC, avisa o fim do timer com notificação do Windows mesmo com a janela fechada, mantém um ícone
na bandeja e pode iniciar junto com o Windows. É o equivalente, no PC, do `Bridge.java` do Android.

```powershell
cargo tauri dev -p tempo-pc            # roda em desenvolvimento (abre o trunk serve sozinho)
cargo tauri build -p tempo-pc          # instalador .exe (NSIS) em target/release/bundle
```

Requisitos extras: `cargo install tauri-cli --version ^2` e os mesmos pré-requisitos da versão
web (o build chama `scripts/build-web.ps1` automaticamente). O banco fica em
`%APPDATA%\com.projetotempo.tempo\tempo.db`; os dados são separados dos da web e do Android.

Diferenças em relação à web pura:

- Fechar a janela só esconde o programa — o timer e o aviso de fim continuam rodando pela bandeja.
  Para sair, clique com o botão direito no ícone perto do relógio.
- O aviso de fim do timer sai mesmo com a janela escondida, porque é agendado pelo processo nativo.
- A tela "Configurações" tem um interruptor para iniciar o Tempo junto com o Windows.
