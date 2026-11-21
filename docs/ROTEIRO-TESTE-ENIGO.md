# Roteiro: teste supervisionado da entrada remota (enigo)

**Para quê:** ver, com o mouse e o teclado reais, se o clique cai onde você clicou (W8, escala de DPI) e
se só administrador injeta (W7). É o único jeito de provar a ligação dentro de `InjetorReal::clique`.

**Quanto tempo:** 15 a 20 minutos. **Risco:** o mouse e o teclado DESTE computador vão se mexer de verdade.

## Antes de começar (segurança)

- Feche o que for importante e tire o que está aberto de sensível da tela. O cockpit vai mostrar a sua tela inteira.
- Deixe só um Bloco de Notas aberto e vazio, sem salvar nada.
- **Parada de emergência:** na janela do gateway (Terminal 1), aperte `Ctrl+C`. O mouse físico continua mandando o tempo todo.
- As variáveis abaixo valem só nas janelas do PowerShell em que você as digitar. Fechou a janela, desligou.
- **Nunca clique** no canto inferior direito (mostra a área de trabalho), no canto inferior esquerdo (menu Iniciar) nem na barra de tarefas pelo cockpit.

## Terminal 1: ligar o gateway

```powershell
cd C:\Users\Paulo\zylos-next
```

```powershell
$env:ZYLOS_AUTH_SECRET = -join ((48..57 + 65..90 + 97..122) | Get-Random -Count 48 | ForEach-Object { [char]$_ })
```

```powershell
$env:ZYLOS_AUTH_BOOTSTRAP_TOKEN = -join ((48..57 + 65..90 + 97..122) | Get-Random -Count 48 | ForEach-Object { [char]$_ })
```

```powershell
Set-Clipboard $env:ZYLOS_AUTH_BOOTSTRAP_TOKEN
```

```powershell
$env:ZYLOS_COMPUTER_VIEW_INPUT = "1"; $env:ZYLOS_COMPUTER_VIEW_CAPTURE = "1"
```

```powershell
cargo run -p zylos-gateway --features input-injection
```

**Esperado:** o gateway sobe em `127.0.0.1:3000` e o log diz que a captura local está LIGADA. A senha de
bootstrap ficou na área de transferência (não aparece na tela). Se não subir, pare aqui e me mande o texto do erro.

## Terminal 2: pegar o token de administrador

O formulário de login do cockpit só pede o papel `editor`, e assumir controle exige `admin`. Por isso o token sai daqui.

```powershell
$b = Get-Clipboard
```

```powershell
$r = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:3000/api/v1/auth/login -Headers @{ Authorization = "Bearer $b" } -ContentType "application/json" -Body '{"user_id":"teste","tenant_id":"tenant-default","roles":["admin"]}'
```

```powershell
Set-Clipboard $r.access_token
```

**Esperado:** nenhuma mensagem de erro. O token de admin está na área de transferência.

## Terminal 2: criar a sessão da tela local

Sessão sem servidor VNC é a que o gateway fotografa (monitor primário).

```powershell
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:3000/api/v1/computer-view/sessions -Headers @{ Authorization = "Bearer $($r.access_token)" } -ContentType "application/json" -Body '{"bot_id":"teste-enigo"}'
```

**Esperado:** devolve um objeto com `session` e um `id`.

## Terminal 3: abrir o cockpit (porta 3100, porque a 3000 é do gateway)

```powershell
cd C:\Users\Paulo\zylos-next\apps\zylos-cockpit
```

```powershell
trunk serve --port 3100
```

Se o comando `trunk` não existir, pare e me avise (não vou pedir para instalar nada sem você decidir).

## No navegador

1. Abra `http://localhost:3100`.
2. Aperte `F12`, aba **Console**, cole a linha abaixo trocando `COLE_AQUI` pelo token (Ctrl+V) e aperte Enter:
   `localStorage.setItem('zylos_token', 'COLE_AQUI')`
3. Recarregue a página (`F5`).
4. Vá ao painel do computador remoto (o que tem o botão **Assumir Controle**) e escolha a sessão `teste-enigo`.
   *Não conferi qual item do menu leva a esse painel; se não achar, me diga o que aparece.*
5. **Esperado:** você vê a sua própria tela no painel, atualizando a cada ~1,5 s.

## Os testes (um de cada vez, olhando o ponteiro REAL)

Anote o resultado de cada um.

| # | O que fazer | Esperado | Se falhar |
|---|---|---|---|
| 1 | Clique em **Assumir Controle**. | O painel diz "Você está no controle". | Anote a mensagem exata que aparecer. |
| 2 | No painel, clique no **centro** da imagem da tela. | O ponteiro real vai ao centro da tela física. | Anote onde ele caiu (mais à direita? mais abaixo?). Falha aqui = o W8 não está resolvido. |
| 3 | Clique a 1/4 da largura e 1/4 da altura (parte de cima à esquerda), numa área vazia. | O ponteiro real cai nesse ponto. | Mesma anotação do passo 2. |
| 4 | Clique a 3/4 da largura e 3/4 da altura, numa área vazia. | Idem. | Idem. |
| 5 | Clique dentro da janela do Bloco de Notas, pelo painel, e digite `teste zylos`. | O texto aparece no Bloco de Notas. | Anote o que apareceu. |
| 6 | Pelo painel, aperte `Ctrl+Esc` ou `Alt+Tab`. | **Nada** acontece (esses atalhos são bloqueados de propósito). | Se o menu Iniciar ou o Alt+Tab abrir, é falha grave: pare e me avise. |
| 7 | Clique em **Liberar Controle**, depois clique na imagem. | Nenhum clique chega ao mouse real. | Se o mouse se mexer, é falha grave. |

**Teste opcional do papel (W7):** repita o passo "pegar o token" com `"roles":["editor"]`, ponha esse token no
navegador e tente **Assumir Controle**. **Esperado:** recusa ("exige entrar como administrador"). Nada se mexe.

## Encerrar (sempre)

1. Terminal 1: `Ctrl+C`. Terminal 3: `Ctrl+C`.
2. No Console do navegador: `localStorage.removeItem('zylos_token')`.
3. Feche as janelas do PowerShell e limpe a área de transferência (copie qualquer texto).

## O que me trazer de volta

- Os resultados de 1 a 7 (passou, ou onde o ponteiro caiu).
- A tela do navegador com o painel, se falhar algo.
- O texto de qualquer erro dos terminais 1 e 3.

## O que este teste NÃO cobre

- Só o monitor primário a 125% desta máquina. Vários monitores com escalas diferentes e tela rotacionada ficam de fora.
- Não testa servidor VNC real, nem macOS ou Linux.
- Os comandos acima eu li no código, mas **não os executei**: o rumo exato da tela no cockpit (passo 4) e o
  `trunk` instalado não foram conferidos.
