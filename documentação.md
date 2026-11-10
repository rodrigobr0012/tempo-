# Documentação — App de Gestão de Tempo por Disponibilidade

> Versão 0.3 · 10/11/2026 · Status: implementado (ver seção 11) · 0.3: sono fixo substituído por blocos reservados criados pelo usuário

---

## 1. Visão geral

O app parte de uma ideia simples: **o dia não tem 24 horas úteis**. Descontado o tempo que a pessoa reserva (sono, faculdade, deslocamento…), sobra um **tempo disponível**, e é sobre ele que ela planeja.

O usuário cria **blocos reservados**, com nome e duração definidos por ele (ex.: "Sono 8h"). O app calcula o tempo disponível (`24h − blocos`) e o usuário o divide em **porcentagens**, cada uma ligada a uma **tag** (Estudo, Trabalho, Exercício, Lazer etc.). Cada porcentagem vira uma duração concreta e um **timer** que o usuário inicia, pausa e acompanha ao longo do dia.

### 1.1 Exemplo de referência

| Item | Valor |
|---|---|
| Dia | 24h |
| Bloco reservado "Sono" (criado pelo usuário) | 8h |
| **Tempo disponível** | **16h (960 min)** |
| Tag "Estudo" = 10% | 96 min = **1h36min** |
| Tag "Trabalho" = 50% | 480 min = **8h00min** |
| Tag "Exercício" = 5% | 48 min = **0h48min** |
| Não alocado (35%) | 336 min = **5h36min** |

### 1.2 Plataforma e tecnologia

- **Plataforma:** somente Android.
- **Linguagem:** Rust (regra de negócio, persistência e interface).
- **Fora de escopo:** iOS, desktop, web, sincronização em nuvem, contas de usuário.

---

## 2. Glossário

| Termo | Definição |
|---|---|
| **Bloco reservado** | Tempo com nome e duração definidos pelo usuário que sai das 24h antes da divisão (ex.: "Sono 8h", "Faculdade 4h"). |
| **Tempo disponível (TD)** | `24h − soma dos blocos reservados`. É a base de 100% para as alocações. |
| **Início do dia** | Horário configurável (padrão 04:00) em que o dia lógico vira. |
| **Dia lógico** | Ciclo do app. Começa no horário de início do dia e termina no mesmo horário do dia seguinte (não necessariamente à meia-noite). |
| **Tag** | Categoria de uso do tempo criada pelo usuário (ex.: Estudo). |
| **Alocação** | Porcentagem do TD atribuída a uma tag. |
| **Tempo alocado** | Duração resultante da alocação: `TD × porcentagem`. |
| **Tempo consumido** | Soma do tempo em que o timer da tag esteve rodando no dia lógico. |
| **Tempo restante** | `tempo alocado − tempo consumido` (mínimo 0). |
| **Excedente** | Tempo consumido além do alocado. |
| **Não alocado** | Parte do TD que não pertence a nenhuma tag (`100% − soma das alocações`). |
| **Sessão** | Um trecho contínuo de timer rodando (do início/retomada até a pausa/parada). |

---

## 3. Requisitos funcionais (RF)

Prioridade no padrão MoSCoW: **M** = obrigatório no MVP · **S** = importante · **C** = desejável · **W** = fora desta versão.

### 3.1 Tempo reservado e tempo disponível

| ID | Requisito | Prioridade |
|---|---|---|
| RF01 | O usuário deve **criar, editar e excluir blocos reservados**, cada um com **nome** e **duração** escolhidos por ele (ex.: "Sono 8h", "Faculdade 4h"). Pode não haver nenhum bloco. | M |
| RF02 | O usuário deve definir o horário de **início do dia** (HH:MM, padrão 04:00), em que os timers zeram e um novo dia começa. | M |
| RF03 | O sistema deve calcular e exibir o tempo disponível (TD) = 24h − soma dos blocos reservados. | M |
| RF04 | Na primeira execução, o app deve apresentar um assistente (onboarding) com os blocos reservados (sugerindo "Sono 8h", editável e removível) e o início do dia, antes de qualquer outra tela. | M |
| RF05 | O usuário pode configurar blocos reservados diferentes por dia da semana (ex.: fim de semana). | C |

### 3.2 Tags e alocação

