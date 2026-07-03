# PGC-Rust: Filtro de Dados Agrícolas com kNN e RLE no Contínuo IoT-Nuvem via WebAssembly

Este repositório contém o código-fonte e o relatório técnico desenvolvidos para o Projeto de Graduação em Ciência da Computação (UFABC). O projeto implementa de forma prática e estende a proposta teórica de Ribeiro Junior et al. (2020), estruturando um pipeline distribuído em Rust compilado para WebAssembly (WASM) para pré-processar, classificar e comprimir dados de sensores físicos na borda e na névoa (*Fog Computing*).

---

## 1. Estrutura do Repositório

O projeto está organizado nos seguintes diretórios principais:

* **[relatorio/](file:///c:/repos/pgc-rust/relatorio)**: Diretório contendo os fontes em LaTeX do artigo técnico (`Victor_Cobo_PGC.tex`), as referências bibliográficas (`references.bib`), imagens de diagramas e o PDF compilado final do trabalho (`Victor_Cobo_PGC.pdf`).
* **[processador/](file:///c:/repos/pgc-rust/processador)**: O núcleo do ecossistema. Implementado em Rust, opera em dois modos:
  * **Modo Online**: Atua como servidor HTTP (usando `hyper`) na camada de névoa, realizando a normalização Min-Max dos dados físicos, classificação $k$-NN ($k=3$) baseada nas regras agronômicas do cultivo de tomateiro e compressão RLE.
  * **Modo Offline (Lote)**: Executa um gerador estocástico off-line que estima a densidade conjunta do sensor via *Kernel Density Estimation* (KDE) e amostra dados utilizando a transformada de Box-Muller sob o sandbox WASM/WASI, avaliando o comportamento sob diferentes buffers.
* **[simulador/](file:///c:/repos/pgc-rust/simulador)**: Aplicação em Rust que simula os nós sensores de campo (*Things*). Ele lê os registros climáticos reais do arquivo `entrada.csv` e dispara requisições HTTP POST para emular leituras de sensores físicos em tempo real.
* **[LoRa/](file:///c:/repos/pgc-rust/LoRa)**: Gateway intermediário que simula a interface de recepção LoRa local, agindo como um proxy reverso para encaminhar as leituras recebidas do sensor para a máquina virtual do processador na névoa.
* **[receiver/](file:///c:/repos/pgc-rust/receiver)**: Servidor HTTP (camada de Nuvem) que recebe os pacotes compactados por RLE, realiza a deserialização JSON, computa a taxa de compressão e registra logs com carimbos de data/hora (RFC3339).
* **[gerador-de-dados/](file:///c:/repos/pgc-rust/gerador-de-dados)**: Conjunto de scripts em Python responsáveis pelo ajuste estatístico do KDE de referência, testes de hipótese de Kolmogorov-Smirnov (KS), análises de correlação de Pearson e geração dos correlogramas e curvas de densidade do artigo.

---

## 2. Pré-requisitos e Ambiente

Os testes foram executados em um ambiente Linux (Ubuntu) através do **WSL** (*Windows Subsystem for Linux*) sobre o Windows 11. Para reproduzir os testes, são necessários:

1. **Rust Toolchain**: Instalado via `rustup`.
2. **Target WebAssembly**: Adicionar suporte a compilação do WASI:
   ```bash
   rustup target add wasm32-wasip1
   ```
3. **Runtime WasmEdge**: O executor de WebAssembly seguro utilizado para a sandbox:
   ```bash
   curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash
   source $HOME/.wasmedge/env
   ```
4. **Python 3**: Para rodar as análises estatísticas:
   ```bash
   cd gerador-de-dados
   python3 -m venv venv
   source venv/bin/activate
   pip install numpy pandas scipy scikit-learn matplotlib seaborn
   ```

---

## 3. Compilação dos Binários WASM

A compilação de todos os módulos Rust para o formato binário portável WebAssembly (.wasm) é feita através do Cargo:

```bash
# Na raiz do repositório
cargo build --target wasm32-wasip1 --release
```

Os artefatos compilados serão gerados na pasta de build comum:
`target/wasm32-wasip1/release/` (`processador.wasm`, `simulador.wasm`, `lora.wasm`, `receiver.wasm`).

---

## 4. Como Executar as Simulações

### 4.1. Modo 1: Simulação Online e Distribuída (Completa)
Para simular o fluxo dinâmico distribuído em rede de ponta a ponta (Thing $\to$ LoRa Proxy $\to$ Fog Processador $\to$ Cloud Receiver), você pode usar o script de automação fornecido:

```bash
# Na raiz do repositório
chmod +x start_all.sh
./start_all.sh
```

**O que o script faz debaixo dos panos:**
1. Inicia o **receiver.wasm** (Nuvem) na porta `8082`.
2. Inicia o **processador.wasm** (Névoa) na porta `8081`.
3. Inicia o **lora.wasm** (Gateway) na porta `8080` (escutando as leituras e roteando para `8081`).
4. Inicia o **simulador.wasm** (Thing), que lê a base física `entrada.csv` e envia as requisições HTTP para a porta `8080`.

Os logs do receptor em nuvem serão gravados continuamente no arquivo de auditoria `monitoramento.txt`.

### 4.2. Modo 2: Experimento em Lote Offline (Simulador Estocástico)
Para rodar apenas o processador em modo batch/estocástico para testar a sensibilidade dos buffers e sintetizar dados com base na densidade KDE:

```bash
# Comando padrão para rodar sob o sandbox com mapeamento de diretórios local:
wasmedge --dir . target/wasm32-wasip1/release/processador.wasm 4
```

A flag `--dir .` é necessária para conceder à sandbox WASM permissão de leitura de `entrada.csv` no sistema de arquivos local e escrita dos relatórios JSON e do arquivo de amostragem `dados_sinteticos_gerados_rust.csv`.

Para automatizar a execução de múltiplas baterias com diferentes buffers (lotes de tamanho 6, 60, 720, 4320 e 20000) e coletar estatísticas:
```bash
chmod +x run_experiments.sh
./run_experiments.sh
```

---

## 5. Validação dos Dados Sintéticos (Python)

Uma vez gerada a base sintética via modo lote do processador Rust (`dados_sinteticos_gerados_rust.csv`), você pode validar a consistência física e matemática através do script de estatísticas Python:

```bash
# Ativar venv e rodar validador
source gerador-de-dados/venv/bin/activate
python gerador-de-dados/validar_dados_rust.py
```

O script executará testes de Kolmogorov-Smirnov bivariados, calculará a correlação de Pearson ($r$) real e simulada (avaliando o desvio absoluto) e salvará os gráficos científicos de dispersão e curvas de nível na pasta `relatorio/analise-experimentos/`.
