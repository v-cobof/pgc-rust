# Walkthrough - Extração de Estatísticas, Modo Offline e Dados Sintéticos

Aprimoramos com sucesso a capacidade de processamento em lote offline (Modo 4) para permitir a execução com dados sintéticos gerados em tempo de execução via Rust, aproximando-se do comportamento estatístico (KDE) anteriormente feito pelo script Python na pasta `gerador-de-dados`. Também criamos um script para validar e comparar estes dados em relação aos originais.

## Mudanças Realizadas

### 1. Componente Processador (`processador`)
* **Cargo.toml**:
  * Adicionada a dependência do crate `rand` para permitir a seleção de índices aleatórios e geração de ruído estocástico no ambiente WebAssembly (WASI).
* **src/main.rs**:
  * **Gerador Normal (Gaussiano)**: Implementada a função `next_gaussian` com o algoritmo Box-Muller para produzir ruído com distribuição normal de média 0 e desvio padrão 1, sem conflito com a palavra-chave `gen` do Rust 2024.
  * **Opção de Dados Sintéticos**: Atualizado o fluxo `batch` para detectar o parâmetro `--synthetic` (ou `--sintetico`).
  * **Amostragem KDE em Rust**: Se a flag estiver presente, o programa carrega o CSV base (ex: `entrada.csv`), seleciona 40.000 amostras uniformemente ao acaso, adiciona ruído gaussiano ($\sigma = 0.5$) em cada dimensão (temperatura e umidade), emula o comportamento do Kernel Density Estimator do scikit-learn e armazena os dados resultantes.
  * **Exportação dos Dados Sintéticos**: Salva a base sintetizada gerada em `dados_sinteticos_gerados_rust.csv` no diretório raiz.
  * **Parser de CLI Aprimorado**: Atualizada a função `main` para processar flags em qualquer ordem (separando flags como `--synthetic` de argumentos posicionais).

### 2. Script de Validação (`gerador-de-dados/validar_dados_rust.py`) [NEW]
* Criado um novo script em Python para realizar a validação estatística dos dados gerados pelo Rust contra a base original:
  * Executa o teste de Kolmogorov-Smirnov de duas amostras para temperatura e umidade.
  * Calcula as correlações de Pearson das duas distribuições.
  * Gera gráficos de Scatter Plot, Histograma, Box Plot e curvas de densidade 2D lado a lado para comparação visual.
  * Salva o gráfico comparativo em `comparacao_dados_rust.png`.
  * Trata codificações de terminal Windows (sem caracteres emoji) para evitar `UnicodeEncodeError`.

### 3. Script de Inicialização (`start_all.sh`)
* Alterada a opção do Modo `4` para permitir a passagem de flags adicionais (`$2`, `$3`), permitindo que a execução se comporte da seguinte forma:
  ```bash
  wsl bash -l start_all.sh 4 --synthetic
  ```

---

## Verificação e Resultados

Executamos a compilação e o Modo 4 com dados sintéticos. Os dados gerados pelo Rust foram validados com sucesso em relação aos dados reais.

### Logs da Execução com Dados Sintéticos:
```text
==========================================
🚀 Build concluído. Iniciando Modo: 4
==========================================
Configuração 4: Processamento Offline em Lote (Batch Mode)
📖 Carregando dados base de entrada.csv...
✅ 44023 registros carregados do CSV base.
🎲 Gerando 40000 registros sintéticos usando KDE (bandwidth = 0.5)...
💾 Salvando dados sintéticos gerados em 'dados_sinteticos_gerados_rust.csv'...
✅ Arquivo 'dados_sinteticos_gerados_rust.csv' salvo com sucesso.
==========================================
📊 Relatório de Processamento em Lote Gerado!
📁 Arquivo salvo em: estatisticas_compressao.json
📈 Total de registros processados: 40000
📦 Tamanho original acumulado: 1856634 bytes
🏷️  Tamanho após kNN: 40000 bytes (Redução: 97.85%)
🗜️  Tamanho após kNN+RLE: 69812 bytes (Redução: 96.24%)
==========================================
```

### Resultados da Validação Estatística (Rust vs Original):
Execução do script `validar_dados_rust.py`:
```text
[*] Carregando dados originais de: entrada.csv
[*] Carregando dados sinteticos do Rust de: dados_sinteticos_gerados_rust.csv
[OK] Dados originais: 44023 linhas
[OK] Dados sinteticos Rust: 40000 linhas

[KS Test] Executando Teste KS (Kolmogorov-Smirnov)...
(p-valor > 0.05 indica que as distribuicoes sao estatisticamente similares)

Temperatura:
  KS statistic: 0.0080
  p-valor: 0.1385
  [OK] Distribicoes similares (p > 0.05)

Umidade:
  KS statistic: 0.0040
  p-valor: 0.8974
  [OK] Distribicoes similares (p > 0.05)

[Correlation] Comparando correlacao de Pearson (Temperatura vs Umidade)...
Correlacao nos dados originais: -0.8837
Correlacao nos dados sinteticos (Rust): -0.8804
Diferenca absoluta: 0.0033

[Plot] Gerando graficos comparativos...
[OK] Graficos salvos como: comparacao_dados_rust.png

==================================================
RESUMO FINAL DA VALIDACAO (RUST VS ORIGINAL)
==================================================
Diferenca de correlacao: 0.0033
Teste KS (p-valor) Temperatura: 0.1385
Teste KS (p-valor) Umidade: 0.8974
Fim da validacao!
```

### Gráfico Comparativo:
Abaixo estão os plots de comparação gerados pelo validador. É possível ver a alta fidelidade da distribuição dos dados sintéticos gerados pelo processador em Rust:

![Gráfico de Comparação Rust vs Original](C:/Users/vcobo/.gemini/antigravity/brain/622f8990-49da-4e22-9cf2-74d22ade96d5/comparacao_dados_rust.png)