| ID | Requisito | Prioridade |
|---|---|---|
| RF06 | O usuário deve criar, editar e excluir tags com nome, cor e porcentagem. | M |
| RF07 | Ao digitar a porcentagem, o sistema deve mostrar em tempo real a duração equivalente (ex.: "10% = 1h36min"). | M |
| RF08 | O usuário pode, alternativamente, informar a duração (ex.: 2h) e o sistema converte para a porcentagem correspondente. | S |
| RF09 | O sistema deve exibir a soma das alocações e o percentual/tempo **não alocado** restante. | M |
| RF10 | O sistema deve impedir salvar alocações cuja soma ultrapasse 100%. | M |
| RF11 | O sistema deve exibir a distribuição do TD em um gráfico (rosca ou barra empilhada) com as cores das tags. | S |
| RF12 | O usuário pode reordenar as tags na lista. | C |
| RF13 | O usuário pode arquivar uma tag (some do planejamento, mas mantém o histórico). | S |
| RF14 | O app deve oferecer tags sugeridas na primeira execução (Trabalho, Estudo, Exercício, Lazer), que o usuário pode aceitar ou descartar. | C |

### 3.3 Timer

| ID | Requisito | Prioridade |
|---|---|---|
| RF15 | Cada tag deve ter um timer regressivo iniciando no seu tempo restante do dia. | M |
| RF16 | O usuário deve poder **iniciar**, **pausar**, **retomar** e **parar** o timer de uma tag. | M |
| RF17 | O timer deve exibir o tempo restante em HH:MM:SS e uma barra/anel de progresso (consumido ÷ alocado). | M |
| RF18 | Enquanto um timer roda, o app deve manter uma **notificação persistente** com o nome da tag, o tempo restante e botões de pausar/parar. | M |
| RF19 | O timer deve continuar contando com o app em segundo plano, fechado pelo usuário, ou com a tela bloqueada. | M |
| RF20 | Ao zerar, o sistema deve emitir notificação com som e/ou vibração (conforme configuração). | M |
| RF21 | Depois de zerar, o timer pode continuar contando como **excedente**, exibido de forma diferenciada (ex.: "+00:12:30" em outra cor). | S |
| RF22 | O usuário pode configurar avisos antecipados (ex.: faltando 10 min). | C |
| RF23 | O usuário pode lançar tempo manualmente para uma tag (ex.: esqueceu de ligar o timer), informando duração e horário. | S |
| RF24 | O usuário pode desfazer/editar sessões registradas no dia corrente. | C |

### 3.4 Painel do dia

| ID | Requisito | Prioridade |
|---|---|---|
| RF25 | A tela inicial deve mostrar: TD do dia, tempo decorrido desde o início do dia, e para cada tag o alocado, consumido e restante. | M |
| RF26 | A tela inicial deve destacar o timer ativo, se houver, com acesso rápido a pausar. | M |
| RF27 | O painel deve mostrar quanto falta para o fim do dia lógico e alertar se a soma dos restantes das tags não cabe mais nesse tempo. | S |

### 3.5 Histórico e estatísticas

| ID | Requisito | Prioridade |
|---|---|---|
| RF28 | O sistema deve registrar cada sessão (tag, início, fim, duração, origem: timer ou manual). | M |
| RF29 | Ao virar o dia lógico, o sistema deve fechar o dia e gravar um resumo (alocado × consumido por tag). | M |
| RF30 | O usuário deve consultar o histórico por dia. | S |
| RF31 | O usuário deve ver estatísticas semanais e mensais: % de cumprimento por tag e média diária. | S |
| RF32 | O usuário pode exportar o histórico em CSV ou JSON. | C |
| RF33 | O usuário pode fazer backup e restaurar todos os dados por arquivo local. | C |

### 3.6 Configurações gerais

| ID | Requisito | Prioridade |
|---|---|---|
| RF34 | O usuário pode ativar/desativar som, vibração e o comportamento de excedente. | S |
| RF35 | O app deve suportar tema claro e escuro, seguindo o sistema por padrão. | S |
| RF36 | O usuário pode apagar todos os dados do app. | M |

### 3.7 Fora desta versão (W)

- Sincronização entre aparelhos / nuvem.
- Widgets na tela inicial do Android.
- Integração com calendário.
- Bloqueio de apps ou modo foco.
- Pomodoro dentro de uma tag.

---

## 4. Regras de negócio (RN)

### 4.1 Cálculo do tempo

