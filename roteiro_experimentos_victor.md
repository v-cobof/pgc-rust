# Roteiro de Novos Experimentos — Artigo "Compile Once, Deploy Anywhere"

**Para:** Victor Cobo Figueiro
**Contexto:** o artigo reposiciona o trabalho do PGC: o foco deixa de ser o filtro
kNN+RLE e passa a ser a **flexibilidade de deployment que o WASM traz ao
continuum** (o mesmo binário rodando em qualquer estágio). O filtro vira o
*workload* de demonstração. Para sustentar essa tese, precisamos de 3 novos
experimentos (E1, E2, E3), que alimentam as seções V-A, V-B e V-C do artigo.
Os resultados do PGC (compressão, kNN vs. RLE) entram como seção V-D, já prontos.

**Princípio geral para TODOS os experimentos:**
- Cada medição repetida **10 vezes**; reportar **média ± desvio padrão**.
- Descartar a 1ª execução de cada bateria (aquecimento de cache).
- Salvar TUDO em CSV bruto (uma linha por repetição), num diretório
  `resultados-artigo/E1/`, `E2/`, `E3/` no repositório. Gráfico se faz depois;
  dado bruto perdido não se recupera.
- Anotar num `ambiente.md`: versão do WasmEdge, do Rust, do SO, modelo exato
  do hardware, e o **SHA-256 do binário .wasm** usado (isso vai para o paper).

---

## Hardware necessário

| Papel no continuum | Máquina | Observação |
|---|---|---|
| Fog gateway (S3-Fog) | Raspberry Pi 4 ou 5, ARM64, ≥4 GB | Falar com o Carlos sobre disponibilidade no laboratório |
| Cloud (S5-Cloud) | Servidor/desktop x86_64 Linux | Pode ser máquina do lab; NÃO usar WSL nos experimentos finais |
| Rede | As duas máquinas na mesma rede local | O enlace restrito será **emulado** com `tc netem` (ver E2) |

> Importante: os experimentos do PGC rodaram em WSL. Para o artigo, tudo deve
> rodar em Linux nativo nas duas máquinas, senão um revisor derruba a medição
> de latência/memória.

---

## E1 — Portabilidade: um binário, duas arquiteturas (seção V-A)

**Pergunta que responde:** o MESMO arquivo `.wasm`, sem recompilar, executa em
ARM e x86? E a que custo em relação ao código nativo?

### Passos

1. Compilar o `processador` UMA vez, em qualquer máquina:
   ```bash
   cargo build --release --target wasm32-wasip1
   sha256sum target/wasm32-wasip1/release/processador.wasm
   ```
   Anotar o hash. Copiar **este mesmo arquivo** para o Pi e para o servidor
   (via `scp`). Conferir o hash nos dois destinos — a igualdade dos hashes é a
   evidência de portabilidade que vai no texto.

2. Em cada máquina, compilar também a versão **nativa** (baseline):
   ```bash
   cargo build --release        # no Pi gera ARM64, no servidor gera x86_64
   ```

3. Em cada máquina, medir o tempo de processamento em **modo batch** com a
   base real (`entrada.csv`, 44.023 registros, lote 720):
   ```bash
   # WASM interpretado
   time wasmedge --dir . processador.wasm batch entrada.csv stats.json --tamanho-pacote 720

   # WASM AOT (compilar o .wasm para o alvo local — ainda é o mesmo bytecode!)
   wasmedge compile processador.wasm processador_aot.wasm
   time wasmedge --dir . processador_aot.wasm batch entrada.csv stats.json --tamanho-pacote 720

   # Nativo
   time ./target/release/processador batch entrada.csv stats.json --tamanho-pacote 720
   ```
   10 repetições de cada. Usar `/usr/bin/time -v` (não o `time` do shell) para
   já capturar também o pico de memória (serve para o E3).

### Saída esperada (Tabela do paper)

| Nó | Nativo (ms) | WASM interp. (ms) | WASM AOT (ms) | Overhead AOT |
|---|---|---|---|---|
| Pi (ARM64) | ... | ... | ... | ~1,1–1,5x esperado |
| Servidor (x86_64) | ... | ... | ... | ... |

Se o overhead do AOT ficar na faixa 1,1–1,5x, está alinhado com a literatura e
é um ótimo resultado. Se der muito acima, me avise ANTES de seguir — pode ser
config do WasmEdge.

---

## E2 — Trade-offs de placement: filtro na Fog vs. na Cloud (seção V-B)

**Pergunta que responde:** colocar o filtro na borda vs. na nuvem muda o quê,
quantitativamente, no enlace restrito e na latência?

### Cenários

- **P1 (filtro na Fog):** `simulador` → `processador` **no Pi** → `receiver`
  no servidor. Só o payload filtrado/comprimido atravessa o enlace restrito.
- **P2 (filtro na Cloud):** `simulador` → relay no Pi (usar o módulo `LoRa`
  como proxy puro) → `processador` + `receiver` **no servidor**. Dados brutos
  em JSON atravessam o enlace restrito.

