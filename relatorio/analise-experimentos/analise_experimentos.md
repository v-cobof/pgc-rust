# Relatório de Análise dos Experimentos de Compressão (kNN + RLE)

Este documento apresenta a análise comparativa do impacto do tamanho do lote/pacote de dados nas taxas de compressão obtidas com a técnica combinada de classificação kNN e Codificação por Comprimento de Corrida (RLE) no ecossistema PGC-Rust.

---

## 1. Contexto e Dados de Entrada

Os experimentos foram realizados utilizando uma base de **dados sintéticos com 40.000 registros** (`dados_sinteticos_gerados_rust.csv`), gerados dinamicamente em Rust através de amostragem por Estimativa de Densidade por Kernel (KDE) com largura de banda de 0,5.

### Validação dos Dados Sintéticos (Rust vs Original)
Antes de rodar as simulações, os dados gerados foram validados estatisticamente em relação à base real (`entrada.csv`):
* **Teste de Kolmogorov-Smirnov (Similaridade de Distribuição)**:
  * **Temperatura**: p-valor = `0.1385` (Como $p > 0.05$, a distribuição gerada é estatisticamente similar à original).
  * **Umidade**: p-valor = `0.8974` (Altíssima similaridade distributiva).
* **Correlação Linear (Pearson)**:
  * Correlação Original: `-0.8837`
  * Correlação Rust: `-0.8804`
  * Diferença Absoluta: `0.0033` (Correlação térmica/hídrica mantida de forma precisa).

---

## 2. Resultados das Simulações por Tamanho de Lote

A base de dados foi dividida em lotes de 5 tamanhos diferentes, simulando diferentes estratégias de armazenamento/transmissão:

| Tamanho do Pacote (Amostras) | Escopo Temporal Equivalente | Qtd. de Lotes | Tamanho Original Acumulado (Bytes) | Tamanho kNN Acumulado (Bytes) | Tamanho kNN+RLE Acumulado (Bytes) | Taxa Redução kNN+RLE (%) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`6`** | 30 segundos (Micro) | 6.667 | 1.856.634 | 40.000 | 70.218 | **96,22%** |
| **`60`** | 5 minutos | 667 | 1.850.634 | 40.000 | 68.288 | **96,31%** |
| **`720`** | 1 hora | 56 | 1.850.023 | 40.000 | 68.062 | **96,32%** |
| **`4.320`** | 6 horas | 10 | 1.849.977 | 40.000 | 67.962 | **96,33%** |
| **`20.000`** | ~14 horas (Metade) | 2 | 1.849.969 | 40.000 | 68.104 | **96,32%** |

---

## 3. Análise Crítica e Insights de Desempenho

### 3.1. Fenômeno de Expansão de Tamanho do RLE
Um resultado notório é que o tamanho final da string comprimida em RLE (~68-70 KB) é maior do que o tamanho das categorias puras geradas pelo kNN (40 KB). 
* **Explicação**: Como as siglas das categorias foram otimizadas para **1 única letra** (ex: `O` para *Optimal*, `M` para *Marginal*), o kNN bruto consome exatamente **1 byte por amostra**.
* Ao aplicar o RLE, cada elemento individual vira uma dupla `<frequência><categoria>`. Desta forma, se houver alta alternância (por exemplo, a sequência `"O M O"`), o kNN bruto consome **3 bytes** (`"OMO"`), mas o RLE consome **6 bytes** (`"1O1M1O"`). 
* O RLE apenas compensa e reduz o tamanho do kNN quando há sequências longas e repetidas da mesma categoria (ex: `"OOOOOO"` vira `"6O"`, caindo de 6 bytes para 2 bytes).

### 3.2. Custo de Serialização JSON (Overhead de Colchetes)
O tamanho acumulado dos dados originais diminui conforme o tamanho do lote aumenta. Isso se deve à sintaxe de vetor do JSON (`[...]`):
* Cada lote adiciona colchetes de início e fim à string serializada (2 bytes por lote).
* Com lote tamanho `6`, temos 6.667 lotes gerando um overhead de aproximadamente **13.334 bytes** em colchetes.
* Com lote tamanho `20.000`, o overhead cai para apenas **4 bytes** no total (apenas 2 lotes).

### 3.3. Determinação do Tamanho de Lote Ideal
A eficiência de compressão do RLE aumenta à medida que o pacote cresce, pois diminui os "efeitos de borda" (onde uma sequência repetida é quebrada na divisão de lotes). 
* O ponto ideal do experimento ocorreu no tamanho de **4.320 registros (6 horas)**, gerando o menor arquivo de saída (**67.962 bytes**).
* Para lotes maiores, como o de **20.000 registros**, o ganho é anulado pelo fato de as frequências precisarem de mais caracteres decimais (ex: representar uma contagem de `12345O` consome 6 bytes, enquanto contagens menores consomem menos dígitos).

---

## 4. Conclusão

Para cenários onde a visualização gráfica ou a transmissão offline é feita em lotes periódicos, a janela de **6 horas (4.320 amostras)** provou ser a mais otimizada, oferecendo o melhor equilíbrio entre eficiência RLE e redução de quebra de sequências em bordas. Para transmissões em tempo real de baixa latência em redes de banda estreita (como LoRa), o tamanho de **6 amostras (30 segundos)** continua sendo a opção adequada, apesar do custo ligeiramente maior de RLE.