| ID | Regra |
|---|---|
| RN01 | **Tempo disponível:** `TD = 86.400 s − soma das durações dos blocos reservados (s)`. |
| RN02 | **Limites dos blocos:** cada bloco tem duração > 0 (passos de 15 min na interface); a soma de todos os blocos é de no máximo **23h**, para sobrar ao menos 1h disponível. |
| RN03 | **Nomes dos blocos:** de 1 a 30 caracteres e **únicos** entre os blocos (sem diferenciar maiúsculas, minúsculas e acentos). Máximo de **20 blocos**. Um bloco pode ter o mesmo nome de uma tag. |
| RN04 | **Tempo alocado:** `alocado = floor(TD × porcentagem)`, calculado em **segundos inteiros**. Exemplo: TD = 57.600 s, 10% → 5.760 s = 1h36min. |
| RN05 | **Precisão da porcentagem:** uma casa decimal (0,1%). Internamente, guardada como inteiro em **milésimos** (10% = 100) para evitar erro de ponto flutuante. |
| RN06 | **Sobras de arredondamento** (frações de segundo descartadas pelo `floor`) vão para o "Não alocado". A soma dos alocados nunca ultrapassa o TD. |
| RN07 | **Conversão duração → porcentagem (RF08):** `porcentagem = round(duração ÷ TD, 0,1%)`. O tempo alocado é então recalculado a partir da porcentagem arredondada (RN04); o valor exibido pode diferir em segundos do digitado. |
| RN08 | **Exibição:** o timer mostra `HH:MM:SS`; resumos e listas mostram `XhYYmin`, com os minutos truncados. |

### 4.2 Tags e alocações

| ID | Regra |
|---|---|
| RN09 | A soma das porcentagens das tags ativas deve ser **≤ 100%**. A parte restante é sempre "Não alocado". |
| RN10 | Cada tag ativa deve ter porcentagem **≥ 0,1%**. |
| RN11 | O nome da tag é obrigatório, tem de 1 a 30 caracteres e é **único** entre as tags (sem diferenciar maiúsculas, minúsculas e acentos). |
| RN12 | Máximo de **20 tags ativas**. |
| RN13 | "Não alocado" é uma categoria do sistema: não pode ser renomeada, excluída nem receber timer. |
| RN14 | **Excluir** uma tag com histórico exige confirmação e apaga o histórico dela. **Arquivar** mantém o histórico e libera a porcentagem. |
| RN15 | **Alteração no meio do dia:** mudar a porcentagem de uma tag recalcula o alocado imediatamente; o consumido é preservado. Se o novo alocado for menor que o consumido, a diferença passa a ser excedente. |
| RN16 | **Alteração do tempo reservado ou do início do dia no meio do dia:** vale a partir do **próximo dia lógico**, para não alterar a base do dia em andamento. |

### 4.3 Timer

| ID | Regra |
|---|---|
| RN17 | **Apenas um timer ativo por vez.** Iniciar outra tag pausa automaticamente a que estava rodando (o tempo é exclusivo: não se estuda e trabalha ao mesmo tempo). |
| RN18 | O timer é baseado em **marcação de tempo**, não em contagem de ticks: ao iniciar, grava-se o instante de início; o consumido é `agora − início + consumido_anterior`. Assim, o timer sobrevive ao fechamento do app, ao encerramento do processo pelo sistema e à reinicialização do aparelho. |
| RN19 | **Pausar** encerra a sessão atual e soma sua duração ao consumido. **Parar** faz o mesmo e zera o estado "ativo" da tag. O consumido do dia é mantido nos dois casos. |
| RN20 | **Fim do timer:** ao atingir o alocado, o sistema notifica (RF20). Se o excedente estiver ativado, a contagem segue; senão, a sessão é encerrada automaticamente no instante exato em que zerou. |
| RN21 | **Virada do dia lógico com timer ativo:** a sessão é dividida no horário de início do dia — a parte anterior fica no dia que termina e a posterior inicia o novo dia, com o timer continuando a rodar. |
| RN22 | **Tempo durante blocos reservados:** os blocos não têm horário fixo; sessões podem acontecer a qualquer hora e contam normalmente no dia lógico corrente. |
| RN23 | **Lançamento manual (RF23):** não pode sobrepor outra sessão já registrada nem ficar no futuro. |
| RN24 | **Mudança de relógio/fuso:** se o relógio do aparelho retroceder durante uma sessão ativa, a sessão é encerrada no último instante válido conhecido e o usuário é avisado. Durações nunca podem ser negativas. |