Nos dois cenários o `.wasm` do processador é **o mesmo arquivo** do E1 — isso
deve ficar explícito no texto do paper.

### Emulação do enlace restrito

No Pi, restringir a interface de saída para simular um backhaul rural
(justificável no paper como enlace celular/satélite limitado):
```bash
sudo tc qdisc add dev eth0 root netem rate 250kbit delay 50ms
# ao final de cada bateria:
sudo tc qdisc del dev eth0 root
```
Anotar os parâmetros usados — eles entram na seção de setup. (Não dá para
emular LoRaWAN de verdade em cima de HTTP; o paper deve dizer "emulated
constrained backhaul", não "LoRaWAN emulation".)

### Métricas e como medir

1. **Bytes no enlace upstream** (a métrica principal): capturar no Pi com
   ```bash
   sudo tcpdump -i eth0 -w captura_P1.pcap 'dst host <IP_SERVIDOR> and port 8082'
   ```
   e somar bytes com `capinfos captura_P1.pcap` (pacote `tshark`). Fazer o
   mesmo em P2 (porta do processador remoto). Rodar cada cenário com a MESMA
   carga: reproduzir as 44.023 leituras da base real via simulador em
   velocidade acelerada (ex.: 1 leitura a cada 50 ms em vez de 5 s — anotar).

2. **Latência fim-a-fim:** medir no PONTO DE ORIGEM para evitar problema de
   relógio entre máquinas: o simulador registra `t0` ao enviar e o receiver
   devolve um ACK HTTP; latência = `t_ack - t0`. Alternativa mais simples:
   sincronizar os dois nós com `chrony` e comparar timestamps dos logs
   (mencionar a sincronização no paper). Escolher UMA abordagem e me contar
   qual antes de rodar tudo.

3. **Tempo de processamento por lote** em cada cenário: já sai nos logs do
   processador; se não sair, instrumentar com `chrono` (1 linha por lote).

### Saída esperada (Figuras do paper)

- Gráfico de barras: bytes upstream em P1 vs. P2 (aqui o 97,8% do PGC
  reaparece, agora como "o que se ganha ao filtrar na borda").
- Gráfico ou tabela: latência média fim-a-fim P1 vs. P2 sob o enlace emulado.

---

## E3 — Footprint do runtime no dispositivo constrito (seção V-C)

**Pergunta que responde:** o WasmEdge cabe confortavelmente num gateway de
borda? Quanto custa instanciar?

### Medições (todas no Pi)

1. **Tamanho dos artefatos:** `ls -la` do `.wasm`, do `.wasm` AOT e do binário
   nativo. Três números, uma linha de tabela.

2. **Pico de memória (RSS):** já capturado no E1 com `/usr/bin/time -v`
   (campo "Maximum resident set size"). Separar interpretado vs. AOT vs. nativo.

3. **Tempo de instanciação (cold start):** medir o tempo até a primeira
   resposta HTTP do processador em modo online:
   ```bash
   # script: registra t0, sobe o wasmedge em background, faz polling
   # com curl na rota /estatisticas até 200 OK, registra t1
   ```
   Fazer o mesmo para o binário nativo. 10 repetições, máquina "fria"
   (matar o processo entre repetições).

4. **(Opcional, se sobrar tempo) Comparação com Docker:** empacotar o binário
   nativo ARM num container mínimo (`FROM debian:slim`) e medir startup
   (`docker run` até 200 OK) e memória (`docker stats`). Se não der tempo,
   citamos números da literatura — não é bloqueante.

### Saída esperada (Tabela do paper)

| Métrica (no Pi) | Nativo | WASM interp. | WASM AOT | Docker (opc.) |
|---|---|---|---|---|
| Tamanho do artefato | | | | |
| Pico de RSS | | | | |
| Cold start até 1ª resposta | | | | |

---

## Ordem de execução e prioridade

1. **E1 primeiro** — é rápido (o código já existe, é rodar em 2 máquinas) e é
   a evidência central do título. Sem E1, não há artigo.
2. **E3 em seguida** — reaproveita as execuções do E1, custo marginal baixo.
3. **E2 por último** — é o mais trabalhoso (rede, tcpdump, dois cenários),
   mas é o que transforma o paper de "demo" em "avaliação".

## Checklist de entrega (para fecharmos os resultados)

- [ ] `ambiente.md` com versões, hardware e SHA-256 do binário
- [ ] CSVs brutos de E1, E2, E3 no repositório (`resultados-artigo/`)
- [ ] Tabela E1 preenchida (média ± dp, n=10)
- [ ] Dois gráficos do E2 (bytes upstream; latência) — pode ser matplotlib
      simples, eu ajusto o estilo depois
- [ ] Tabela E3 preenchida
- [ ] Scripts de automação commitados (quem revisar deve conseguir reproduzir)

Qualquer resultado estranho (overhead > 2x, latência invertida entre P1 e P2,
memória explodindo no Pi), **pare e me escreva antes de continuar** — melhor
diagnosticar cedo do que refazer bateria inteira.
