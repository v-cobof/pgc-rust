import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import stats
from sklearn.neighbors import KernelDensity

# ==============================================================================
# 1. CARREGAR OS DADOS ORIGINAIS
# ==============================================================================
# Esta seção carrega o arquivo de entrada CSV contendo os dados reais coletados
# pelos sensores físicos de temperatura e umidade.
print("1. Carregando dados...")
df = pd.read_csv('entrada.csv')  # Nome do arquivo de dados reais de sensores
print(f"✅ Dados carregados: {len(df)} linhas")

# Exibe as primeiras 5 linhas do dataset para verificação visual inicial do formato
print("\nPrimeiras 5 linhas:")
print(df.head())

# ==============================================================================
# 2. ANALISAR DISTRIBUIÇÃO ORIGINAL
# ==============================================================================
# Esta seção calcula estatísticas descritivas (média, desvio padrão, min, max)
# e executa testes de hipótese de normalidade para determinar se os dados 
# seguem uma distribuição normal ou se exigem abordagens não-paramétricas como KDE.
print("\n2. Analisando distribuição original...")

# Exibe estatísticas gerais das colunas do dataset original
print("\nEstatísticas descritivas:")
print(df.describe())

# Executa o teste de normalidade de D'Agostino-Pearson para cada coluna.
# Se o p-valor for menor que 0.05, a hipótese nula de normalidade é rejeitada.
for col in df.columns:
    stat, p_valor = stats.normaltest(df[col].dropna())
    print(f"{col}: p-valor do teste de normalidade = {p_valor:.6f}")
    if p_valor < 0.05:
        print(f"  → Não é normal (use KDE)")

# ==============================================================================
# 3. TREINAR O MODELO KDE (KERNEL DENSITY ESTIMATION)
# ==============================================================================
# O estimador de densidade por kernel (KDE) é usado para modelar a distribuição 
# conjunta das variáveis contínuas (temperatura e umidade) de forma não-paramétrica.
print("\n3. Treinando modelo KDE...")

# O modelo do Scikit-Learn espera uma matriz bidimensional (n_amostras, n_features)
dados_original = df[['temperatura', 'umidade']].values

# Ajusta o parâmetro 'bandwidth' (largura de banda), que controla a suavização.
# Valores muito pequenos causam overfitting; valores muito grandes causam underfitting.
bandwidth = 0.5  

# Inicializa o KDE com o kernel gaussiano e ajusta aos dados originais
kde = KernelDensity(bandwidth=bandwidth, kernel='gaussian')
kde.fit(dados_original)
print(f"✅ Modelo KDE treinado (bandwidth = {bandwidth})")

# ==============================================================================
# 4. GERAR DADOS SINTÉTICOS
# ==============================================================================
# A partir do modelo KDE ajustado, amostra-se um conjunto de dados sintéticos 
# contendo 40.000 registros de temperatura e umidade, que representam a distribuição.
print("\n4. Gerando dados sintéticos...")
n_amostras = 40000  # Quantidade de amostras sintéticas a gerar

# Gera os pontos de forma estocástica e cria um DataFrame pandas com as mesmas colunas
dados_sinteticos = kde.sample(n_samples=n_amostras)
df_sintetico = pd.DataFrame(dados_sinteticos, columns=['temperatura', 'umidade'])
print(f"✅ Geradas {n_amostras} linhas sintéticas")

# ==============================================================================
# 5. TESTE DE KOLMOGOROV-SMIRNOV (KS Test)
# ==============================================================================
# Executa o teste de KS de duas amostras para validar se as distribuições
# das variáveis sintéticas geradas diferem estatisticamente das reais.
# A hipótese nula H0 é que as amostras provêm da mesma distribuição de probabilidade.
# p-valor > 0.05 indica sucesso em rejeitar a diferença (distribuições similares).
print("\n5. Teste KS (Kolmogorov-Smirnov)...")
print("(Quanto maior o p-valor > 0.05, mais similares as distribuições)")

resultados_ks = {}
for col in df.columns:
    # Executa o teste KS de duas amostras para a variável atual (temp ou umid)
    ks_stat, p_valor = stats.ks_2samp(df[col], df_sintetico[col])
    resultados_ks[col] = {'statistic': ks_stat, 'p-value': p_valor}
    
    print(f"\n{col}:")
    print(f"  KS statistic: {ks_stat:.4f}")
    print(f"  p-valor: {p_valor:.4f}")
    
    if p_valor > 0.05:
        print(f"  ✅ Distribuições similares (p > 0.05)")
    else:
        print(f"  ⚠️  Distribuições podem ser diferentes (p <= 0.05)")

# ==============================================================================
# 6. COMPARAR CORRELAÇÕES
# ==============================================================================
# Esta seção calcula o coeficiente de correlação linear de Pearson para verificar
# se o gerador KDE conseguiu reter a relação física inversa intrínseca existente
# entre as variáveis de temperatura e umidade nos dados gerados.
print("\n6. Comparando correlação entre variáveis...")