### 4.4 Dia lógico e histórico

| ID | Regra |
|---|---|
| RN25 | O dia lógico começa no horário de início do dia configurado (padrão 04:00) e é identificado pela data em que começou. |
| RN26 | Na virada, o consumido de todas as tags é zerado e um novo dia é criado com as alocações vigentes. |
| RN27 | Se o app ficar sem abrir por vários dias, ao abrir ele cria os dias faltantes como "sem registro" (consumido zero), sem pedir confirmação. |
| RN28 | O histórico guarda uma cópia (snapshot) de TD, porcentagens e alocados de cada dia, para que mudanças futuras não alterem o passado. |
| RN29 | **% de cumprimento** de uma tag no dia = `min(consumido ÷ alocado, 100%)`. O excedente é exibido à parte. |
| RN30 | Por padrão, o histórico é mantido indefinidamente. O usuário pode configurar a remoção automática após 90, 180 ou 365 dias. |

---

## 5. Requisitos não funcionais (RNF)

### 5.1 Plataforma e tecnologia

| ID | Requisito |
|---|---|
| RNF01 | O app é **exclusivamente Android**. Versão mínima: **Android 8.0 (API 26)**. Target: a API exigida pela Google Play na data da publicação (hoje 35+). |
| RNF02 | Todo o domínio, persistência e interface devem ser escritos em **Rust estável**. |
| RNF03 | A integração com APIs do Android (notificações, alarmes, permissões, boot) deve ser feita via **JNI** a partir do Rust. Se alguma API exigir uma classe Java/Kotlin declarada no manifesto (ex.: `BroadcastReceiver`), essa classe deve ser um **shim mínimo, sem regra de negócio**, que apenas repassa a chamada para o Rust. |
| RNF04 | ABIs suportadas: `arm64-v8a` (obrigatória) e `x86_64` (emulador). `armeabi-v7a` (32 bits) fica de fora: o Skia usado pelo Slint não tem binário pronto para ele (ver seção 11). |
| RNF05 | O código deve ser separado em: **`core`** (Rust puro, sem dependência de Android: cálculos, regras, estado do timer), **`storage`** (persistência) e **`app`** (UI + integração Android). O `core` deve compilar e rodar testes no desktop. |

### 5.2 Confiabilidade e precisão

| ID | Requisito |
|---|---|
| RNF06 | O tempo exibido pelo timer pode divergir no máximo **±1 s** do tempo real. |
| RNF07 | Nenhuma sessão pode ser perdida se o processo for encerrado pelo sistema, se o app for fechado ou se o aparelho reiniciar (RN18). |
| RNF08 | A notificação de fim do timer deve disparar com atraso máximo de **5 s**, mesmo com o app fechado e o aparelho em Doze (usando alarme exato; o app deve pedir a permissão `SCHEDULE_EXACT_ALARM`/`USE_EXACT_ALARM` quando aplicável). |
| RNF09 | Após reiniciar o aparelho, um timer que estava ativo deve ter sua notificação e alarme reagendados (`RECEIVE_BOOT_COMPLETED`). |
| RNF10 | Gravações no banco devem ser **transacionais**: uma falha no meio de uma operação não pode deixar dados inconsistentes. |

### 5.3 Desempenho e consumo

| ID | Requisito |
|---|---|
| RNF11 | Abertura a frio em até **2 s** em aparelho intermediário (ex.: 4 GB de RAM, 2021+). |
| RNF12 | Ações da interface (iniciar, pausar, salvar tag) devem responder em até **100 ms**. |
| RNF13 | Com o app em segundo plano, o timer **não** deve manter a CPU acordada (sem wake lock contínuo). O tempo é calculado por marcação (RN18) e o fim é avisado por alarme. |
| RNF14 | O APK deve ter no máximo **20 MB** por ABI. |

### 5.4 Privacidade e segurança

| ID | Requisito |
|---|---|
| RNF15 | O app funciona **100% offline** e **não solicita a permissão `INTERNET`**. |
| RNF16 | Os dados ficam apenas no armazenamento interno privado do app. Não há coleta de dados, analytics nem anúncios. |
| RNF17 | O app pede só as permissões estritamente necessárias: notificações (`POST_NOTIFICATIONS`, API 33+), serviço em primeiro plano, alarme exato, boot e vibração. Recusar uma permissão degrada a funcionalidade sem travar o app, e o motivo é explicado ao usuário. |

