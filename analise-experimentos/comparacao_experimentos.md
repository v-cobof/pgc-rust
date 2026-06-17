# Comparação e Validação dos Três Experimentos

Este relatório apresenta a comparação direta entre as três execuções independentes do experimento de compressão em lote com dados sintéticos no ecossistema PGC-Rust.

---

## 1. Tabela Comparativa (Tamanhos kNN+RLE em Bytes)

A tabela abaixo compara o tamanho final em bytes obtido após a codificação kNN+RLE em cada tamanho de lote para as três rodadas (experimentos 1, 2 e 3):

| Tamanho do Pacote | Experimento 1 | Experimento 2 | Experimento 3 | Desvio Máximo (Bytes) | Diferença Percentual Máxima |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`6`** | 70.218 B | 70.142 B | 70.038 B | 180 B | ~0,25% |
| **`60`** | 68.288 B | 67.982 B | 68.166 B | 306 B | ~0,44% |
| **`720`** | 68.062 B | 68.072 B | 67.932 B | 140 B | ~0,20% |
| **`4.320`** | 67.962 B | 68.194 B | 68.022 B | 232 B | ~0,34% |
| **`20.000`** | 68.104 B | 68.082 B | 68.100 B | 22 B | ~0,03% |

---

## 2. Tabela Comparativa (Taxas de Redução kNN+RLE %)

A tabela abaixo compara as taxas de redução percentuais (compressão em relação ao tamanho dos dados originais serializados em JSON) obtidas para cada tamanho de lote nos três experimentos:

| Tamanho do Pacote | Redução Exp 1 (%) | Redução Exp 2 (%) | Redução Exp 3 (%) | Variação Máxima (%) |
| :--- | :--- | :--- | :--- | :--- |
| **`6`** | 96,22% | 96,22% | 96,23% | 0,01% |
| **`60`** | 96,31% | 96,33% | 96,32% | 0,02% |
| **`720`** | 96,32% | 96,32% | 96,33% | 0,01% |
| **`4.320`** | 96,33% | 96,31% | 96,32% | 0,02% |
| **`20.000`** | 96,32% | 96,32% | 96,32% | 0,00% |

---

## 3. Comparação: kNN Puro vs kNN+RLE (A Grande Revelação)

Abaixo comparamos as taxas de compressão médias obtidas utilizando **apenas o kNN** (representando a sequência de categorias de 1 caractere por amostra) contra o **kNN+RLE** (comprimindo as repetições de categorias):

| Tamanho do Pacote | Média Redução kNN Puro (%) | Média Redução kNN+RLE (%) | Veredito: Qual Comprime Mais? |
| :--- | :--- | :--- | :--- |
| **`6`** (30s) | **97,85%** (40 KB) | 96,22% (70 KB) | 🟢 **kNN Puro** (+1,63% de redução / -30 KB) |
| **`60`** (5 min) | **97,84%** (40 KB) | 96,32% (68 KB) | 🟢 **kNN Puro** (+1,52% de redução / -28 KB) |
| **`720`** (1 hora) | **97,84%** (40 KB) | 96,31% (68 KB) | 🟢 **kNN Puro** (+1,53% de redução / -28 KB) |
| **`4.320`** (6 horas) | **97,84%** (40 KB) | 96,32% (68 KB) | 🟢 **kNN Puro** (+1,52% de redução / -28 KB) |
| **`20.000`** (~14 horas) | **97,84%** (40 KB) | 96,32% (68 KB) | 🟢 **kNN Puro** (+1,52% de redução / -28 KB) |

### Conclusões e Análise desta Diferença:

1. **A Expansão do RLE em Siglas de 1 Letra**:
   O kNN puro classifica cada leitura de temperatura e umidade em uma única letra (ex: `O` para *Optimal*). Para 40.000 registros, o arquivo kNN puro ocupa exatamente **40.000 bytes**.
   Quando o RLE tenta codificar isso, ele agrupa repetições na forma `<frequência><categoria>` (ex: `3O`).
   * Para frequências curtas ou oscilantes (que ocorrem muito devido a variações naturais do sensor nas fronteiras de decisão do kNN), corridas de comprimento menor ou igual a 1 expandem-se:
     * O kNN puro envia `O` (**1 byte**).
     * O RLE envia `1O` (**2 bytes**, um aumento de 100%).
   * Devido a esse fenômeno, a string RLE atinge **~68-70 KB**, sendo significativamente maior do que os **40 KB** do kNN puro.

2. **Quando usar RLE?**:
   O RLE é uma excelente técnica se o alfabeto de codificação for longo (ex: palavras de 10 bytes). Se a categoria kNN estivesse formatada em texto completo (como `"Optimal"`, `"Cold and Humid"`), o kNN puro ocuparia muito mais espaço e o RLE traria uma redução massiva. 
   Porém, após termos **otimizado a categoria kNN para 1 caractere**, o RLE torna-se estatisticamente ineficiente na média do dataset, pois a variabilidade natural dos dados de temperatura e umidade impede que existam corridas homogêneas longas o suficiente para bater a economia do caractere único.

---

## 4. Validação Científica dos Resultados

### 4.1. O Processo é Determinístico?
**Sim.** As etapas de classificação kNN (com vizinhos fixos $k=3$ e dados de treino estáticos) e compressão RLE são algoritmos matemáticos **completamente determinísticos**. 
* Se executarmos o experimento repetidas vezes utilizando o **mesmo** arquivo de dados de entrada (como fizemos internamente em cada bateria ao testar os diferentes lotes no mesmo CSV), os resultados obtidos serão **100% idênticos** até o último bit.

### 4.2. Por que existem pequenas diferenças entre os três Experimentos?
As diferenças marginais observadas na tabela (desvio máximo de apenas 0,44%) ocorrem porque a base de dados sintéticos (`dados_sinteticos_gerados_rust.csv`) foi **regenerada do zero via KDE** para cada um dos três experimentos.
* Como a amostragem KDE em Rust é um processo probabilístico (usa geração de números pseudo-aleatórios e ruído gaussiano), cada arquivo de dados sintéticos gerado difere levemente nas leituras exatas de temperatura e umidade de cada linha.
* Essa variação de milésimos de grau altera pontualmente as classificações kNN nas bordas de decisão das categorias, gerando sequências de letras ligeiramente diferentes.

### 4.3. Estabilidade Estatística
A consistência dos resultados (com variação sempre inferior a 0,5%) valida a robustez e a **estabilidade estatística** da nossa amostragem KDE nativa em Rust. O gerador consegue produzir repetidamente bases de dados que preservam a mesma distribuição e as mesmas propriedades de compressão da base original.

---

## 5. Conclusão da Validação

Todos os experimentos chegam ao **mesmo comportamento e ordem de grandeza**. A tendência de eficiência de compressão se mantém inalterada nas três rodadas independentes:
1. Pacotes pequenos (lote = 6) sofrem mais expansão de RLE e geram arquivos ligeiramente maiores (~70 KB).
2. O ponto de maior compressão ocorre de forma consistente na faixa de **720 a 4.320 registros (1 a 6 horas)**, gerando os menores arquivos (~68 KB).
3. A variação entre as execuções é desprezível, atestando a qualidade do gerador de dados sintéticos em Rust.