corr_original = df.corr().iloc[0,1]
corr_sintetico = df_sintetico.corr().iloc[0,1]

print(f"Correlação original: {corr_original:.4f}")
print(f"Correlação sintética: {corr_sintetico:.4f}")
print(f"Diferença: {abs(corr_original - corr_sintetico):.4f}")

# ==============================================================================
# 7. GRÁFICOS DE COMPARAÇÃO
# ==============================================================================
# Renderiza painéis de gráficos comparativos para avaliar visualmente e salvar 
# em um arquivo PNG a similaridade das curvas de densidade e dispersão.
print("\n7. Gerando gráficos...")

# Define estilo visual
sns.set_style("whitegrid")
fig = plt.figure(figsize=(15, 10))

# Subgráfico 1: Scatter plot (dispersão) dos dados reais originais
ax1 = plt.subplot(2, 3, 1)
ax1.scatter(df['temperatura'], df['umidade'], alpha=0.3, s=1)
ax1.set_title('Original - Scatter Plot')
ax1.set_xlabel('Temperatura')
ax1.set_ylabel('Umidade')

# Subgráfico 2: Scatter plot (dispersão) dos dados sintéticos gerados pelo KDE
ax2 = plt.subplot(2, 3, 2)
ax2.scatter(df_sintetico['temperatura'], df_sintetico['umidade'], alpha=0.3, s=1, c='orange')
ax2.set_title('Sintético (KDE) - Scatter Plot')
ax2.set_xlabel('Temperatura')
ax2.set_ylabel('Umidade')

# Subgráfico 3: Histograma e curva de densidade estimada para a Temperatura
ax3 = plt.subplot(2, 3, 3)
ax3.hist(df['temperatura'], bins=50, alpha=0.5, label='Original', density=True)
ax3.hist(df_sintetico['temperatura'], bins=50, alpha=0.5, label='Sintético', density=True)
ax3.set_title('Distribuição - Temperatura')
ax3.set_xlabel('Temperatura')
ax3.set_ylabel('Densidade')
ax3.legend()

# Subgráfico 4: Histograma e curva de densidade estimada para a Umidade
ax4 = plt.subplot(2, 3, 4)
ax4.hist(df['umidade'], bins=50, alpha=0.5, label='Original', density=True)
ax4.hist(df_sintetico['umidade'], bins=50, alpha=0.5, label='Sintético', density=True)
ax4.set_title('Distribuição - Umidade')
ax4.set_xlabel('Umidade')
ax4.set_ylabel('Densidade')
ax4.legend()

# Subgráfico 5: Boxplot comparando a distribuição estatística de ambas as bases
ax5 = plt.subplot(2, 3, 5)
dados_box = pd.DataFrame({
    'Original_Temp': df['temperatura'],
    'Sintético_Temp': df_sintetico['temperatura'],
    'Original_Umid': df['umidade'],
    'Sintético_Umid': df_sintetico['umidade']
})
dados_box.boxplot(ax=ax5)
ax5.set_title('Box Plot Comparison')
ax5.tick_params(axis='x', rotation=45)

# Subgráfico 6: Linhas de contorno de densidade bidimensional estimada para as duas bases
ax6 = plt.subplot(2, 3, 6)
sns.kdeplot(data=df, x='temperatura', y='umidade', ax=ax6, label='Original')
sns.kdeplot(data=df_sintetico, x='temperatura', y='umidade', ax=ax6, label='Sintético', linestyle='--')
ax6.set_title('Densidade 2D (KDE)')
ax6.legend()

# Ajusta layouts dos subgráficos e salva a imagem combinada
plt.tight_layout()
plt.savefig('comparacao_dados.png', dpi=150, bbox_inches='tight')
plt.close()

print("\n✅ Gráfico salvo como 'comparacao_dados.png'")

# ==============================================================================
# 8. SALVAR DADOS SINTÉTICOS
# ==============================================================================
# Exporta os dados sintéticos em formato CSV para uso posterior nos testes do simulador.
print("\n8. Salvando dados sintéticos...")
df_sintetico.to_csv('dados_sinteticos.csv', index=False)
print("✅ Arquivo salvo como 'dados_sinteticos.csv'")

# ==============================================================================
# 9. RESUMO FINAL
# ==============================================================================
# Apresenta uma síntese final das métricas de fidelidade obtidas.
print("\n" + "="*50)
print("RESUMO FINAL")
print("="*50)
print(f"✅ Dados originais: {len(df)} linhas")
print(f"✅ Dados sintéticos: {len(df_sintetico)} linhas")
print(f"\nTeste KS (p-valor):")
for col, res in resultados_ks.items():
    print(f"  {col}: {res['p-value']:.4f}")
print(f"\nDiferença de correlação: {abs(corr_original - corr_sintetico):.4f}")
print("\n✨ Processo concluído com sucesso!")