### 5.5 Usabilidade e acessibilidade

| ID | Requisito |
|---|---|
| RNF18 | Idioma padrão **português do Brasil**, com textos isolados em arquivos de tradução para permitir outros idiomas. |
| RNF19 | Compatível com **TalkBack**: todo elemento interativo tem descrição acessível. |
| RNF20 | A interface deve respeitar o tamanho de fonte do sistema até 200% sem quebrar o layout. |
| RNF21 | A cor nunca é a única forma de identificar uma tag: há sempre o nome junto. O contraste mínimo é **4,5:1** (WCAG AA). |
| RNF22 | Iniciar o timer de uma tag deve exigir no máximo **2 toques** a partir da tela inicial. |

### 5.6 Manutenibilidade e qualidade

| ID | Requisito |
|---|---|
| RNF23 | O crate `core` deve ter cobertura de testes de no mínimo **80%**, incluindo todos os cálculos (RN01–RN08) e as regras de timer (RN17–RN24). |
| RNF24 | O código deve passar em `cargo fmt --check` e `cargo clippy -- -D warnings`. |
| RNF25 | O esquema do banco deve ser versionado com migrações; atualizar o app não pode perder dados. |
| RNF26 | O build do APK deve ser reproduzível com um único comando documentado no README. |

---

## 6. Stack adotada

| Camada | Escolha | Observação |
|---|---|---|
| UI | **Slint 1.18** (estilo Material) | Renderiza com Skia, sem WebView. Tema claro/escuro, áreas seguras e teclado virtual são tratados pelo backend Android do Slint. |
| Ciclo de vida Android | `android-activity` 0.6 (via Slint) + `NativeActivity` | Nenhuma Activity Java própria. |
| Ponte com Java | `jni` 0.21 | Rust chama `com.projetotempo.Bridge`; receptores Java chamam `Native.onCommand` (Rust). |
| Banco | **SQLite** via `rusqlite` 0.41 (`bundled`) | Migrações por `PRAGMA user_version`, modo WAL. |
| Datas/horas | `chrono` 0.4 | Fuso local via `Clock` (testável com relógio manual). |
| Build | `cargo-ndk` 4 + Gradle 9.9 / AGP 9.5 | Script único: `scripts/build-android.ps1` (RNF26). |

## 7. Modelo de dados (conceitual)

```
Configuracao
  inicio_do_dia       HH:MM   (padrão 04:00)
  excedente_ativo     bool
  som / vibracao      bool
  retencao_dias       int | null

BlocoReservado         (RF01)
  id, nome, duracao_segundos, ordem

Tag
  id, nome, cor, ordem
  porcentagem_milesimos   int   (10% = 100)
  arquivada               bool
  criada_em

DiaLogico
  id, data_inicio, td_segundos (snapshot)
  fechado bool

AlocacaoDia            (snapshot de cada tag no dia — RN28)
  dia_id, tag_id, porcentagem_milesimos, alocado_segundos

Sessao
  id, dia_id, tag_id
  inicio_utc, fim_utc | null   (null = timer ativo)
  origem: TIMER | MANUAL
```

---

## 8. Fluxos principais

### 8.1 Primeira execução
1. Boas-vindas → lista de blocos reservados, já com a sugestão "Sono 8h" (o usuário edita, apaga ou cria outros) e o horário de início do dia.
2. O app mostra o TD calculado ("8h reservadas · 16h disponíveis por dia").
3. Cria/aceita tags e define as porcentagens, vendo a duração em tempo real.
4. Pede a permissão de notificação, explicando o motivo.
5. Vai para o painel do dia.

### 8.2 Usar um timer
1. No painel, toca na tag "Estudo" (1h36min restantes) → **Iniciar**.
2. Aparece a notificação persistente; o usuário pode sair do app.
3. Toca em "Trabalho" → o timer de Estudo pausa sozinho (RN17) e o de Trabalho começa.
4. Quando Trabalho zera → notificação de fim; segue como excedente ou para (RN20).

### 8.3 Virada do dia
1. Chega o horário de início do dia → o dia anterior é fechado e gravado no histórico (RN26, RN28).
2. Um timer ativo é dividido entre os dois dias (RN21).
3. O painel mostra o novo dia com todos os alocados cheios.

---

