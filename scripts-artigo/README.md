# Scripts de Automação dos Experimentos (Artigo "Compile Once, Deploy Anywhere")

Este diretório contém os scripts Bash criados para executar de forma automatizada e padronizada a bateria de testes de portabilidade, placement e footprint do runtime, conforme descrito no roteiro de experimentos.

---

## Estrutura dos Scripts

* **[run_E1.sh](file:///c:/repos/pgc-rust/scripts-artigo/run_E1.sh)**: Executa o Experimento E1 (Portabilidade e Performance) comparando o tempo de processamento offline e o consumo máximo de memória RAM (RSS) do processador em modo batch nas opções: **Nativo**, **WASM Interpretado** e **WASM AOT**.
* **[run_E3.sh](file:///c:/repos/pgc-rust/scripts-artigo/run_E3.sh)**: Executa o Experimento E3 (Footprint do Runtime), medindo o tamanho físico em disco de cada artefato (.wasm, .wasm AOT, nativo) e calculando o tempo de inicialização a frio (*cold start*) de cada um até estarem prontos para responder requisições HTTP.
* **[run_E2_server.sh](file:///c:/repos/pgc-rust/scripts-artigo/run_E2_server.sh)**: Lado do servidor (Nuvem x86_64) para o Experimento E2. Sobe os listeners HTTP em background de acordo com o cenário sob teste.
* **[run_E2_pi.sh](file:///c:/repos/pgc-rust/scripts-artigo/run_E2_pi.sh)**: Lado do gateway (Raspberry Pi ARM64) para o Experimento E2. Configura a limitação de rede no backhaul com `tc netem`, inicializa o listener local (processador ou proxy lora), dispara o simulador de sensores de campo de forma acelerada, captura os pacotes de upload via `tcpdump` e registra o total de bytes.

---

## Como Executar

### 1. Compilação Automatizada

Os scripts foram desenvolvidos para serem **totalmente autossuficientes**. No início de cada execução, o script verifica se os compiladores e dependências de sistema estão presentes e realiza automaticamente o `cargo build --release` (tanto nativo quanto para o target `wasm32-wasip1`), copiando os binários finais prontos para a raiz do repositório.

Portanto, **não é necessário compilar os códigos manualmente** antes de rodar os testes. Apenas garanta que o código fonte do repositório esteja atualizado.

---

### 2. Rodando o E1 (Portabilidade e Performance)

Execute o script `run_E1.sh` tanto na Raspberry Pi quanto no Servidor Nuvem:

```bash
chmod +x scripts-artigo/run_E1.sh
./scripts-artigo/run_E1.sh
```

Os resultados serão gravados de forma bruta no CSV `resultados-artigo/E1/resultados_E1.csv`.

---

### 3. Rodando o E3 (Footprint e Cold Start)

Execute o script `run_E3.sh` na Raspberry Pi:

```bash
chmod +x scripts-artigo/run_E3.sh
./scripts-artigo/run_E3.sh
```

Os resultados do tamanho físico e do tempo de inicialização em milissegundos serão gravados em `resultados-artigo/E3/tamanhos.csv` e `resultados-artigo/E3/cold_start.csv`.

---

### 4. Rodando o E2 (Placement e Latência)

#### Passo A: Configuração na Nuvem (Servidor)
Descubra o endereço IP do seu servidor Nuvem (ex: `192.168.1.100`) e inicie os listeners correspondentes:

* **Para rodar o Cenário P1 (Filtro na Fog/Borda)**:
  ```bash
  chmod +x scripts-artigo/run_E2_server.sh
  ./scripts-artigo/run_E2_server.sh P1
  ```
  *(O receiver ficará escutando na porta 8082)*.

* **Para rodar o Cenário P2 (Filtro na Cloud/Nuvem)**:
  ```bash
  chmod +x scripts-artigo/run_E2_server.sh
  ./scripts-artigo/run_E2_server.sh P2
  ```
  *(O receiver e o processador em nuvem ficarão escutando nas portas 8082 e 8081)*.

* **Para encerrar os processos ao fim do experimento**:
  ```bash
  ./scripts-artigo/run_E2_server.sh stop
  ```

#### Passo B: Execução no Gateway (Raspberry Pi)
No Pi, execute a automação passando o cenário desejado, o IP do servidor e a interface física de rede que comunica com o servidor (ex: `eth0` ou `wlan0`):

* **Para rodar o Cenário P1 (Filtro na Fog/Borda)**:
  ```bash
  chmod +x scripts-artigo/run_E2_pi.sh
  ./scripts-artigo/run_E2_pi.sh P1 <IP_DO_SERVIDOR> eth0
  ```

* **Para rodar o Cenário P2 (Filtro na Cloud/Nuvem)**:
  ```bash
  chmod +x scripts-artigo/run_E2_pi.sh
  ./scripts-artigo/run_E2_pi.sh P2 <IP_DO_SERVIDOR> eth0
  ```

* **Para restaurar a interface de rede se houver travamentos**:
  ```bash
  ./scripts-artigo/run_E2_pi.sh stop <IP_DO_SERVIDOR> eth0
  ```

Os resultados de tempo e tráfego upstream em bytes serão compilados no arquivo `resultados-artigo/E2/resultados_E2.csv`.

---

### 5. (Opcional) Comparação com Docker (Cenário E3)

Para comparar o footprint de memória e o tempo de boot (*cold start*) do WasmEdge rodando nativo na máquina contra um container Docker rodando o Rust Nativo (Linux ELF):

```bash
chmod +x scripts-artigo/run_docker_compare.sh
./scripts-artigo/run_docker_compare.sh
```

Os resultados (tempo de inicialização em milissegundos e RAM em MB de 10 rodadas) serão gravados em `resultados-artigo/E3/docker_resultados.csv`.