## 9. Critérios de aceite (amostra)

| Cenário | Dado | Quando | Então |
|---|---|---|---|
| Cálculo básico | bloco "Sono 8h" | abre o painel | TD = 16h00min |
| Vários blocos | "Sono 8h" e "Faculdade 4h" | abre o painel no dia seguinte | TD = 12h00min |
| Limite de 23h | blocos somam 23h | tenta criar outro bloco | erro "podem somar no máximo 23h" |
| Porcentagem → tempo | TD = 16h | cria tag com 10% | exibe 1h36min |
| Decimal | TD = 16h | cria tag com 12,3% | alocado = 7.084 s (1h58min04s) |
| Limite de 100% | tags somam 95% | tenta criar tag com 10% | salvar fica bloqueado com mensagem "Restam apenas 5%" |
| Um timer por vez | Estudo rodando | inicia Trabalho | Estudo pausa, Trabalho roda |
| Sobrevive ao kill | timer rodando há 10 min | o sistema mata o app e ele é reaberto 5 min depois | consumido = 15 min |
| Reboot | timer rodando | o aparelho reinicia | notificação e alarme de fim reaparecem |
| Mudança no reservado | meio do dia, "Sono" alterado para 7h | — | TD de hoje continua 16h; amanhã será 17h |

---

## 10. Decisões tomadas na implementação

1. **Excedente e "Não alocado":** o excedente é exibido à parte, na própria tag; não desconta do "Não alocado".
2. **Blocos reservados sem horário:** um bloco é só uma duração (ex.: 8h), não um intervalo no relógio. O dia vira no horário de início configurado, não no "acordar". Bancos da versão 0.1 são convertidos automaticamente: o par dormir/acordar vira o bloco "Sono" e o acordar vira o início do dia.
3. **Rotinas por dia da semana (RF05):** fora desta versão.
4. **Shim Java:** aceito, em Java (sem Kotlin, para não exigir plugin extra). São 4 classes: `Bridge` (APIs do Android), `Native` (carrega a biblioteca Rust), `TimerReceiver` (botões da notificação e alarme) e `SystemReceiver` (boot, atualização, relógio/fuso). Nenhuma tem regra de negócio.
5. **Sem serviço em primeiro plano:** como o timer é por marcação de tempo (RN18), a notificação persistente é uma notificação `ongoing` com o **cronômetro do próprio sistema** (contagem regressiva até o fim; depois, contagem do excedente), e o fim é avisado por `AlarmManager`. Isso atende RF18/RF19 sem wake lock (RNF13) e sem as restrições de `foregroundServiceType` do Android 14+. No Android 14+, o usuário pode dispensar essa notificação; o timer continua correndo mesmo assim.
6. **Framework de UI:** Slint.

## 11. Situação da implementação

### 11.1 Requisitos funcionais

| Situação | Requisitos |
|---|---|
| Implementado | RF01–RF04, RF06–RF21, RF23, RF25–RF32, RF34–RF36 |
| Parcial | **RF24**: é possível apagar sessões do dia; editar não (apague e lance de novo com RF23). **RF32**: exporta CSV (sessões); JSON não. |
| Não implementado (desejáveis) | **RF05** (blocos por dia da semana), **RF22** (aviso antecipado), **RF33** (backup/restauração por arquivo) |

### 11.2 Requisitos não funcionais — observações

| ID | Situação |
|---|---|
| RNF04 | Apenas `arm64-v8a` e `x86_64`. |
| RNF08 | Atendido quando o usuário concede "Alarmes e lembretes" (Ajustes → Permissões). Sem essa permissão, o app usa alarme inexato, que pode atrasar alguns minutos em repouso. `USE_EXACT_ALARM` não é usado porque a Play Store o restringe a apps de despertador/calendário. |
| RNF14 | APK `arm64-v8a` ≈ 17 MB, `x86_64` ≈ 18 MB (gerados separadamente; há também um universal ≈ 35 MB). |
| RNF18 | Textos em pt-BR direto no código (`ui/*.slint` e `present.rs`). A extração para arquivos de tradução (`@tr()` do Slint) fica para quando houver um segundo idioma. |
| RNF20 | Depende do suporte do Slint à escala de fonte do Android; não verificado em aparelho. |
| RNF23 | 64 testes automatizados (core + serviço); a cobertura percentual não foi medida. |
| RNF11, RNF12 | Não medidos em aparelho real. |
